use std::collections::HashSet;

use flagfile_lib::parse_flagfile::{FlagDefinition, FlagReturn, Rule};

use super::LintWarning;

pub fn check(name: &str, def: &FlagDefinition) -> Vec<LintWarning> {
    let mut warnings = Vec::new();
    let mut types = HashSet::new();
    collect_return_types(&def.rules, &mut types);
    if types.len() > 1 {
        let mut sorted: Vec<&str> = types.into_iter().collect();
        sorted.sort();
        warnings.push(LintWarning::warn(format!(
            "{}: mixed return types across rules: {}",
            name,
            sorted.join(", ")
        )));
    }
    warnings
}

fn return_type_name(ret: &FlagReturn) -> &'static str {
    match ret {
        FlagReturn::OnOff(_) => "boolean",
        FlagReturn::Integer(_) => "integer",
        FlagReturn::Str(_) => "string",
        FlagReturn::Json(_) => "json",
    }
}

fn collect_return_types(rules: &[Rule], out: &mut HashSet<&'static str>) {
    for rule in rules {
        match rule {
            Rule::Value(ret) | Rule::BoolExpressionValue(_, ret) => {
                out.insert(return_type_name(ret));
            }
            Rule::EnvRule { rules, .. } => collect_return_types(rules, out),
        }
    }
}
