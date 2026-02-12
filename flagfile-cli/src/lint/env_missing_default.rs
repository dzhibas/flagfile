use flagfile_lib::parse_flagfile::{FlagDefinition, Rule};

use super::LintWarning;

pub fn check(name: &str, def: &FlagDefinition) -> Vec<LintWarning> {
    let mut warnings = Vec::new();
    let has_env_rule = def.rules.iter().any(|r| matches!(r, Rule::EnvRule { .. }));
    let has_fallback = matches!(def.rules.last(), Some(Rule::Value(_)));
    if has_env_rule && !has_fallback {
        warnings.push(LintWarning::warn(format!(
            "{}: has @env rules but no fallback for unlisted environments",
            name
        )));
    }
    warnings
}
