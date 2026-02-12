use flagfile_lib::ast::AstNode;
use flagfile_lib::parse_flagfile::{FlagDefinition, Rule};

use super::LintWarning;

pub fn check(name: &str, def: &FlagDefinition) -> Vec<LintWarning> {
    let mut warnings = Vec::new();
    check_rules(name, &def.rules, &mut warnings);
    warnings
}

fn check_rules(name: &str, rules: &[Rule], warnings: &mut Vec<LintWarning>) {
    for rule in rules {
        match rule {
            Rule::BoolExpressionValue(expr, _) => check_node(name, expr, warnings),
            Rule::EnvRule { rules, .. } => check_rules(name, rules, warnings),
            Rule::Value(_) => {}
        }
    }
}

fn check_node(name: &str, node: &AstNode, warnings: &mut Vec<LintWarning>) {
    match node {
        AstNode::Function(outer_fn, inner) => {
            if let AstNode::Function(inner_fn, _) = inner.as_ref() {
                if outer_fn == inner_fn {
                    let fn_name = format!("{:?}", outer_fn).to_lowercase();
                    warnings.push(LintWarning::warn(format!(
                        "{}: redundant nested {}({}(...))",
                        name, fn_name, fn_name
                    )));
                }
            }
            check_node(name, inner, warnings);
        }
        AstNode::Logic(lhs, _, rhs)
        | AstNode::Compare(lhs, _, rhs)
        | AstNode::Match(lhs, _, rhs)
        | AstNode::Array(lhs, _, rhs) => {
            check_node(name, lhs, warnings);
            check_node(name, rhs, warnings);
        }
        AstNode::Scope { expr, .. } => check_node(name, expr, warnings),
        AstNode::Percentage { field, .. } => check_node(name, field, warnings),
        AstNode::Coalesce(nodes) => {
            for n in nodes {
                check_node(name, n, warnings);
            }
        }
        AstNode::NullCheck { variable, .. } => check_node(name, variable, warnings),
        AstNode::Void
        | AstNode::Variable(_)
        | AstNode::Constant(_)
        | AstNode::List(_)
        | AstNode::Segment(_) => {}
    }
}
