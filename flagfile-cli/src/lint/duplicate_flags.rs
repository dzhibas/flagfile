use std::collections::HashSet;

use flagfile_lib::parse_flagfile::ParsedFlagfile;

use super::LintWarning;

pub fn check(parsed: &ParsedFlagfile) -> Vec<LintWarning> {
    let mut warnings = Vec::new();
    let mut seen = HashSet::new();
    for fv in &parsed.flags {
        for (name, _) in fv.iter() {
            if !seen.insert(name) {
                warnings.push(LintWarning::warn(format!(
                    "{} is defined more than once",
                    name
                )));
            }
        }
    }
    warnings
}
