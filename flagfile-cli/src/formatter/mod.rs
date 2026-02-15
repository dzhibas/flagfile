/// Flagfile formatter — `ff fmt` command.
///
/// Formats Flagfile source text with consistent indentation, operator spacing,
/// boolean casing, and blank-line handling while preserving all comments.
mod classify;
mod format;
mod normalize;

use std::io::IsTerminal;
use std::process;

use flagfile_lib::parse_flagfile::parse_flagfile_with_segments;

pub use format::format_flagfile;

/// Public entry point for the `fmt` subcommand.
pub fn run_fmt(flagfile_path: &str, check: bool, diff: bool) {
    if run_fmt_inner(flagfile_path, check, diff).is_err() {
        process::exit(1);
    }
}

/// Inner logic returning `Result<(), ()>` for composability with `check`.
pub fn run_fmt_inner(flagfile_path: &str, check: bool, diff: bool) -> Result<(), ()> {
    let content = match std::fs::read_to_string(flagfile_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("{} does not exist", flagfile_path);
            return Err(());
        }
    };

    // Validate the file parses correctly before formatting
    match parse_flagfile_with_segments(&content) {
        Ok((remainder, _)) => {
            if !remainder.trim().is_empty() {
                let near = remainder.trim().lines().next().unwrap_or("");
                eprintln!(
                    "Parsing failed: unexpected content near: {}",
                    truncate(near, 60)
                );
                return Err(());
            }
        }
        Err(e) => {
            eprintln!("Parsing failed: {}", e);
            return Err(());
        }
    }

    let formatted = format_flagfile(&content);

    if check {
        if content == formatted {
            return Ok(());
        }
        let is_tty = std::io::stderr().is_terminal();
        if is_tty {
            eprintln!("\x1b[1;31mwould reformat:\x1b[0m {}", flagfile_path);
        } else {
            eprintln!("would reformat: {}", flagfile_path);
        }
        return Err(());
    }

    if diff {
        print_diff(&content, &formatted, flagfile_path);
        return Ok(());
    }

    // Write back if changed
    if content == formatted {
        println!("already formatted: {}", flagfile_path);
    } else {
        match std::fs::write(flagfile_path, &formatted) {
            Ok(_) => println!("formatted: {}", flagfile_path),
            Err(e) => {
                eprintln!("Failed to write {}: {}", flagfile_path, e);
                return Err(());
            }
        }
    }

    Ok(())
}

/// Print a simple unified-style diff between the original and formatted text.
fn print_diff(original: &str, formatted: &str, path: &str) {
    let orig_lines: Vec<&str> = original.lines().collect();
    let fmt_lines: Vec<&str> = formatted.lines().collect();

    if orig_lines == fmt_lines {
        println!("no changes: {}", path);
        return;
    }

    println!("--- {}", path);
    println!("+++ {}", path);

    let max = orig_lines.len().max(fmt_lines.len());
    let mut i = 0;
    while i < max {
        // Find a contiguous hunk of changes
        if i < orig_lines.len() && i < fmt_lines.len() && orig_lines[i] == fmt_lines[i] {
            i += 1;
            continue;
        }

        // Determine hunk boundaries — include some context
        let ctx = 2;
        let hunk_start = i.saturating_sub(ctx);

        // Find where this change run ends
        let mut j = i;
        while j < max {
            if j < orig_lines.len() && j < fmt_lines.len() && orig_lines[j] == fmt_lines[j] {
                // Check if the next few lines are also equal (end of hunk)
                let mut all_equal = true;
                for k in j..j + ctx {
                    if k < orig_lines.len() && k < fmt_lines.len() && orig_lines[k] != fmt_lines[k]
                    {
                        all_equal = false;
                        break;
                    }
                    if k >= orig_lines.len() || k >= fmt_lines.len() {
                        break;
                    }
                }
                if all_equal {
                    break;
                }
            }
            j += 1;
        }

        let hunk_end = (j + ctx).min(max);

        println!(
            "@@ -{},{} +{},{} @@",
            hunk_start + 1,
            (hunk_end).min(orig_lines.len()).saturating_sub(hunk_start),
            hunk_start + 1,
            (hunk_end).min(fmt_lines.len()).saturating_sub(hunk_start),
        );

        for k in hunk_start..hunk_end {
            let orig = orig_lines.get(k);
            let fmt = fmt_lines.get(k);
            match (orig, fmt) {
                (Some(o), Some(f)) if o == f => println!(" {}", o),
                (Some(o), Some(f)) => {
                    println!("-{}", o);
                    println!("+{}", f);
                }
                (Some(o), None) => println!("-{}", o),
                (None, Some(f)) => println!("+{}", f),
                (None, None) => {}
            }
        }

        i = hunk_end;
    }
}

/// Truncate a string for error display.
fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}
