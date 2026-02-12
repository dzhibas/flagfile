use std::collections::HashSet;

use flagfile_lib::parse_flagfile::{FlagDefinition, Rule};

use super::LintWarning;

pub fn check(name: &str, def: &FlagDefinition) -> Vec<LintWarning> {
    let mut warnings = Vec::new();
    let mut seen = HashSet::new();
    for rule in &def.rules {
        if let Rule::EnvRule { env, .. } = rule {
            if !seen.insert(env.as_str()) {
                warnings.push(LintWarning::warn(format!(
                    "{}: duplicate @env \"{}\" (only the first match is used)",
                    name, env
                )));
            }
        }
    }
    warnings
}
