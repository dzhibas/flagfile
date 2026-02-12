use flagfile_lib::parse_flagfile::FlagDefinition;

use super::LintWarning;

pub fn check(name: &str, def: &FlagDefinition) -> Vec<LintWarning> {
    let mut warnings = Vec::new();
    if def.metadata.deprecated.is_some() && def.metadata.expires.is_none() {
        warnings.push(LintWarning::warn(format!(
            "{}: @deprecated but no @expires set",
            name
        )));
    }
    warnings
}
