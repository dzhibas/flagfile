//! One-way transpiler: Flagfile IR -> LaunchDarkly flag representation.
//!
//! Pure, no I/O. `transpile()` turns a `ParsedFlagfile` into a set of
//! `LdFlag`s that the (separate) sync engine diffs against live LD state and
//! pushes via semantic patch. Anything that cannot be represented in LD's
//! static, variation-based, ANDed-clause model is surfaced as a
//! `TranspileError` rather than silently dropped.
//!
//! Pipeline per flag:
//!   rules ──split @env──> per-env rule list
//!        ──synthesize variations (dedupe FlagReturn)──> variations[] + index map
//!        ──lower each conditional rule──> NNF/DNF ──> one LD rule per OR-term
//!
//! Ownership boundary (one-way "git owns flags"): we emit variations, rules,
//! fallthrough and prerequisites. We deliberately DO NOT emit `on` or
//! individual `targets`, so live kill-switches / ad-hoc targeting set in the
//! LD UI survive the next merge. See `RuleTarget` / `LdEnvironment`.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use crate::ast::{ArrayOp, AstNode, Atom, ComparisonOp, FlagMetadata, FnCall, LogicOp, MatchOp};
use crate::eval::Segments;
use crate::parse_flagfile::{FlagDefinition, FlagReturn, ParsedFlagfile, Rule};

// ──────────────────────────── configuration ────────────────────────────

/// Maps Flagfile concepts onto a concrete LD project/environment layout.
pub struct TranspileConfig {
    pub project_key: String,
    /// `@env <name>` -> LD environment key. Rules outside any `@env` block
    /// apply to every environment listed here.
    pub env_keys: BTreeMap<String, String>,
    /// Context kind for every clause. Flagfile has a flat context, LD v2
    /// needs a kind per clause; default "user".
    pub default_context_kind: String,
}

// ──────────────────────────── LD target model ───────────────────────────
// Only the subset we write. Field names match the LD REST schema so these
// serialize straight into create/patch bodies.

