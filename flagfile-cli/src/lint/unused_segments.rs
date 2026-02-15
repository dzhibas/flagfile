use std::collections::HashSet;

use flagfile_lib::ast::AstNode;
use flagfile_lib::parse_flagfile::{ParsedFlagfile, Rule};

use super::LintWarning;

pub fn check(parsed: &ParsedFlagfile) -> Vec<LintWarning> {
    let mut warnings = Vec::new();
    let mut used = HashSet::new();

    for fv in &parsed.flags {
        for (_, def) in fv.iter() {
            collect_refs_from_rules(&def.rules, &mut used);
        }
    }
    // Segments can reference other segments
    for expr in parsed.segments.values() {
        collect_refs(expr, &mut used);
    }

    for seg_name in parsed.segments.keys() {
        if !used.contains(seg_name) {
            warnings.push(LintWarning::warn(format!(
                "segment \"{}\" is defined but never used",
                seg_name
            )));
        }
    }
    warnings
}

fn collect_refs(node: &AstNode, out: &mut HashSet<String>) {
    match node {
        AstNode::Segment(name) => {
            out.insert(name.clone());
        }
        AstNode::Logic(lhs, _, rhs)
        | AstNode::Compare(lhs, _, rhs)
        | AstNode::Match(lhs, _, rhs)
        | AstNode::Array(lhs, _, rhs) => {
            collect_refs(lhs, out);
            collect_refs(rhs, out);
        }
        AstNode::Scope { expr, .. } => collect_refs(expr, out),
        AstNode::Function(_, inner) => collect_refs(inner, out),
        AstNode::Percentage { field, .. } => collect_refs(field, out),
        AstNode::Coalesce(nodes) => {
            for n in nodes {
                collect_refs(n, out);
            }
        }
        AstNode::NullCheck { variable, .. } => collect_refs(variable, out),
        AstNode::Void | AstNode::Variable(_) | AstNode::Constant(_) | AstNode::List(_) => {}
    }
}

fn collect_refs_from_rules(rules: &[Rule], out: &mut HashSet<String>) {
    for rule in rules {
        match rule {
            Rule::BoolExpressionValue(expr, _) => collect_refs(expr, out),
            Rule::EnvRule { rules, .. } => collect_refs_from_rules(rules, out),
            Rule::Value(_) => {}
        }
    }
}
