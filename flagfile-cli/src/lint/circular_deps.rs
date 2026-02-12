use std::collections::{HashMap, HashSet};

use flagfile_lib::parse_flagfile::ParsedFlagfile;

use super::LintWarning;

pub fn check(parsed: &ParsedFlagfile) -> Vec<LintWarning> {
    let mut warnings = Vec::new();
    let mut requires_map: HashMap<&str, &Vec<String>> = HashMap::new();
    for fv in &parsed.flags {
        for (name, def) in fv.iter() {
            if !def.metadata.requires.is_empty() {
                requires_map.insert(*name, &def.metadata.requires);
            }
        }
    }

    let mut visited = HashSet::new();
    for flag in requires_map.keys() {
        if !visited.contains(*flag) {
            let mut stack = HashSet::new();
            if let Some(cycle) = detect_cycle(flag, &requires_map, &mut visited, &mut stack) {
                warnings.push(LintWarning::error(format!(
                    "circular dependency: {}",
                    cycle
                )));
            }
        }
    }
    warnings
}

fn detect_cycle(
    flag: &str,
    requires_map: &HashMap<&str, &Vec<String>>,
    visited: &mut HashSet<String>,
    stack: &mut HashSet<String>,
) -> Option<String> {
    visited.insert(flag.to_string());
    stack.insert(flag.to_string());

    if let Some(deps) = requires_map.get(flag) {
        for dep in deps.iter() {
            if stack.contains(dep.as_str()) {
                return Some(format!("{} -> {}", flag, dep));
            }
            if !visited.contains(dep.as_str()) {
                if let Some(cycle) = detect_cycle(dep.as_str(), requires_map, visited, stack) {
                    return Some(format!("{} -> {}", flag, cycle));
                }
            }
        }
    }

    stack.remove(flag);
    None
}