#[derive(Debug, Serialize, PartialEq)]
pub struct LdFlag {
    pub key: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub variations: Vec<LdVariation>,
    pub tags: Vec<String>,
    /// keyed by LD environment key
    pub environments: BTreeMap<String, LdEnvironment>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct LdVariation {
    pub value: Value,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LdEnvironment {
    // NB: `on` and `targets` intentionally omitted — owned by LD at runtime.
    pub rules: Vec<LdRule>,
    pub fallthrough: RuleTarget,
    pub off_variation: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub prerequisites: Vec<LdPrerequisite>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct LdRule {
    /// Human-readable rule label, from a Flagfile `@name` annotation. Maps to LD's
    /// per-rule `description` field shown in the targeting UI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub clauses: Vec<LdClause>,
    #[serde(flatten)]
    pub target: RuleTarget,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LdClause {
    pub context_kind: String,
    pub attribute: String,
    pub op: String,
    pub values: Vec<Value>,
    pub negate: bool,
}

/// What a rule (or fallthrough) serves: a fixed variation, or a percentage
/// rollout (from a `percentage(...)` term).
#[derive(Debug, Serialize, PartialEq)]
#[serde(untagged)]
pub enum RuleTarget {
    Variation { variation: usize },
    Rollout { rollout: LdRollout },
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LdRollout {
    /// weights are thousandths of a percent and must sum to 100_000
    pub variations: Vec<LdWeightedVariation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bucket_by: Option<String>,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct LdWeightedVariation {
    pub variation: usize,
    pub weight: u32,
}

#[derive(Debug, Serialize, PartialEq, Clone)]
pub struct LdPrerequisite {
    pub key: String,
    pub variation: usize,
}

// ──────────────────────────── errors ────────────────────────────────────

#[derive(Debug, PartialEq)]
pub enum TranspileError {
    /// Flag mixes return kinds (e.g. bool + json) -> not one LD flag.
    MixedVariationKinds { flag: String },
    /// `NOW()`, time-relative rules: LD has no eval-time clock clause.
    TimeRelative { flag: String },
    /// `coalesce()`, `upper()/lower()` on attributes, null checks — no LD op.
    UnsupportedConstruct { flag: String, what: String },
    /// Clause shape we can't read (e.g. literal on the left of a compare).
    UnsupportedClauseShape { flag: String },
    /// `@requires` pointing at a flag we can't resolve to a variation.
    UnresolvedPrerequisite { flag: String, requires: String },
    /// More than one `percentage(...)` in a single conjunction.
    MultiplePercentage { flag: String },
}

// ──────────────────────────── entry point ───────────────────────────────

pub fn transpile(
    parsed: &ParsedFlagfile,
    cfg: &TranspileConfig,
) -> Result<Vec<LdFlag>, Vec<TranspileError>> {
    let mut out = Vec::new();
    let mut errors = Vec::new();

    for flag_map in &parsed.flags {
        for (name, def) in flag_map {
            match transpile_flag(name, def, &parsed.segments, cfg) {
                Ok(flag) => out.push(flag),
                Err(mut e) => errors.append(&mut e),
            }
        }
    }

    if errors.is_empty() {
        Ok(out)
    } else {
        Err(errors)
    }
}

fn transpile_flag(
    name: &str,
    def: &FlagDefinition,
    segments: &Segments,
    cfg: &TranspileConfig,
) -> Result<LdFlag, Vec<TranspileError>> {
    let mut errors = Vec::new();

    // 1. Synthesize variations from every distinct return value in the flag.
    let (variations, index_of) = match synthesize_variations(name, def) {
        Ok(v) => v,
        Err(e) => return Err(vec![e]),
    };

    // 2. Split rules by environment. Non-@env rules become the "base" applied
    //    to every configured env; @env blocks override/extend per env.
    let (base_rules, env_rules) = partition_env_rules(&def.rules);

    let mut environments = BTreeMap::new();
    for (env_name, env_key) in &cfg.env_keys {
        let rules_for_env: Vec<&Rule> = base_rules
            .iter()
            .copied()
            .chain(env_rules.get(env_name).into_iter().flatten().copied())
            .collect();

        match lower_environment(name, &rules_for_env, &index_of, &variations, segments, cfg) {
            Ok(env) => {
                environments.insert(env_key.clone(), env);
            }
            Err(mut e) => errors.append(&mut e),
        }
    }

    // 3. Prerequisites from @requires (boolean prereqs -> their `true` index).
    let prerequisites = match lower_prerequisites(name, &def.metadata) {
        Ok(p) => p,
        Err(e) => {
            errors.push(e);
            Vec::new()
        }
    };
    for env in environments.values_mut() {
        env.prerequisites = prerequisites.clone();
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(LdFlag {
        key: name.to_string(),
        name: name.to_string(),
        description: def.metadata.description.clone(),
        variations,
        tags: metadata_to_tags(&def.metadata),
        environments,
    })
}

// ──────────────────────────── variations ────────────────────────────────

/// Collect distinct FlagReturn values -> LD variations, plus a lookup from a
/// return value to its variation index. Enforces a single LD kind per flag.
fn synthesize_variations(
    flag: &str,
    def: &FlagDefinition,
) -> Result<(Vec<LdVariation>, ReturnIndex), TranspileError> {
    let mut variations: Vec<LdVariation> = Vec::new();
    let mut kind: Option<LdKind> = None;

    let mut record = |ret: &FlagReturn| -> Result<(), TranspileError> {
        let value = flag_return_to_value(ret);
        let k = LdKind::of(&value);
        match &kind {
            Some(prev) if *prev != k => {
                return Err(TranspileError::MixedVariationKinds { flag: flag.into() })
            }
            None => kind = Some(k),
            _ => {}
        }
        if !variations.iter().any(|v| v.value == value) {
            variations.push(LdVariation { value });
        }
        Ok(())
    };

    walk_returns(&def.rules, &mut record)?;

    // LD requires every flag to expose at least two variations. A Flagfile that
    // only ever yields one value (e.g. `FF-x => true`) still transpiles to a
    // single variation, so pad it: booleans gain their missing true/false
    // counterpart; other kinds gain a kind-appropriate "disabled" placeholder
    // (LD rejects `null` values for typed/JSON variations).
    let push_unique = |variations: &mut Vec<LdVariation>, value: Value| {
        if !variations.iter().any(|v| v.value == value) {
            variations.push(LdVariation { value });
        }
    };
    match kind {
        Some(LdKind::Boolean) | None => {
            push_unique(&mut variations, Value::Bool(true));
            push_unique(&mut variations, Value::Bool(false));
        }
        Some(k) => {
            if variations.len() < 2 {
                let existing = variations.first().map(|v| v.value.clone());
                push_unique(&mut variations, off_placeholder(k, existing.as_ref()));
            }
        }
    }

    let values: Vec<Value> = variations.iter().map(|v| v.value.clone()).collect();
    Ok((variations, ReturnIndex { variations: values }))
}

/// A non-null "disabled" placeholder used to satisfy LD's two-variation minimum
/// for single-value non-boolean flags, guaranteed distinct from `existing`.
fn off_placeholder(kind: LdKind, existing: Option<&Value>) -> Value {
    let primary = match kind {
        LdKind::String => Value::String(String::new()),
        LdKind::Number => Value::Number(0.into()),
        LdKind::Json => Value::Object(serde_json::Map::new()),
        LdKind::Boolean => Value::Bool(false),
    };
    if Some(&primary) != existing {
        return primary;
    }
    // The real value already equals the natural placeholder — pick another.
    match kind {
        LdKind::String => Value::String("__flagfile_off__".to_string()),
        LdKind::Number => Value::Number(1.into()),
        LdKind::Json => {
            let mut m = serde_json::Map::new();
            m.insert("__flagfile_off__".to_string(), Value::Bool(true));
            Value::Object(m)
        }
        LdKind::Boolean => Value::Bool(true),
    }
}

fn walk_returns(
    rules: &[Rule],
    f: &mut impl FnMut(&FlagReturn) -> Result<(), TranspileError>,
) -> Result<(), TranspileError> {
    for r in rules {
        match r {
            Rule::Value(ret) => f(ret)?,
            Rule::BoolExpressionValue(_, ret, _) => f(ret)?,
            Rule::EnvRule { rules, .. } => walk_returns(rules, f)?,
        }
    }
    Ok(())
}

/// Resolves a FlagReturn to its synthesized variation index.
struct ReturnIndex {
    variations: Vec<Value>,
}
impl ReturnIndex {
    fn index_of(&self, ret: &FlagReturn) -> usize {
        let v = flag_return_to_value(ret);
        self.variations.iter().position(|x| *x == v).unwrap_or(0)
    }
    fn bool_index(&self, want: bool) -> Option<usize> {
        self.variations.iter().position(|v| *v == Value::Bool(want))
    }
}

fn flag_return_to_value(ret: &FlagReturn) -> Value {
    match ret {
        FlagReturn::OnOff(b) => Value::Bool(*b),
        FlagReturn::Integer(n) => Value::Number((*n).into()),
        FlagReturn::Str(s) => Value::String(s.clone()),
        FlagReturn::Json(v) => v.clone(),
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum LdKind {
    Boolean,
    Number,
    String,
    Json,
}
impl LdKind {
    fn of(v: &Value) -> Self {
        match v {
            Value::Bool(_) => LdKind::Boolean,
            Value::Number(_) => LdKind::Number,
            Value::String(_) => LdKind::String,
            _ => LdKind::Json,
        }
    }
}

// ──────────────────────────── env partition ─────────────────────────────

type IndexMap = ReturnIndex;

fn partition_env_rules(rules: &[Rule]) -> (Vec<&Rule>, BTreeMap<String, Vec<&Rule>>) {
    let mut base = Vec::new();
    let mut by_env: BTreeMap<String, Vec<&Rule>> = BTreeMap::new();
    for r in rules {
        match r {
            Rule::EnvRule { env, rules } => {
                by_env.entry(env.clone()).or_default().extend(rules.iter());
            }
            other => base.push(other),
        }
    }
    (base, by_env)
}

// ──────────────────────────── environment lowering ──────────────────────

fn lower_environment(
    flag: &str,
    rules: &[&Rule],
    index_of: &IndexMap,
    _variations: &[LdVariation],
    segments: &Segments,
    cfg: &TranspileConfig,
) -> Result<LdEnvironment, Vec<TranspileError>> {
    let mut errors = Vec::new();
    let mut ld_rules = Vec::new();
    let mut fallthrough: Option<RuleTarget> = None;

    for rule in rules {
        match rule {
            // A bare value is the block default -> fallthrough.
            Rule::Value(ret) => {
                fallthrough = Some(RuleTarget::Variation {
                    variation: index_of.index_of(ret),
                });
            }
            Rule::BoolExpressionValue(cond, ret, name) => {
                match lower_condition(flag, cond, ret, name.as_deref(), index_of, segments, cfg) {
                    Ok(mut produced) => ld_rules.append(&mut produced),
                    Err(mut e) => errors.append(&mut e),
                }
            }
            Rule::EnvRule { .. } => unreachable!("env rules partitioned out"),
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    // When the flag is off, serve `false` for booleans (LD convention),
    // otherwise the first variation.
    let off_variation = index_of.bool_index(false).unwrap_or(0);

    Ok(LdEnvironment {
        rules: ld_rules,
        // No explicit default -> serve the off value.
        fallthrough: fallthrough.unwrap_or(RuleTarget::Variation {
            variation: off_variation,
        }),
        off_variation,
        prerequisites: Vec::new(),
    })
}

/// One Flagfile conditional rule -> one-or-more LD rules (via DNF over OR).
fn lower_condition(
    flag: &str,
    cond: &AstNode,
    ret: &FlagReturn,
    name: Option<&str>,
    index_of: &IndexMap,
    segments: &Segments,
    cfg: &TranspileConfig,
) -> Result<Vec<LdRule>, Vec<TranspileError>> {
    let mut errors = Vec::new();
    // Resolve Flagfile `@segment` references to their defining expressions so the
    // emitted clauses don't depend on a segment existing in LD. `inlined` must
    // outlive `dnf`, which borrows from it.
    let inlined = match inline_segments(flag, cond, segments, &mut Vec::new()) {
        Ok(n) => n,
        Err(e) => return Err(vec![e]),
    };
    let dnf = match to_dnf(flag, &inlined, false) {
        Ok(d) => d,
        Err(e) => return Err(vec![e]),
    };

    let mut out = Vec::new();
    for conjunction in dnf {
        let mut clauses = Vec::new();
        let mut rollout_term: Option<&PercentageLit> = None;

        for lit in &conjunction {
            match lit {
                Literal::Pred { node, negate } => {
                    match lower_leaf(flag, node, *negate, segments, cfg) {
                        Ok(c) => clauses.push(c),
                        Err(e) => errors.push(e),
                    }
                }
                Literal::Pct(p) => {
                    if rollout_term.is_some() {
                        errors.push(TranspileError::MultiplePercentage { flag: flag.into() });
                    }
                    rollout_term = Some(p);
                }
            }
        }

        let target = match rollout_term {
            None => RuleTarget::Variation {
                variation: index_of.index_of(ret),
            },
            Some(p) => build_rollout(flag, p, ret, index_of).unwrap_or_else(|e| {
                errors.push(e);
                RuleTarget::Variation { variation: 0 }
            }),
        };
        out.push(LdRule {
            description: name.map(|s| s.to_string()),
            clauses,
            target,
        });
    }

    if errors.is_empty() {
        Ok(out)
    } else {
        Err(errors)
    }
}

// ──────────────────────────── segment inlining ──────────────────────────
// LD only accepts `segmentMatch` against segments that already exist in the
// project. A Flagfile `@segment` is just a named boolean expression, so we
// substitute each `segment(name)` reference with its defining expression before
// lowering. A `segment(...)` that isn't defined in the Flagfile is left intact
// (assumed to be a natively-managed LD segment) and still lowers to a
// `segmentMatch` clause. Substitution is recursive (segments may reference other
// segments) with cycle detection.

fn inline_segments(
    flag: &str,
    node: &AstNode,
    segments: &Segments,
    seen: &mut Vec<String>,
) -> Result<AstNode, TranspileError> {
    match node {
        AstNode::Segment(name) => match segments.get(name) {
            Some(expr) => {
                if seen.iter().any(|n| n == name) {
                    return Err(TranspileError::UnsupportedConstruct {
                        flag: flag.into(),
                        what: format!("recursive segment \"{name}\""),
                    });
                }
                seen.push(name.clone());
                let inlined = inline_segments(flag, expr, segments, seen)?;
                seen.pop();
                Ok(inlined)
            }
            // Not a Flagfile segment — keep it; lower_leaf emits a segmentMatch.
            None => Ok(node.clone()),
        },
        AstNode::Logic(l, op, r) => Ok(AstNode::Logic(
            Box::new(inline_segments(flag, l, segments, seen)?),
            op.clone(),
            Box::new(inline_segments(flag, r, segments, seen)?),
        )),
        AstNode::Scope { expr, negate } => Ok(AstNode::Scope {
            expr: Box::new(inline_segments(flag, expr, segments, seen)?),
            negate: *negate,
        }),
        // Segment references only occur at boolean positions, so leaf predicates
        // and other constructs are returned unchanged.
        other => Ok(other.clone()),
    }
}

// ──────────────────────────── DNF / NNF ──────────────────────────────────
// Disjunctive normal form: Vec<conjunction>, conjunction = Vec<Literal>.
// LD rules are ANDed clauses with no in-rule OR, so each conjunction becomes
// one LD rule. Negation is pushed to the leaves (De Morgan) so it can fold
// into LdClause.negate.

#[derive(Clone)]
enum Literal<'a> {
    Pred { node: &'a AstNode, negate: bool },
    Pct(PercentageLit),
}

#[derive(Clone)]
struct PercentageLit {
    rate: f64,
    field: String,
}

type Conjunction<'a> = Vec<Literal<'a>>;

fn to_dnf<'a>(
    flag: &str,
    node: &'a AstNode,
    negated: bool,
) -> Result<Vec<Conjunction<'a>>, TranspileError> {
    match node {
        AstNode::Logic(l, LogicOp::And, r) => {
            if negated {
                // ¬(A ∧ B) = ¬A ∨ ¬B
                let mut d = to_dnf(flag, l, true)?;
                d.extend(to_dnf(flag, r, true)?);
                Ok(d)
            } else {
                // distribute: (a1∨a2) ∧ (b1∨b2) = a1b1 ∨ a1b2 ∨ a2b1 ∨ a2b2
                let left = to_dnf(flag, l, false)?;
                let right = to_dnf(flag, r, false)?;
                let mut out = Vec::new();
                for a in &left {
                    for b in &right {
                        let mut conj = a.clone();
                        conj.extend(b.clone());
                        out.push(conj);
                    }
                }
                Ok(out)
            }
        }
        AstNode::Logic(l, LogicOp::Or, r) => {
            if negated {
                // ¬(A ∨ B) = ¬A ∧ ¬B  -> product of the negated sides
                let left = to_dnf(flag, l, true)?;
                let right = to_dnf(flag, r, true)?;
                let mut out = Vec::new();
                for a in &left {
                    for b in &right {
                        let mut conj = a.clone();
                        conj.extend(b.clone());
                        out.push(conj);
                    }
                }
                Ok(out)
            } else {
                let mut d = to_dnf(flag, l, false)?;
                d.extend(to_dnf(flag, r, false)?);
                Ok(d)
            }
        }
        AstNode::Scope { expr, negate } => to_dnf(flag, expr, negated ^ negate),

        AstNode::Percentage { rate, field, .. } => {
            // salt is dropped: LD owns the bucketing seed.
            let field = field.as_str().unwrap_or("key").to_string();
            Ok(vec![vec![Literal::Pct(PercentageLit {
                rate: *rate,
                field,
            })]])
        }

        // Constructs that have no clause form at all.
        AstNode::Coalesce(_) => Err(TranspileError::UnsupportedConstruct {
            flag: flag.into(),
            what: "coalesce()".into(),
        }),
        AstNode::NullCheck { .. } => Err(TranspileError::UnsupportedConstruct {
            flag: flag.into(),
            what: "null check".into(),
        }),
        AstNode::Function(FnCall::Now, _) => {
            Err(TranspileError::TimeRelative { flag: flag.into() })
        }
        AstNode::Function(_, _) => Err(TranspileError::UnsupportedConstruct {
            flag: flag.into(),
            what: "attribute function (upper/lower)".into(),
        }),

        // Anything else is a leaf predicate (Compare / Match / Array / Segment).
        leaf => Ok(vec![vec![Literal::Pred {
            node: leaf,
            negate: negated,
        }]]),
    }
}

// ──────────────────────────── leaf -> clause ────────────────────────────

fn lower_leaf(
    flag: &str,
    node: &AstNode,
    outer_negate: bool,
    _segments: &Segments,
    cfg: &TranspileConfig,
) -> Result<LdClause, TranspileError> {
    match node {
        AstNode::Segment(name) => Ok(LdClause {
            context_kind: cfg.default_context_kind.clone(),
            attribute: "key".into(),
            op: "segmentMatch".into(),
            values: vec![Value::String(name.clone())],
            negate: outer_negate,
        }),

        AstNode::Compare(lhs, op, rhs) => {
            let attribute = lhs
                .as_str()
                .ok_or(TranspileError::UnsupportedClauseShape { flag: flag.into() })?
                .to_string();
            // NOW() / function on either side -> not representable.
            if matches!(**lhs, AstNode::Function(..)) || matches!(**rhs, AstNode::Function(..)) {
                return Err(TranspileError::TimeRelative { flag: flag.into() });
            }
            let value = constant_value(flag, rhs)?;
            let (ld_op, op_negate) = map_comparison(op, &value);
            Ok(LdClause {
                context_kind: cfg.default_context_kind.clone(),
                attribute,
                op: ld_op.into(),
                values: vec![value],
                negate: outer_negate ^ op_negate,
            })
        }

        AstNode::Match(lhs, op, rhs) => {
            let attribute = lhs
                .as_str()
                .ok_or(TranspileError::UnsupportedClauseShape { flag: flag.into() })?
                .to_string();
            let value = constant_value(flag, rhs)?;
            let (ld_op, op_negate) = map_match(op);
            Ok(LdClause {
                context_kind: cfg.default_context_kind.clone(),
                attribute,
                op: ld_op.into(),
                values: vec![value],
                negate: outer_negate ^ op_negate,
            })
        }

        AstNode::Array(lhs, op, rhs) => {
            let attribute = lhs
                .as_str()
                .ok_or(TranspileError::UnsupportedClauseShape { flag: flag.into() })?
                .to_string();
            let values = list_values(flag, rhs)?;
            let op_negate = matches!(op, ArrayOp::NotIn);
            Ok(LdClause {
                context_kind: cfg.default_context_kind.clone(),
                attribute,
                op: "in".into(),
                values,
                negate: outer_negate ^ op_negate,
            })
        }

        _ => Err(TranspileError::UnsupportedClauseShape { flag: flag.into() }),
    }
}

/// (LD op, whether it implies negate). Date/semver-aware.
fn map_comparison(op: &ComparisonOp, value: &Value) -> (&'static str, bool) {
    let is_date = matches!(value, Value::String(s) if looks_like_date(s));
    let is_semver = matches!(value, Value::String(s) if looks_like_semver(s));
    match op {
        ComparisonOp::Eq => ("in", false),
        ComparisonOp::NotEq => ("in", true),
        ComparisonOp::More if is_date => ("after", false),
        ComparisonOp::Less if is_date => ("before", false),
        ComparisonOp::More if is_semver => ("semVerGreaterThan", false),
        ComparisonOp::Less if is_semver => ("semVerLessThan", false),
        ComparisonOp::More => ("greaterThan", false),
        ComparisonOp::Less => ("lessThan", false),
        ComparisonOp::MoreEq if is_semver => ("semVerGreaterThan", false), // TODO: emit OR semVerEqual
        ComparisonOp::LessEq if is_semver => ("semVerLessThan", false),    // TODO: ditto
        ComparisonOp::MoreEq => ("greaterThanOrEqual", false),
        ComparisonOp::LessEq => ("lessThanOrEqual", false),
    }
}

fn map_match(op: &MatchOp) -> (&'static str, bool) {
    match op {
        MatchOp::Contains => ("contains", false),
        MatchOp::NotContains => ("contains", true),
        MatchOp::StartsWith => ("startsWith", false),
        MatchOp::NotStartsWith => ("startsWith", true),
        MatchOp::EndsWith => ("endsWith", false),
        MatchOp::NotEndsWith => ("endsWith", true),
    }
}

// ──────────────────────────── rollout ───────────────────────────────────

fn build_rollout(
    flag: &str,
    p: &PercentageLit,
    on_ret: &FlagReturn,
    index_of: &IndexMap,
) -> Result<RuleTarget, TranspileError> {
    // percentage(rate, field) -> true ⇒ rate% to the matched return, rest off.
    let on_idx = index_of.index_of(on_ret);
    let off_idx = index_of
        .bool_index(false)
        .or_else(|| index_of.bool_index(true).map(|_| 0))
        .unwrap_or(0);
    let on_weight = (p.rate.clamp(0.0, 100.0) * 1000.0).round() as u32;
    let _ = flag;
    Ok(RuleTarget::Rollout {
        rollout: LdRollout {
            variations: vec![
                LdWeightedVariation {
                    variation: on_idx,
                    weight: on_weight,
                },
                LdWeightedVariation {
                    variation: off_idx,
                    weight: 100_000 - on_weight,
                },
            ],
            bucket_by: Some(p.field.clone()),
        },
    })
}

// ──────────────────────────── prerequisites / metadata ──────────────────

fn lower_prerequisites(
    flag: &str,
    meta: &FlagMetadata,
) -> Result<Vec<LdPrerequisite>, TranspileError> {
    // @requires FF-x means "x is true". We need x's `true` variation index,
    // which lives on the *other* flag — resolved in a second pass by the sync
    // engine once all flags' variations are known. Default to index 0 (true is
    // conventionally index 0 for boolean flags in LD) and let the engine fix up.
    let _ = flag;
    Ok(meta
        .requires
        .iter()
        .map(|k| LdPrerequisite {
            key: k.clone(),
            variation: 0,
        })
        .collect())
}

fn metadata_to_tags(meta: &FlagMetadata) -> Vec<String> {
    // TODO: owner -> maintainerId requires an LD member lookup; for now tag it.
    let mut tags = vec!["managed-by-flagfile".to_string()];
    if let Some(t) = &meta.flag_type {
        tags.push(format!("type:{t}"));
    }
    if let Some(o) = &meta.owner {
        tags.push(format!("owner:{o}"));
    }
    if let Some(tk) = &meta.ticket {
        tags.push(format!("ticket:{tk}"));
    }
    if let Some(d) = &meta.deprecated {
        let _ = d;
        tags.push("deprecated".into());
    }
    // TODO: meta.expires -> custom property or scheduled archive.
    tags
}

// ──────────────────────────── value helpers ─────────────────────────────

fn constant_value(flag: &str, node: &AstNode) -> Result<Value, TranspileError> {
    match node {
        AstNode::Constant(a) | AstNode::Variable(a) => Ok(atom_to_value(a)),
        _ => Err(TranspileError::UnsupportedClauseShape { flag: flag.into() }),
    }
}

fn list_values(flag: &str, node: &AstNode) -> Result<Vec<Value>, TranspileError> {
    match node {
        AstNode::List(items) => Ok(items.iter().map(atom_to_value).collect()),
        AstNode::Constant(Atom::List(items)) => Ok(items.iter().map(atom_to_value).collect()),
        AstNode::Constant(a) => Ok(vec![atom_to_value(a)]),
        _ => Err(TranspileError::UnsupportedClauseShape { flag: flag.into() }),
    }
}

fn atom_to_value(a: &Atom) -> Value {
    match a {
        Atom::String(s) | Atom::Variable(s) => Value::String(s.clone()),
        Atom::Number(n) => Value::Number((*n).into()),
        Atom::Float(f) => serde_json::Number::from_f64(*f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        Atom::Boolean(b) => Value::Bool(*b),
        Atom::Date(d) => Value::String(d.to_string()),
        Atom::DateTime(dt) => Value::String(dt.to_string()),
        Atom::Semver(a, b, c) => Value::String(format!("{a}.{b}.{c}")),
        Atom::Regex(r) => Value::String(r.clone()),
        Atom::List(items) => Value::Array(items.iter().map(atom_to_value).collect()),
    }
}

fn looks_like_date(s: &str) -> bool {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok()
}
fn looks_like_semver(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| p.parse::<u32>().is_ok())
}

// ──────────────────────────── tests ─────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> TranspileConfig {
        TranspileConfig {
            project_key: "default".into(),
            env_keys: BTreeMap::from([("prod".into(), "production".into())]),
            default_context_kind: "user".into(),
        }
    }

    // (A or B) and C  ->  two LD rules: {A,C} and {B,C}
    #[test]
    fn or_expands_to_two_rules() {
        use AstNode::*;
        let a = Compare(
            Box::new(Variable(Atom::Variable("country".into()))),
            ComparisonOp::Eq,
            Box::new(Constant(Atom::String("NL".into()))),
        );
        let b = Compare(
            Box::new(Variable(Atom::Variable("country".into()))),
            ComparisonOp::Eq,
            Box::new(Constant(Atom::String("BE".into()))),
        );
        let c = Compare(
            Box::new(Variable(Atom::Variable("tier".into()))),
            ComparisonOp::Eq,
            Box::new(Constant(Atom::String("premium".into()))),
        );
        let cond = Logic(
            Box::new(Logic(Box::new(a), LogicOp::Or, Box::new(b))),
            LogicOp::And,
            Box::new(c),
        );
        let dnf = to_dnf("FF-x", &cond, false).unwrap();
        assert_eq!(dnf.len(), 2);
        assert_eq!(dnf[0].len(), 2); // {A, C}
        assert_eq!(dnf[1].len(), 2); // {B, C}
    }

    // not (A or B)  ->  one rule {¬A, ¬B}
    #[test]
    fn negated_or_is_de_morgan() {
        use AstNode::*;
        let inner = Logic(
            Box::new(Compare(
                Box::new(Variable(Atom::Variable("x".into()))),
                ComparisonOp::Eq,
                Box::new(Constant(Atom::String("1".into()))),
            )),
            LogicOp::Or,
            Box::new(Compare(
                Box::new(Variable(Atom::Variable("y".into()))),
                ComparisonOp::Eq,
                Box::new(Constant(Atom::String("2".into()))),
            )),
        );
        let scoped = Scope {
            expr: Box::new(inner),
            negate: true,
        };
        let dnf = to_dnf("FF-x", &scoped, false).unwrap();
        assert_eq!(dnf.len(), 1);
        assert_eq!(dnf[0].len(), 2);
    }

    #[test]
    fn now_is_rejected() {
        let node = AstNode::Function(FnCall::Now, Box::new(AstNode::Void));
        assert!(matches!(
            to_dnf("FF-x", &node, false),
            Err(TranspileError::TimeRelative { .. })
        ));
    }

    #[test]
    fn semver_operator_mapping() {
        let (op, neg) = map_comparison(&ComparisonOp::More, &Value::String("5.3.2".into()));
        assert_eq!(op, "semVerGreaterThan");
        assert!(!neg);
    }
}

#[cfg(test)]
mod e2e {
    use super::*;
    use crate::parse_flagfile::parse_flagfile_with_segments;

    fn transpile_one(src: &str) -> LdFlag {
        let (_, parsed) = parse_flagfile_with_segments(src).expect("parse");
        let cfg = TranspileConfig {
            project_key: "default".into(),
            env_keys: BTreeMap::from([("_".into(), "production".into())]),
            default_context_kind: "user".into(),
        };
        let flags = transpile(&parsed, &cfg).expect("transpile");
        flags.into_iter().next().expect("one flag")
    }

    // LD rejects flags with < 2 variations; a constant boolean flag must still
    // produce both true and false, with off serving false.
    #[test]
    fn constant_boolean_flag_gets_two_variations() {
        let flag = transpile_one("FF-welcome-banner -> true\n");
        assert_eq!(flag.variations.len(), 2);
        assert!(flag.variations.iter().any(|v| v.value == Value::Bool(true)));
        assert!(flag
            .variations
            .iter()
            .any(|v| v.value == Value::Bool(false)));
        let false_idx = flag
            .variations
            .iter()
            .position(|v| v.value == Value::Bool(false))
            .unwrap();
        assert_eq!(flag.environments["production"].off_variation, false_idx);
    }

    // A single-value non-boolean flag is padded with a non-null, kind-appropriate
    // "disabled" variation (LD rejects null for typed/JSON variations).
    #[test]
    fn constant_string_flag_is_padded() {
        let flag = transpile_one("FF-theme -> \"dark\"\n");
        assert_eq!(flag.variations.len(), 2);
        assert!(flag.variations.iter().all(|v| !v.value.is_null()));
        assert!(flag
            .variations
            .iter()
            .any(|v| v.value == Value::String(String::new())));
    }

    // A single-value JSON flag is padded with an empty object, never null.
    #[test]
    fn constant_json_flag_is_padded_without_null() {
        let flag = transpile_one("FF-checking-json -> json({\"success\": true})\n");
        assert_eq!(flag.variations.len(), 2);
        assert!(flag.variations.iter().all(|v| !v.value.is_null()));
        assert!(flag
            .variations
            .iter()
            .any(|v| v.value == serde_json::json!({})));
    }

    // A named conditional rule carries its name as the LD rule `description`.
    #[test]
    fn named_rule_sets_description() {
        let flag = transpile_one(
            "FF-checkout {\n    @name \"EU rollout\"\n    country == \"NL\" -> true\n    false\n}\n",
        );
        let rules = &flag.environments["production"].rules;
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].description.as_deref(), Some("EU rollout"));
    }

    // An OR condition expands to several LD rules; each gets the same description.
    #[test]
    fn named_or_rule_names_all_expanded_rules() {
        let flag = transpile_one(
            "FF-checkout {\n    @name \"EU\"\n    country == \"NL\" or country == \"BE\" -> true\n    false\n}\n",
        );
        let rules = &flag.environments["production"].rules;
        assert_eq!(rules.len(), 2);
        assert!(rules.iter().all(|r| r.description.as_deref() == Some("EU")));
    }

    // Without an annotation the rule has no description (omitted from JSON).
    #[test]
    fn unnamed_rule_has_no_description() {
        let flag = transpile_one("FF-checkout {\n    country == \"NL\" -> true\n    false\n}\n");
        let rules = &flag.environments["production"].rules;
        assert_eq!(rules.len(), 1);
        assert!(rules[0].description.is_none());
        let json = serde_json::to_value(&rules[0]).unwrap();
        assert!(json.get("description").is_none());
    }

    // A `segment(...)` referencing a Flagfile @segment is inlined into clauses,
    // never emitted as a segmentMatch against a (possibly non-existent) LD segment.
    #[test]
    fn flagfile_segment_is_inlined() {
        let flag = transpile_one(
            "@segment eu_region {\n    country in (DE, FR, NL)\n}\n\nFF-testing_new {\n    segment(eu_region) -> true\n    false\n}\n",
        );
        let rules = &flag.environments["production"].rules;
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].clauses.len(), 1);
        let clause = &rules[0].clauses[0];
        assert_eq!(clause.op, "in");
        assert_eq!(clause.attribute, "country");
        assert!(clause.op != "segmentMatch");
        assert_eq!(
            clause.values,
            vec![
                Value::String("DE".into()),
                Value::String("FR".into()),
                Value::String("NL".into()),
            ]
        );
    }

    // An OR inside an inlined segment expands to multiple LD rules.
    #[test]
    fn segment_with_or_inlines_to_multiple_rules() {
        let flag = transpile_one(
            "@segment vip {\n    tier == \"gold\" or country == \"NL\"\n}\n\nFF-perk {\n    segment(vip) -> true\n    false\n}\n",
        );
        let rules = &flag.environments["production"].rules;
        assert_eq!(rules.len(), 2);
        assert!(rules
            .iter()
            .all(|r| r.clauses.iter().all(|c| c.op != "segmentMatch")));
    }

    // A segment(...) not defined in the Flagfile is assumed to be a native LD
    // segment and still lowers to a segmentMatch clause.
    #[test]
    fn unknown_segment_stays_segment_match() {
        let flag = transpile_one("FF-x {\n    segment(native_ld_seg) -> true\n    false\n}\n");
        let rules = &flag.environments["production"].rules;
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].clauses[0].op, "segmentMatch");
        assert_eq!(
            rules[0].clauses[0].values,
            vec![Value::String("native_ld_seg".into())]
        );
    }

    #[test]
    fn transpile_real_example_file() {
        let data = include_str!("../../Flagfile.example");
        let (rest, parsed) = parse_flagfile_with_segments(data).expect("parse example");
        assert_eq!(rest.trim(), "", "parser should consume the whole file");

        let cfg = TranspileConfig {
            project_key: "default".into(),
            env_keys: BTreeMap::from([("_".into(), "production".into())]),
            default_context_kind: "user".into(),
        };

        let mut ok = 0usize;
        let mut blocked: Vec<(String, String)> = Vec::new();
        let mut first_json: Option<String> = None;

        for flag_map in &parsed.flags {
            for (name, def) in flag_map {
                match transpile_flag(name, def, &parsed.segments, &cfg) {
                    Ok(flag) => {
                        ok += 1;
                        if first_json.is_none() && !flag.environments["production"].rules.is_empty()
                        {
                            first_json = Some(serde_json::to_string_pretty(&flag).unwrap());
                        }
                    }
                    Err(errs) => {
                        let why = format!("{:?}", errs.first().unwrap());
                        blocked.push((name.to_string(), why));
                    }
                }
            }
        }

        println!("\n=== transpiled OK: {ok}   blocked: {} ===", blocked.len());
        for (name, why) in &blocked {
            println!("  BLOCKED {name}: {why}");
        }
        println!(
            "\n=== sample LD flag JSON (first non-trivial) ===\n{}",
            first_json.unwrap_or_else(|| "<none had rules>".into())
        );

        assert!(ok > 0, "at least some flags should transpile");
    }
}
