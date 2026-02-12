use std::collections::HashSet;

use flagfile_lib::ast::AstNode;
use flagfile_lib::parse_flagfile::{ParsedFlagfile, Rule};

use super::LintWarning;

pub fn check(parsed: &ParsedFlagfile) -> Vec<LintWarning> {
    let mut warnings = Vec::new();
    let mut used = HashSet::new();

    for fv in &parsed.flags {
        for (_, def) in fv.iter() {
            collect_segment_refs_from_rules(&def.rules, &mut used);
        }
    }
    // Segments can reference other segments
    for (_, expr) in &parsed.segments {
        collect_segment_refs(expr, &mut used);
    }

    let defined: HashSet<&str> = parsed.segments.keys().map(|s| s.as_str()).collect();

    for name in &used {
        if !defined.contains(name.as_str()) {
            warnings.push(LintWarning::error(format!(
                "segment \"{}\" is used but never defined",
                name
            )));
        }
    }
    warnings
}

fn collect_segment_refs(node: &AstNode, out: &mut HashSet<String>) {
    match node {
        AstNode::Segment(name) => {
            out.insert(name.clone());
        }
        AstNode::Logic(lhs, _, rhs)
        | AstNode::Compare(lhs, _, rhs)
        | AstNode::Match(lhs, _, rhs)
        | AstNode::Array(lhs, _, rhs) => {
            collect_segment_refs(lhs, out);
            collect_segment_refs(rhs, out);
        }
        AstNode::Scope { expr, .. } => collect_segment_refs(expr, out),
        AstNode::Function(_, inner) => collect_segment_refs(inner, out),
        AstNode::Percentage { field, .. } => collect_segment_refs(field, out),
        AstNode::Coalesce(nodes) => {
            for n in nodes {
                collect_segment_refs(n, out);
            }
        }
        AstNode::NullCheck { variable, .. } => collect_segment_refs(variable, out),
        AstNode::Void | AstNode::Variable(_) | AstNode::Constant(_) | AstNode::List(_) => {}
    }
}

fn collect_segment_refs_from_rules(rules: &[Rule], out: &mut HashSet<String>) {
    for rule in rules {
        match rule {
            Rule::BoolExpressionValue(expr, _) => collect_segment_refs(expr, out),
            Rule::EnvRule { rules, .. } => collect_segment_refs_from_rules(rules, out),
            Rule::Value(_) => {}
        }
    }
}
