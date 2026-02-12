use flagfile_lib::ast::{AstNode, Atom};
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
            Rule::BoolExpressionValue(AstNode::Constant(Atom::Boolean(true)), _) => {
                warnings.push(LintWarning::warn(format!(
                    "{}: tautological condition (true -> ...) is always matched",
                    name
                )));
            }
            Rule::EnvRule { rules, .. } => check_rules(name, rules, warnings),
            _ => {}
        }
    }
}
