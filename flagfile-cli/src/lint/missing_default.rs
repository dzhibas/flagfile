use flagfile_lib::parse_flagfile::{FlagDefinition, Rule};

use super::LintWarning;

pub fn check(name: &str, def: &FlagDefinition) -> Vec<LintWarning> {
    let mut warnings = Vec::new();
    let has_conditional = def.rules.iter().any(|r| !matches!(r, Rule::Value(_)));
    if has_conditional && !matches!(def.rules.last(), Some(Rule::Value(_))) {
        warnings.push(LintWarning::error(format!(
            "{}: no default case (last rule is conditional)",
            name
        )));
    }
    warnings
}
