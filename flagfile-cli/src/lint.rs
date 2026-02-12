use std::collections::HashSet;
use std::io::{self, IsTerminal};
use std::process;

use chrono::Local;
use flagfile_lib::parse_flagfile::parse_flagfile_with_segments;

/// Inner lint logic that returns Ok(()) on success or Err(()) on failure.
/// Used by both the standalone `lint` command and the combined `check` command.
pub fn run_lint_inner(flagfile_path: &str) -> Result<(), ()> {
    let flagfile_content = match std::fs::read_to_string(flagfile_path) {
        Ok(content) => content,
        Err(_) => {
            eprintln!("{} does not exist", flagfile_path);
            return Err(());
        }
    };

    let (remainder, parsed) = match parse_flagfile_with_segments(&flagfile_content) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Parsing failed: {}", e);
            return Err(());
        }
    };

    if !remainder.trim().is_empty() {
        eprintln!(
            "Parsing failed: unexpected content near: {}",
            remainder.trim().lines().next().unwrap_or("")
        );
        return Err(());
    }

    let today = Local::now().date_naive();
    let use_color = io::stderr().is_terminal();
    let warn_icon = if use_color {
        "\x1b[33m\u{26a0}\x1b[0m"
    } else {
        "\u{26a0}"
    };
    let error_icon = if use_color {
        "\x1b[31m\u{26a0}\x1b[0m"
    } else {
        "\u{26a0}"
    };
    let mut warnings = 0;

    // Check for duplicate flag names
    let mut seen_flags = HashSet::new();
    for fv in &parsed.flags {
        for (name, _) in fv.iter() {
            if !seen_flags.insert(name) {
                eprintln!("{} {} is defined more than once", warn_icon, name);
                warnings += 1;
            }
        }
    }

    for fv in &parsed.flags {
        for (name, def) in fv.iter() {
            if let Some(ref msg) = def.metadata.deprecated {
                eprintln!("{} {} is deprecated: \"{}\"", warn_icon, name, msg);
                warnings += 1;
            }
            if let Some(expires) = def.metadata.expires {
                if expires < today {
                    let days_ago = (today - expires).num_days();
                    eprintln!(
                        "{} {} expired {} ({} days ago). Run: ff find -s {}",
                        error_icon, name, expires, days_ago, name
                    );
                    warnings += 1;
                }
            }
            let has_lifecycle_metadata = def.metadata.deprecated.is_some()
                || def.metadata.expires.is_some()
                || def.metadata.flag_type.is_some();
            if has_lifecycle_metadata && def.metadata.owner.is_none() {
                eprintln!("{} {}: missing @owner", warn_icon, name);
                warnings += 1;
            }
            if def.metadata.flag_type.as_deref() == Some("experiment")
                && def.metadata.expires.is_none()
            {
                eprintln!(
                    "{} {}: type=experiment but no @expires set",
                    warn_icon, name
                );
                warnings += 1;
            }
        }
    }

    if warnings == 0 {
        println!("{} ok, no warnings", flagfile_path);
        Ok(())
    } else {
        eprintln!();
        eprintln!("{} warnings found", warnings);
        Err(())
    }
}

/// Standalone lint command entry point. Calls `run_lint_inner` and exits on failure.
pub fn run_lint(flagfile_path: &str) {
    if run_lint_inner(flagfile_path).is_err() {
        process::exit(1);
    }
}
