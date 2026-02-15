use flagfile_lib::parse_flagfile::FlagDefinition;

use super::LintWarning;

pub fn check(name: &str, def: &FlagDefinition) -> Vec<LintWarning> {
    let mut warnings = Vec::new();
    if def.metadata.flag_type.as_deref() == Some("experiment") && def.metadata.expires.is_none() {
        warnings.push(LintWarning::warn(format!(
            "{}: type=experiment but no @expires set",
            name
        )));
    }
    warnings
}
