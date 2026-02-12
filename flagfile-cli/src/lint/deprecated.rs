use flagfile_lib::parse_flagfile::FlagDefinition;

use super::LintWarning;

pub fn check(name: &str, def: &FlagDefinition) -> Vec<LintWarning> {
    let mut warnings = Vec::new();
    if let Some(ref msg) = def.metadata.deprecated {
        warnings.push(LintWarning::warn(format!(
            "{} is deprecated: \"{}\"",
            name, msg
        )));
    }
    warnings
}
