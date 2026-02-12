use std::collections::{HashMap, HashSet};

use flagfile_lib::ast::AstNode;
use flagfile_lib::parse_flagfile::ParsedFlagfile;

use super::LintWarning;

pub fn check(parsed: &ParsedFlagfile) -> Vec<LintWarning> {
    let mut warnings = Vec::new();

    // Build segment → segment dependency graph
    let mut deps: HashMap<&str, HashSet<String>> = HashMap::new();
    for (name, expr) in &parsed.segments {
        let mut refs = HashSet::new();
        collect_segment_refs(expr, &mut refs);
        deps.insert(name.as_str(), refs);
    }

    let mut visited = HashSet::new();
    for seg_name in deps.keys() {
        if !visited.contains(*seg_name) {
            let mut stack = HashSet::new();
            if let Some(cycle) = detect_cycle(seg_name, &deps, &mut visited, &mut stack) {
                warnings.push(LintWarning::error(format!(
                    "circular segment dependency: {}",
                    cycle
                )));
            }
        }
    }
    warnings
}

fn detect_cycle(
    seg: &str,
    deps: &HashMap<&str, HashSet<String>>,
    visited: &mut HashSet<String>,
    stack: &mut HashSet<String>,
) -> Option<String> {
    visited.insert(seg.to_string());
    stack.insert(seg.to_string());

    if let Some(refs) = deps.get(seg) {
        for dep in refs {
            if stack.contains(dep.as_str()) {
                return Some(format!("{} -> {}", seg, dep));
            }
            if !visited.contains(dep.as_str()) {
                if let Some(cycle) = detect_cycle(dep.as_str(), deps, visited, stack) {
                    return Some(format!("{} -> {}", seg, cycle));
                }
            }
        }
    }

    stack.remove(seg);
    None
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
