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
        AstNode::Percentage { rate, field, .. } => {
            if *rate < 0.0 || *rate > 100.0 {
                warnings.push(LintWarning::error(format!(
                    "{}: percentage rate {}% is out of valid range (0-100)",
                    name, rate
                )));
            }
            check_node(name, field, warnings);
        }
        AstNode::Logic(lhs, _, rhs)
        | AstNode::Compare(lhs, _, rhs)
        | AstNode::Match(lhs, _, rhs)
        | AstNode::Array(lhs, _, rhs) => {
            check_node(name, lhs, warnings);
            check_node(name, rhs, warnings);
        }
        AstNode::Scope { expr, .. } => check_node(name, expr, warnings),
        AstNode::Function(_, inner) => check_node(name, inner, warnings),
        AstNode::Coalesce(nodes) => {
            for n in nodes {
                check_node(name, n, warnings);
            }
        }
        AstNode::NullCheck { variable, .. } => check_node(name, variable, warnings),
        AstNode::Void | AstNode::Variable(_) | AstNode::Constant(_) | AstNode::List(_) => {}
        AstNode::Segment(_) => {}
    }
}
