mod lint;
mod serve;

use std::collections::HashMap;
use std::io::{self, BufRead, IsTerminal, Write};
use std::process;
use std::sync::Mutex;

use clap::{Parser, Subcommand};
use flagfile_lib::ast::{Atom, FlagMetadata};
use flagfile_lib::eval::{eval_with_segments, Context, Segments};
use flagfile_lib::parse_flagfile::{
    extract_test_annotations, parse_flagfile_with_segments, FlagReturn, Rule, TestAnnotation,
};
use ignore::WalkBuilder;
use regex::Regex;

#[derive(Parser, Debug)]
#[command(name = "Flagfile")]
#[command(version = "0.1.7")]
#[command(about = "Feature flagging for developers and devops", long_about = None)]
struct Args {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Init, // creates empty file with demo flag
    List {
        /// Path to the Flagfile
        #[arg(short = 'f', long = "flagfile", default_value = "Flagfile")]
        flagfile: String,

        /// Show flag descriptions
        #[arg(short = 'd', long = "description")]
        description: bool,
    },
    Validate {
        /// Path to the Flagfile to validate
        #[arg(short = 'f', long = "flagfile", default_value = "Flagfile")]
        flagfile: String,
    },
    /// Run validate, lint, and test together
    Check {
        /// Path to the Flagfile
        #[arg(short = 'f', long = "flagfile", default_value = "Flagfile")]
        flagfile: String,

        /// Path to the test file
        #[arg(short = 't', long = "testfile", default_value = "Flagfile.tests")]
        testfile: String,

        /// Environment to evaluate @env rules against
        #[arg(short = 'e', long = "env")]
        env: Option<String>,
    },
    Lint {
        /// Path to the Flagfile to lint
        #[arg(short = 'f', long = "flagfile", default_value = "Flagfile")]
        flagfile: String,
    },
    Test {
        /// Path to the Flagfile to check
        #[arg(short = 'f', long = "flagfile", default_value = "Flagfile")]
        flagfile: String,

        /// Path to the test file to check
        #[arg(short = 't', long = "testfile", default_value = "Flagfile.tests")]
        testfile: String,

        /// Environment to evaluate @env rules against
        #[arg(short = 'e', long = "env")]
        env: Option<String>,
    },
    Eval {
        /// Path to the Flagfile
        #[arg(short = 'f', long = "flagfile", default_value = "Flagfile")]
        flagfile: String,

        /// Environment to evaluate @env rules against
        #[arg(short = 'e', long = "env")]
        env: Option<String>,

        /// Flag name to evaluate (e.g. FF-my-feature)
        flag_name: String,

        /// Context key=value pairs (e.g. country=NL plan=premium)
        context: Vec<String>,
    },
    Find {
        /// Directory to search in
        #[arg(default_value = ".")]
        path: String,

        /// Search term to filter flag names (case-insensitive substring match)
        #[arg(short = 's', long = "search")]
        search: Option<String>,
    },
    Serve {
        /// Path to the Flagfile
        #[arg(short = 'f', long = "flagfile")]
        flagfile: Option<String>,

        /// Port to listen on
        #[arg(short = 'p', long = "port")]
        port: Option<u16>,

        /// Path to config file
        #[arg(short = 'c', long = "config", default_value = "ff.toml")]
        config: String,

        /// Environment to evaluate @env rules against
        #[arg(short = 'e', long = "env")]
        env: Option<String>,
    },
}

/// Parse a test line like: FF-name(key=val,key=val) == EXPECTED
/// Also supports no-context form: FF-name == EXPECTED
type TestLine<'a> = (&'a str, Vec<(&'a str, &'a str)>, &'a str);

fn parse_test_line(line: &str) -> Option<TestLine<'_>> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    if let Some(paren_open) = line.find('(') {
        // Find the matching closing paren (skip parens inside brackets/quotes)
        let paren_close = find_matching_paren(line, paren_open)?;
        let flag_name = &line[..paren_open];
        let params_str = &line[paren_open + 1..paren_close];

        let pairs = split_context_params(params_str);

        let rest = &line[paren_close + 1..];
        let eq_pos = rest.find("==")?;
        let expected = rest[eq_pos + 2..].trim();

        Some((flag_name, pairs, expected))
    } else {
        // No-context form: FF-name == EXPECTED
        let eq_pos = line.find("==")?;
        let flag_name = line[..eq_pos].trim();
        let expected = line[eq_pos + 2..].trim();

        Some((flag_name, vec![], expected))
    }
}

