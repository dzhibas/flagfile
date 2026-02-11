use std::collections::HashMap;
use std::sync::OnceLock;

use wasm_bindgen::prelude::wasm_bindgen;

pub mod ast;
pub mod eval;
pub mod parse;
pub mod parse_flagfile;

pub use ast::FlagMetadata;
pub use eval::{Context, Segments};
pub use parse_flagfile::{
    extract_test_annotations, FlagDefinition, FlagReturn, ParsedFlagfile, Rule, TestAnnotation,
};

static FLAGS: OnceLock<HashMap<String, Vec<Rule>>> = OnceLock::new();
static METADATA: OnceLock<HashMap<String, FlagMetadata>> = OnceLock::new();
static SEGMENTS: OnceLock<Segments> = OnceLock::new();
static ENVIRONMENT: OnceLock<Option<String>> = OnceLock::new();

/// Reads and parses a `Flagfile` from the current directory, storing the
/// result in global state for later use with [`ff`].
///
/// Panics if the file cannot be read or parsed.
#[cfg(not(target_arch = "wasm32"))]
pub fn init() {
    init_from_str(
        &std::fs::read_to_string("Flagfile")
            .expect("Could not read 'Flagfile' in current directory"),
    );
}

/// Parses flagfile content from a string and stores the result in global
/// state. Useful when the content is already in memory or in WASM contexts.
///
/// Panics if parsing fails.
pub fn init_from_str(content: &str) {
    init_from_str_inner(content, None);
}

/// Like [`init_from_str`] but also sets the current environment for
/// `@env` rule evaluation.
pub fn init_from_str_with_env(content: &str, env: &str) {
    init_from_str_inner(content, Some(env.to_string()));
}

/// Reads and parses a `Flagfile` with the given environment name.
#[cfg(not(target_arch = "wasm32"))]
pub fn init_with_env(env: &str) {
    init_from_str_with_env(
        &std::fs::read_to_string("Flagfile")
            .expect("Could not read 'Flagfile' in current directory"),
        env,
    );
}

fn init_from_str_inner(content: &str, env: Option<String>) {
    let (remainder, parsed) =
        parse_flagfile::parse_flagfile_with_segments(content).expect("Failed to parse Flagfile");
    if !remainder.trim().is_empty() {
        panic!(
            "Flagfile parsing failed: unexpected content near: {}",
            remainder.trim().lines().next().unwrap_or("")
        );
    }
    let mut flags: HashMap<String, Vec<Rule>> = HashMap::new();
    let mut metadata_map: HashMap<String, FlagMetadata> = HashMap::new();
    for fv in parsed.flags {
        for (name, def) in fv {
            flags.insert(name.to_string(), def.rules);
            metadata_map.insert(name.to_string(), def.metadata);
        }
    }
    FLAGS
        .set(flags)
        .expect("init() or init_from_str() was called more than once");
    METADATA
        .set(metadata_map)
        .expect("init() or init_from_str() was called more than once");
    SEGMENTS
        .set(parsed.segments)
        .expect("init() or init_from_str() was called more than once");
    ENVIRONMENT
        .set(env)
        .expect("init() or init_from_str() was called more than once");
}

/// Evaluates a flag by name against the given context.
///
/// Returns `Some(FlagReturn)` if the flag exists and a rule matched,
/// or `None` if the flag was not found or no rule matched.
///
/// Panics if [`init`] or [`init_from_str`] has not been called.
pub fn ff(flag_name: &str, context: &Context) -> Option<FlagReturn> {
    let flags = FLAGS
        .get()
        .expect("flagfile_lib::init() must be called before ff()");
    let segments = SEGMENTS
        .get()
        .expect("flagfile_lib::init() must be called before ff()");
    let metadata_map = METADATA
        .get()
        .expect("flagfile_lib::init() must be called before ff()");

    let current_env = ENVIRONMENT.get().and_then(|v| v.as_deref());

    // Check @requires prerequisites
    if let Some(meta) = metadata_map.get(flag_name) {
        for req in &meta.requires {
            match flags.get(req.as_str()) {
                None => return None, // required flag doesn't exist
                Some(req_rules) => {
                    match evaluate_rules(req_rules, context, Some(req), segments, current_env) {
                        Some(FlagReturn::OnOff(true)) => {} // prerequisite satisfied
                        _ => return None,                   // prerequisite not met
                    }
                }
            }
        }
    }

    let rules = flags.get(flag_name)?;
    evaluate_rules(rules, context, Some(flag_name), segments, current_env)
}

/// Returns the metadata annotations for a flag, if any.
///
/// Returns `None` if the flag was not found.
///
/// Panics if [`init`] or [`init_from_str`] has not been called.
pub fn ff_metadata(flag_name: &str) -> Option<FlagMetadata> {
    let metadata = METADATA
        .get()
        .expect("flagfile_lib::init() must be called before ff_metadata()");
    metadata.get(flag_name).cloned()
}

