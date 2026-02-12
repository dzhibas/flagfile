use flagfile_lib::parse_flagfile::FlagDefinition;

use super::LintWarning;

pub fn check(name: &str, def: &FlagDefinition) -> Vec<LintWarning> {
    let mut warnings = Vec::new();
    if def.rules.is_empty() {
        warnings.push(LintWarning::warn(format!(
            "{}: flag has no rules (will always evaluate to None)",
            name
        )));
    }
    warnings
}