/// Find the closing ')' that matches the '(' at `open_pos`, skipping brackets and quotes.
fn find_matching_paren(s: &str, open_pos: usize) -> Option<usize> {
    let mut depth = 0;
    let mut in_quote = false;
    let mut bracket_depth = 0;
    for (i, ch) in s[open_pos..].char_indices() {
        match ch {
            '"' if !in_quote => in_quote = true,
            '"' if in_quote => in_quote = false,
            '[' if !in_quote => bracket_depth += 1,
            ']' if !in_quote => bracket_depth -= 1,
            '(' if !in_quote && bracket_depth == 0 => depth += 1,
            ')' if !in_quote && bracket_depth == 0 => {
                depth -= 1;
                if depth == 0 {
                    return Some(open_pos + i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Split context params on commas, but skip commas inside brackets or quotes.
/// Returns Vec of (key, value) pairs.
fn split_context_params(s: &str) -> Vec<(&str, &str)> {
    let mut pairs = Vec::new();
    let mut bracket_depth = 0;
    let mut in_quote = false;
    let mut start = 0;

    for (i, ch) in s.char_indices() {
        match ch {
            '"' => in_quote = !in_quote,
            '[' if !in_quote => bracket_depth += 1,
            ']' if !in_quote => bracket_depth -= 1,
            ',' if !in_quote && bracket_depth == 0 => {
                if let Some(pair) = parse_kv_pair(&s[start..i]) {
                    pairs.push(pair);
                }
                start = i + 1;
            }
            _ => {}
        }
    }
    // Last segment
    if start < s.len() {
        if let Some(pair) = parse_kv_pair(&s[start..]) {
            pairs.push(pair);
        }
    }
    pairs
}

fn parse_kv_pair(s: &str) -> Option<(&str, &str)> {
    let s = s.trim();
    let eq_pos = s.find('=')?;
    Some((&s[..eq_pos], &s[eq_pos + 1..]))
}

/// Evaluate a flag's rules with an optional environment for @env rules
pub(crate) fn evaluate_rules_with_env(
    rules: &[Rule],
    context: &Context,
    flag_name: Option<&str>,
    segments: &Segments,
    env: Option<&str>,
) -> Option<FlagReturn> {
    for rule in rules {
        match rule {
            Rule::BoolExpressionValue(expr, return_val) => {
                if let Ok(true) = eval_with_segments(expr, context, flag_name, segments) {
                    return Some(return_val.clone());
                }
            }
            Rule::Value(return_val) => {
                return Some(return_val.clone());
            }
            Rule::EnvRule {
                env: rule_env,
                rules: sub_rules,
            } => {
                if env == Some(rule_env.as_str()) {
                    let result =
                        evaluate_rules_with_env(sub_rules, context, flag_name, segments, env);
                    if result.is_some() {
                        return result;
                    }
                }
            }
        }
    }
    None
}

/// Evaluate a flag checking @requires dependencies first.
/// If any required flag doesn't evaluate to true, returns None.
pub(crate) fn evaluate_flag_with_env(
    flag_name: &str,
    context: &Context,
    all_flags: &HashMap<&str, Vec<Rule>>,
    metadata: &HashMap<&str, FlagMetadata>,
    segments: &Segments,
    env: Option<&str>,
) -> Option<FlagReturn> {
    // Check @requires prerequisites
    if let Some(meta) = metadata.get(flag_name) {
        for req in &meta.requires {
            match all_flags.get(req.as_str()) {
                None => return None, // required flag doesn't exist
                Some(req_rules) => {
                    match evaluate_rules_with_env(req_rules, context, Some(req), segments, env) {
                        Some(FlagReturn::OnOff(true)) => {} // prerequisite satisfied
                        _ => return None,                   // prerequisite not met
                    }
                }
            }
        }
    }

    let rules = all_flags.get(flag_name)?;
    evaluate_rules_with_env(rules, context, Some(flag_name), segments, env)
}

/// Compare evaluation result with expected string
fn result_matches(result: &FlagReturn, expected: &str) -> bool {
    match result {
        FlagReturn::OnOff(val) => {
            let expected_upper = expected.to_uppercase();
            match expected_upper.as_str() {
                "TRUE" => *val,
                "FALSE" => !*val,
                _ => false,
            }
        }
        FlagReturn::Json(val) => {
            if let Ok(expected_json) = serde_json::from_str::<serde_json::Value>(expected) {
                *val == expected_json
            } else {
                false
            }
        }
        FlagReturn::Integer(val) => expected.parse::<i64>() == Ok(*val),
        FlagReturn::Str(val) => {
            // Strip surrounding quotes if present
            let expected_str = expected
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .unwrap_or(expected);
            val == expected_str
        }
    }
}

const INIT_FLAGFILE: &str = r#"// Simple on/off flag
FF-welcome-banner -> true

// Feature with rules based on context
// @test FF-premium-feature(plan=premium) == true
FF-premium-feature {
    // enable for users in premium plan
    plan == premium -> true
    // enable for beta testers
    beta == true -> true
    // disabled by default
    false
}

// Rollout by country
FF-new-checkout {
    country in (US, CA, GB) and platform == web -> true
    false
}
"#;

const INIT_TESTS: &str = r#"FF-premium-feature(plan=premium) == TRUE
FF-premium-feature(plan=free) == FALSE
FF-premium-feature(plan=free,beta=true) == TRUE
FF-new-checkout(country=US,platform=web) == TRUE
FF-new-checkout(country=US,platform=mobile) == FALSE
FF-new-checkout(country=DE,platform=web) == FALSE
"#;

fn run_init() {
    let flagfile_exists = std::path::Path::new("Flagfile").exists();
    let tests_exists = std::path::Path::new("Flagfile.tests").exists();

    if flagfile_exists || tests_exists {
        if flagfile_exists {
            eprintln!("Flagfile already exists in current folder");
        }
        if tests_exists {
            eprintln!("Flagfile.tests already exists in current folder");
        }
        process::exit(1);
    }

    std::fs::write("Flagfile", INIT_FLAGFILE).unwrap_or_else(|e| {
        eprintln!("Failed to create Flagfile: {}", e);
        process::exit(1);
    });

    std::fs::write("Flagfile.tests", INIT_TESTS).unwrap_or_else(|e| {
        eprintln!("Failed to create Flagfile.tests: {}", e);
        process::exit(1);
    });

    println!("Created Flagfile and Flagfile.tests");
}

fn run_list(flagfile_path: &str, show_description: bool) {
    let flagfile_content = match std::fs::read_to_string(flagfile_path) {
        Ok(content) => content,
        Err(_) => {
            eprintln!("{} does not exist", flagfile_path);
            process::exit(1);
        }
    };

    let (remainder, parsed) = match parse_flagfile_with_segments(&flagfile_content) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Parsing failed: {}", e);
            process::exit(1);
        }
    };

    if !remainder.trim().is_empty() {
        eprintln!(
            "Parsing failed: unexpected content near: {}",
            remainder.trim().lines().next().unwrap_or("")
        );
        process::exit(1);
    }

    for fv in &parsed.flags {
        for (name, def) in fv.iter() {
            if show_description {
                if let Some(ref desc) = def.metadata.description {
                    println!("{} ({})", name, desc);
                } else {
                    println!("{}", name);
                }
            } else {
                println!("{}", name);
            }
        }
    }
}

fn run_check(flagfile_path: &str, testfile_path: &str, env: Option<&str>) {
    let mut failed = false;

    println!("=== validate ===");
    if run_validate_inner(flagfile_path).is_err() {
        failed = true;
    }

    println!();
    println!("=== lint ===");
    if lint::run_lint_inner(flagfile_path).is_err() {
        failed = true;
    }

    println!();
    println!("=== test ===");
    if run_tests_inner(flagfile_path, testfile_path, env).is_err() {
        failed = true;
    }

    if failed {
        process::exit(1);
    }
}

/// Inner validate logic that returns Ok(()) on success or Err(()) on failure.
/// Used by both the standalone `validate` command and the combined `check` command.
fn run_validate_inner(flagfile_path: &str) -> Result<(), ()> {
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

    let mut total_flags = 0;
    let mut total_rules = 0;

    println!("Flags:");
    for fv in &parsed.flags {
        for (name, def) in fv.iter() {
            total_flags += 1;
            total_rules += def.rules.len();
            if let Some(ref desc) = def.metadata.description {
                println!("  {} ({} rules) - {}", name, def.rules.len(), desc);
            } else {
                println!("  {} ({} rules)", name, def.rules.len());
            }
        }
    }

    if !parsed.segments.is_empty() {
        println!();
        println!("Segments:");
        for name in parsed.segments.keys() {
            println!("  {}", name);
        }
    }

    println!();
    println!(
        "{} valid, {} flags, {} rules, {} segments",
        flagfile_path,
        total_flags,
        total_rules,
        parsed.segments.len()
    );
    Ok(())
}

fn run_validate(flagfile_path: &str) {
    if run_validate_inner(flagfile_path).is_err() {
        process::exit(1);
    }
}

/// Inner test logic that returns Ok(()) on success or Err(()) on failure.
/// Used by both the standalone `test` command and the combined `check` command.
fn run_tests_inner(flagfile_path: &str, testfile_path: &str, env: Option<&str>) -> Result<(), ()> {
    let use_color = io::stdout().is_terminal();
    let pass_label = if use_color {
        "\x1b[32mPASS\x1b[0m"
    } else {
        "PASS"
    };
    let fail_label = if use_color {
        "\x1b[31mFAIL\x1b[0m"
    } else {
        "FAIL"
    };

    // 1. Read Flagfile
    let flagfile_content = match std::fs::read_to_string(flagfile_path) {
        Ok(content) => content,
        Err(_) => {
            eprintln!("{} does not exist", flagfile_path);
            return Err(());
        }
    };

    // 2. Parse Flagfile
    let (remainder, parsed) = match parse_flagfile_with_segments(&flagfile_content) {
        Ok(result) => result,
        Err(_) => {
            eprintln!("Flagfile parsing failed");
            return Err(());
        }
    };

    if !remainder.trim().is_empty() {
        eprintln!("Flagfile parsing failed");
        return Err(());
    }

    // Merge all FlagValue entries into a single map and collect @test annotations from metadata
    let mut flags: HashMap<&str, Vec<Rule>> = HashMap::new();
    let mut metadata: HashMap<&str, FlagMetadata> = HashMap::new();
    let mut annotation_tests: Vec<TestAnnotation> = Vec::new();
    for fv in &parsed.flags {
        for (name, def) in fv.iter() {
            for test_assertion in &def.metadata.tests {
                annotation_tests.push(TestAnnotation {
                    assertion: test_assertion.clone(),
                    line_number: 0,
                });
            }
            flags.insert(name, def.rules.clone());
            metadata.insert(name, def.metadata.clone());
        }
    }
    let segments = &parsed.segments;

    // Extract inline @test annotations from comments
    let inline_tests = extract_test_annotations(&flagfile_content);

    // 3. Read test file (optional if inline or annotation tests exist)
    let tests_content = match std::fs::read_to_string(testfile_path) {
        Ok(content) => Some(content),
        Err(_) => {
            if inline_tests.is_empty() && annotation_tests.is_empty() {
                eprintln!("{} does not exist", testfile_path);
                return Err(());
            }
            None
        }
    };

    let mut passed = 0;
    let mut failed = 0;
    let mut total = 0;

    // 4. Run tests from test file
    if let Some(ref content) = tests_content {
        println!("--- {} ---", testfile_path);
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("//") {
                continue;
            }

            let Some((flag_name, pairs, expected)) = parse_test_line(line) else {
                eprintln!("SKIP  Invalid test line: {}", line);
                continue;
            };

            total += 1;

            let context: Context = pairs.iter().map(|(k, v)| (*k, Atom::from(*v))).collect();

            if !flags.contains_key(flag_name) {
                println!("{}  {} - flag not found", fail_label, line);
                failed += 1;
                continue;
            }

            let result =
                evaluate_flag_with_env(flag_name, &context, &flags, &metadata, segments, env);

            match result {
                Some(ref ret) if result_matches(ret, expected) => {
                    println!("{}  {}", pass_label, line);
                    passed += 1;
                }
                Some(_) => {
                    println!("{}  {}", fail_label, line);
                    failed += 1;
                }
                None => {
                    println!("{}  {} - no rule matched", fail_label, line);
                    failed += 1;
                }
            }
        }
    }

    // 5. Run inline @test annotations (from comments)
    if !inline_tests.is_empty() {
        if tests_content.is_some() {
            println!();
        }
        println!("--- inline @test ({}) ---", flagfile_path);

        for annotation in &inline_tests {
            let line = annotation.assertion.as_str();

            let Some((flag_name, pairs, expected)) = parse_test_line(line) else {
                eprintln!(
                    "SKIP  Invalid @test annotation: {} (line {})",
                    line, annotation.line_number
                );
                continue;
            };

            total += 1;

            let context: Context = pairs.iter().map(|(k, v)| (*k, Atom::from(*v))).collect();

            if !flags.contains_key(flag_name) {
                println!(
                    "{}  {} - flag not found (line {})",
                    fail_label, line, annotation.line_number
                );
                failed += 1;
                continue;
            }

            let result =
                evaluate_flag_with_env(flag_name, &context, &flags, &metadata, segments, env);

            match result {
                Some(ref ret) if result_matches(ret, expected) => {
                    println!("{}  {} (line {})", pass_label, line, annotation.line_number);
                    passed += 1;
                }
                Some(_) => {
                    println!("{}  {} (line {})", fail_label, line, annotation.line_number);
                    failed += 1;
                }
                None => {
                    println!(
                        "{}  {} - no rule matched (line {})",
                        fail_label, line, annotation.line_number
                    );
                    failed += 1;
                }
            }
        }
    }

    // 6. Run @test annotations from flag metadata
    if !annotation_tests.is_empty() {
        if tests_content.is_some() || !inline_tests.is_empty() {
            println!();
        }
        println!("--- @test annotations ({}) ---", flagfile_path);

        for annotation in &annotation_tests {
            let line = annotation.assertion.as_str();

            let Some((flag_name, pairs, expected)) = parse_test_line(line) else {
                eprintln!("SKIP  Invalid @test annotation: {}", line);
                continue;
            };

            total += 1;

            let context: Context = pairs.iter().map(|(k, v)| (*k, Atom::from(*v))).collect();

            if !flags.contains_key(flag_name) {
                println!("{}  {} - flag not found", fail_label, line);
                failed += 1;
                continue;
            }

            let result =
                evaluate_flag_with_env(flag_name, &context, &flags, &metadata, segments, env);

            match result {
                Some(ref ret) if result_matches(ret, expected) => {
                    println!("{}  {}", pass_label, line);
                    passed += 1;
                }
                Some(_) => {
                    println!("{}  {}", fail_label, line);
                    failed += 1;
                }
                None => {
                    println!("{}  {} - no rule matched", fail_label, line);
                    failed += 1;
                }
            }
        }
    }

    // 7. Summary
    println!();
    println!(
        "{} passed, {} failed out of {} tests",
        passed, failed, total
    );

    if failed > 0 {
        Err(())
    } else {
        Ok(())
    }
}

fn run_tests(flagfile_path: &str, testfile_path: &str, env: Option<&str>) {
    if run_tests_inner(flagfile_path, testfile_path, env).is_err() {
        process::exit(1);
    }
}

fn run_eval(flagfile_path: &str, flag_name: &str, context_args: &[String], env: Option<&str>) {
    let flagfile_content = match std::fs::read_to_string(flagfile_path) {
        Ok(content) => content,
        Err(_) => {
            eprintln!("{} does not exist", flagfile_path);
            process::exit(1);
        }
    };

    let (remainder, parsed) = match parse_flagfile_with_segments(&flagfile_content) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Parsing failed: {}", e);
            process::exit(1);
        }
    };

    if !remainder.trim().is_empty() {
        eprintln!(
            "Parsing failed: unexpected content near: {}",
            remainder.trim().lines().next().unwrap_or("")
        );
        process::exit(1);
    }

    let mut flags: HashMap<&str, Vec<Rule>> = HashMap::new();
    let mut metadata: HashMap<&str, FlagMetadata> = HashMap::new();
    for fv in &parsed.flags {
        for (name, def) in fv.iter() {
            flags.insert(name, def.rules.clone());
            metadata.insert(name, def.metadata.clone());
        }
    }

    if !flags.contains_key(flag_name) {
        eprintln!("Flag '{}' not found", flag_name);
        process::exit(1);
    }

    let context: Context = context_args
        .iter()
        .filter_map(|arg| {
            let eq_pos = arg.find('=')?;
            Some((arg[..eq_pos].as_ref(), Atom::from(&arg[eq_pos + 1..])))
        })
        .collect();

    match evaluate_flag_with_env(
        flag_name,
        &context,
        &flags,
        &metadata,
        &parsed.segments,
        env,
    ) {
        Some(FlagReturn::OnOff(val)) => println!("{}", val),
        Some(FlagReturn::Json(val)) => println!("{}", val),
        Some(FlagReturn::Integer(val)) => println!("{}", val),
        Some(FlagReturn::Str(val)) => println!("{}", val),
        None => {
            eprintln!("No rule matched for '{}'", flag_name);
            process::exit(1);
        }
    }
}

fn run_find(path: &str, search: Option<&str>) {
    let regex_pattern = match search {
        Some(term) if term.starts_with("FF-") || term.starts_with("FF_") => {
            format!(r"\b{}", regex::escape(term))
        }
        Some(term) => format!(
            r"\bFF[-_][a-zA-Z0-9_-]*{}[a-zA-Z0-9_-]*",
            regex::escape(term)
        ),
        None => r"\bFF[-_][a-zA-Z0-9_-]+".to_string(),
    };
    let pattern = Regex::new(&regex_pattern).unwrap();
    let use_color = io::stdout().is_terminal();
    let stdout = Mutex::new(io::stdout());

    WalkBuilder::new(path).build_parallel().run(|| {
        let pattern = pattern.clone();
        let stdout = &stdout;
        Box::new(move |entry| {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => return ignore::WalkState::Continue,
            };

            if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                return ignore::WalkState::Continue;
            }

            let path = entry.path();

            let file = match std::fs::File::open(path) {
                Ok(f) => f,
                Err(_) => return ignore::WalkState::Continue,
            };

            let reader = io::BufReader::new(file);
            let display_path = path.display();

            // Batch output per file to reduce lock contention
            let mut matches = Vec::new();
            for (line_idx, line) in reader.lines().enumerate() {
                let line = match line {
                    Ok(l) => l,
                    Err(_) => break, // binary file or encoding error
                };

                if pattern.is_match(&line) {
                    let colored_line = if use_color {
                        pattern.replace_all(&line, "\x1b[31m$0\x1b[0m").into_owned()
                    } else {
                        line
                    };
                    matches.push(format!(
                        "{}:{}:{}",
                        display_path,
                        line_idx + 1,
                        colored_line
                    ));
                }
            }

            if !matches.is_empty() {
                let mut out = stdout.lock().unwrap();
                for m in &matches {
                    let _ = writeln!(out, "{}", m);
                }
            }

            ignore::WalkState::Continue
        })
    });
}

#[tokio::main]
async fn main() {
    let cli = Args::parse();
    match cli.cmd {
        Command::Init => run_init(),
        Command::List {
            flagfile,
            description,
        } => run_list(&flagfile, description),
        Command::Validate { flagfile } => run_validate(&flagfile),
        Command::Check {
            flagfile,
            testfile,
            env,
        } => run_check(&flagfile, &testfile, env.as_deref()),
        Command::Lint { flagfile } => lint::run_lint(&flagfile),
        Command::Test {
            flagfile,
            testfile,
            env,
        } => run_tests(&flagfile, &testfile, env.as_deref()),
        Command::Eval {
            flagfile,
            env,
            flag_name,
            context,
        } => run_eval(&flagfile, &flag_name, &context, env.as_deref()),
        Command::Find { path, search } => run_find(&path, search.as_deref()),
        Command::Serve {
            flagfile,
            port,
            config,
            env,
        } => serve::run_serve(flagfile, port, &config, env).await,
    }
}