fn evaluate_rules(
    rules: &[Rule],
    context: &Context,
    flag_name: Option<&str>,
    segments: &Segments,
    env: Option<&str>,
) -> Option<FlagReturn> {
    for rule in rules {
        match rule {
            Rule::BoolExpressionValue(expr, return_val) => {
                if let Ok(true) = eval::eval_with_segments(expr, context, flag_name, segments) {
                    return Some(return_val.clone());
                }
            }
            Rule::Value(return_val) => {
                return Some(return_val.clone());
            }
            Rule::EnvRule {
                env: rule_env,
                rules: sub_rules,
            } => {
                if env == Some(rule_env.as_str()) {
                    let result = evaluate_rules(sub_rules, context, flag_name, segments, env);
                    if result.is_some() {
                        return result;
                    }
                }
            }
        }
    }
    None
}

#[wasm_bindgen]
pub fn parse_wasm(i: &str) -> String {
    let Ok((_i, tree)) = parse::parse(i) else {
        todo!()
    };
    let b = format!("{:?}", tree);
    b.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Atom;

    #[test]
    fn test_evaluate_rules_bool_on() {
        let content = "FF-test-flag -> true";
        let (_, fvs) = parse_flagfile::parse_flagfile(content).unwrap();
        let def = &fvs[0]["FF-test-flag"];
        let ctx = Context::new();
        let result = evaluate_rules(&def.rules, &ctx, None, &Segments::new(), None);
        assert!(matches!(result, Some(FlagReturn::OnOff(true))));
    }

    #[test]
    fn test_evaluate_rules_bool_off() {
        let content = "FF-disabled -> false";
        let (_, fvs) = parse_flagfile::parse_flagfile(content).unwrap();
        let def = &fvs[0]["FF-disabled"];
        let ctx = Context::new();
        let result = evaluate_rules(&def.rules, &ctx, None, &Segments::new(), None);
        assert!(matches!(result, Some(FlagReturn::OnOff(false))));
    }

    #[test]
    fn test_evaluate_rules_with_context() {
        let content = r#"FF-premium {
    plan == premium -> true
    false
}"#;
        let (_, fvs) = parse_flagfile::parse_flagfile(content).unwrap();
        let def = &fvs[0]["FF-premium"];

        // matching context
        let ctx: Context = HashMap::from([("plan", Atom::String("premium".to_string()))]);
        assert!(matches!(
            evaluate_rules(&def.rules, &ctx, None, &Segments::new(), None),
            Some(FlagReturn::OnOff(true))
        ));

        // non-matching context falls through to default
        let ctx: Context = HashMap::from([("plan", Atom::String("free".to_string()))]);
        assert!(matches!(
            evaluate_rules(&def.rules, &ctx, None, &Segments::new(), None),
            Some(FlagReturn::OnOff(false))
        ));
    }

    #[test]
    fn test_evaluate_rules_json_return() {
        let content = r#"FF-config -> json({"timeout": 30})"#;
        let (_, fvs) = parse_flagfile::parse_flagfile(content).unwrap();
        let def = &fvs[0]["FF-config"];
        let ctx = Context::new();
        let result = evaluate_rules(&def.rules, &ctx, None, &Segments::new(), None);
        assert!(matches!(result, Some(FlagReturn::Json(_))));
    }

    #[test]
    fn test_evaluate_rules_integer_return() {
        let content = "FF-timeout -> 5000";
        let (_, fvs) = parse_flagfile::parse_flagfile(content).unwrap();
        let def = &fvs[0]["FF-timeout"];
        let ctx = Context::new();
        assert!(matches!(
            evaluate_rules(&def.rules, &ctx, None, &Segments::new(), None),
            Some(FlagReturn::Integer(5000))
        ));
    }

    #[test]
    fn test_evaluate_rules_string_return() {
        let content = r#"FF-level -> "debug""#;
        let (_, fvs) = parse_flagfile::parse_flagfile(content).unwrap();
        let def = &fvs[0]["FF-level"];
        let ctx = Context::new();
        let result = evaluate_rules(&def.rules, &ctx, None, &Segments::new(), None);
        assert!(matches!(result, Some(FlagReturn::Str(ref s)) if s == "debug"));
    }

    #[test]
    fn test_evaluate_rules_no_match() {
        let content = r#"FF-strict {
    plan == enterprise -> true
}"#;
        let (_, fvs) = parse_flagfile::parse_flagfile(content).unwrap();
        let def = &fvs[0]["FF-strict"];
        let ctx: Context = HashMap::from([("plan", Atom::String("free".to_string()))]);
        assert!(evaluate_rules(&def.rules, &ctx, None, &Segments::new(), None).is_none());
    }

    #[test]
    fn test_flag_return_into_bool() {
        let val: bool = FlagReturn::OnOff(true).into();
        assert!(val);
        let val: bool = FlagReturn::OnOff(false).into();
        assert!(!val);
    }

    #[test]
    #[should_panic(expected = "cannot convert non-boolean FlagReturn to bool")]
    fn test_flag_return_into_bool_panics_on_non_bool() {
        let _: bool = FlagReturn::Integer(42).into();
    }

    #[test]
    fn test_env_rule_matching() {
        let content = r#"FF-debug {
    @env dev -> true
    @env prod -> false
}"#;
        let (_, fvs) = parse_flagfile::parse_flagfile(content).unwrap();
        let def = &fvs[0]["FF-debug"];
        let ctx = Context::new();

        // With env=dev, should return true
        assert!(matches!(
            evaluate_rules(&def.rules, &ctx, None, &Segments::new(), Some("dev")),
            Some(FlagReturn::OnOff(true))
        ));

        // With env=prod, should return false
        assert!(matches!(
            evaluate_rules(&def.rules, &ctx, None, &Segments::new(), Some("prod")),
            Some(FlagReturn::OnOff(false))
        ));

        // With env=stage (no match), should return None
        assert!(evaluate_rules(&def.rules, &ctx, None, &Segments::new(), Some("stage")).is_none());

        // With no env, should skip @env rules and return None
        assert!(evaluate_rules(&def.rules, &ctx, None, &Segments::new(), None).is_none());
    }

    #[test]
    fn test_env_rule_block_with_sub_rules() {
        let content = r#"FF-feature {
    @env prod {
        plan == premium -> true
        false
    }
    true
}"#;
        let (_, fvs) = parse_flagfile::parse_flagfile(content).unwrap();
        let def = &fvs[0]["FF-feature"];
        let ctx: Context = HashMap::from([("plan", Atom::String("premium".to_string()))]);

        // env=prod, plan=premium -> true (from sub-rule match)
        assert!(matches!(
            evaluate_rules(&def.rules, &ctx, None, &Segments::new(), Some("prod")),
            Some(FlagReturn::OnOff(true))
        ));

        // env=prod, plan=free -> false (from sub-rule default)
        let ctx: Context = HashMap::from([("plan", Atom::String("free".to_string()))]);
        assert!(matches!(
            evaluate_rules(&def.rules, &ctx, None, &Segments::new(), Some("prod")),
            Some(FlagReturn::OnOff(false))
        ));

        // env=dev -> skip @env prod, fall through to true
        assert!(matches!(
            evaluate_rules(&def.rules, &ctx, None, &Segments::new(), Some("dev")),
            Some(FlagReturn::OnOff(true))
        ));

        // No env -> skip @env rules, fall through to true
        assert!(matches!(
            evaluate_rules(&def.rules, &ctx, None, &Segments::new(), None),
            Some(FlagReturn::OnOff(true))
        ));
    }

    #[test]
    fn test_env_rule_multiple_envs() {
        let content = r#"FF-logging {
    @env dev -> true
    @env stage -> true
    @env prod -> false
}"#;
        let (_, fvs) = parse_flagfile::parse_flagfile(content).unwrap();
        let def = &fvs[0]["FF-logging"];
        let ctx = Context::new();

        assert!(matches!(
            evaluate_rules(&def.rules, &ctx, None, &Segments::new(), Some("dev")),
            Some(FlagReturn::OnOff(true))
        ));
        assert!(matches!(
            evaluate_rules(&def.rules, &ctx, None, &Segments::new(), Some("stage")),
            Some(FlagReturn::OnOff(true))
        ));
        assert!(matches!(
            evaluate_rules(&def.rules, &ctx, None, &Segments::new(), Some("prod")),
            Some(FlagReturn::OnOff(false))
        ));
    }

    #[test]
    fn test_init_from_str_and_ff() {
        // This test can only run once per process due to OnceLock.
        // If other tests already called init, this will panic, so we
        // guard it.
        if FLAGS.get().is_some() {
            return;
        }
        let content = r#"FF-hello -> true
FF-api-timeout -> 5000
FF-gated {
    tier == premium -> true
    false
}"#;
        init_from_str(content);
        let ctx = Context::new();
        assert!(matches!(
            ff("FF-hello", &ctx),
            Some(FlagReturn::OnOff(true))
        ));
        assert!(matches!(
            ff("FF-api-timeout", &ctx),
            Some(FlagReturn::Integer(5000))
        ));
        assert!(ff("FF-nonexistent", &ctx).is_none());

        let ctx: Context = HashMap::from([("tier", Atom::String("premium".to_string()))]);
        assert!(matches!(
            ff("FF-gated", &ctx),
            Some(FlagReturn::OnOff(true))
        ));

        let ctx: Context = HashMap::from([("tier", Atom::String("free".to_string()))]);
        assert!(matches!(
            ff("FF-gated", &ctx),
            Some(FlagReturn::OnOff(false))
        ));
    }
}
