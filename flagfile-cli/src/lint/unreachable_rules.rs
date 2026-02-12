use flagfile_lib::parse_flagfile::{FlagDefinition, Rule};

use super::LintWarning;

pub fn check(name: &str, def: &FlagDefinition) -> Vec<LintWarning> {
    let mut warnings = Vec::new();
    let unreachable = find_unreachable(&def.rules);
    if !unreachable.is_empty() {
        warnings.push(LintWarning::warn(format!(
            "{}: {} unreachable rule(s) after catch-all",
            name,
            unreachable.len()
        )));
    }
    warnings
}

fn find_unreachable(rules: &[Rule]) -> Vec<usize> {
    let mut unreachable = Vec::new();
    let mut found_catchall = false;
    for (i, rule) in rules.iter().enumerate() {
        if found_catchall {
            unreachable.push(i);
            continue;
        }
        if matches!(rule, Rule::Value(_)) {
            found_catchall = true;
        }
    }
    unreachable
}
