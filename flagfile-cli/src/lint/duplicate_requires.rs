use std::collections::HashSet;

use flagfile_lib::parse_flagfile::FlagDefinition;

use super::LintWarning;

pub fn check(name: &str, def: &FlagDefinition) -> Vec<LintWarning> {
    let mut warnings = Vec::new();
    let mut seen = HashSet::new();
    for req in &def.metadata.requires {
        if !seen.insert(req.as_str()) {
            warnings.push(LintWarning::warn(format!(
                "{}: duplicate @requires \"{}\"",
                name, req
            )));
        }
    }
    warnings
}
