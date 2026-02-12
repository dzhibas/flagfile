mod circular_deps;
mod circular_segments;
mod coalesce_constant_first;
mod deprecated;
mod deprecated_no_expiry;
mod duplicate_flags;
mod duplicate_requires;
mod empty_flag;
mod env_missing_default;
mod experiment_no_expiry;
mod expired;
mod missing_default;
mod missing_owner;
mod mixed_return_types;
mod percentage_range;
mod redundant_function;
mod shadowed_env_rules;
mod tautology;
mod undefined_requires;
mod undefined_segment;
mod unreachable_rules;
mod unused_segments;

use std::io::{self, IsTerminal};
use std::process;

use chrono::Local;
use flagfile_lib::parse_flagfile::parse_flagfile_with_segments;

#[derive(Debug)]
pub enum LintLevel {
    Warning,
    Error,
}

#[derive(Debug)]
pub struct LintWarning {
    pub level: LintLevel,
    pub message: String,
}

impl LintWarning {
    pub fn warn(message: impl Into<String>) -> Self {
        Self {
            level: LintLevel::Warning,
            message: message.into(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            level: LintLevel::Error,
            message: message.into(),
        }
    }
}

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

    let mut warnings: Vec<LintWarning> = Vec::new();

    // Global lints
    warnings.extend(duplicate_flags::check(&parsed));
    warnings.extend(circular_deps::check(&parsed));
    warnings.extend(circular_segments::check(&parsed));
    warnings.extend(unused_segments::check(&parsed));
    warnings.extend(undefined_requires::check(&parsed));
    warnings.extend(undefined_segment::check(&parsed));

    // Per-flag lints
    for fv in &parsed.flags {
        for (name, def) in fv.iter() {
            warnings.extend(deprecated::check(name, def));
            warnings.extend(expired::check(name, def, today));
            warnings.extend(missing_owner::check(name, def));
            warnings.extend(experiment_no_expiry::check(name, def));
            warnings.extend(deprecated_no_expiry::check(name, def));
            warnings.extend(unreachable_rules::check(name, def));
            warnings.extend(missing_default::check(name, def));
            warnings.extend(mixed_return_types::check(name, def));
            warnings.extend(empty_flag::check(name, def));
            warnings.extend(duplicate_requires::check(name, def));
            warnings.extend(percentage_range::check(name, def));
            warnings.extend(tautology::check(name, def));
            warnings.extend(coalesce_constant_first::check(name, def));
            warnings.extend(redundant_function::check(name, def));
            warnings.extend(env_missing_default::check(name, def));
            warnings.extend(shadowed_env_rules::check(name, def));
        }
    }

    if warnings.is_empty() {
        println!("{} ok, no warnings", flagfile_path);
        Ok(())
    } else {
        for w in &warnings {
            let icon = match w.level {
                LintLevel::Warning => warn_icon,
                LintLevel::Error => error_icon,
            };
            eprintln!("{} {}", icon, w.message);
        }
        eprintln!();
        eprintln!("{} warnings found", warnings.len());
        Err(())
    }
}

/// Standalone lint command entry point. Calls `run_lint_inner` and exits on failure.
pub fn run_lint(flagfile_path: &str) {
    if run_lint_inner(flagfile_path).is_err() {
        process::exit(1);
    }
}
