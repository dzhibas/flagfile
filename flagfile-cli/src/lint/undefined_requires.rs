use std::collections::HashSet;

use flagfile_lib::parse_flagfile::ParsedFlagfile;

use super::LintWarning;

pub fn check(parsed: &ParsedFlagfile) -> Vec<LintWarning> {
    let mut warnings = Vec::new();

    let defined: HashSet<&str> = parsed
        .flags
        .iter()
        .flat_map(|fv| fv.keys().copied())
        .collect();

    for fv in &parsed.flags {
        for (name, def) in fv.iter() {
            for req in &def.metadata.requires {
                if !defined.contains(req.as_str()) {
                    warnings.push(LintWarning::error(format!(
                        "{}: @requires references undefined flag \"{}\"",
                        name, req
                    )));
                }
            }
        }
    }
    warnings
}
