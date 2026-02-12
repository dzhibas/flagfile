use flagfile_lib::parse_flagfile::FlagDefinition;

use super::LintWarning;

pub fn check(name: &str, def: &FlagDefinition) -> Vec<LintWarning> {
    let mut warnings = Vec::new();
    let has_lifecycle_metadata = def.metadata.deprecated.is_some()
        || def.metadata.expires.is_some()
        || def.metadata.flag_type.is_some();
    if has_lifecycle_metadata && def.metadata.owner.is_none() {
        warnings.push(LintWarning::warn(format!("{}: missing @owner", name)));
    }
    warnings
}
