use std::collections::HashMap;
use std::process;

use clap::{Parser, Subcommand};
use flagfile_lib::ast::Atom;
use flagfile_lib::eval::{eval, Context};
use flagfile_lib::parse_flagfile::{parse_flagfile, FlagReturn, Rule};

#[derive(Parser, Debug)]
#[command(name = "Flagfile")]
#[command(version = "1.0")]
#[command(about = "Feature flagging for developers", long_about = None)]
struct Args {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Init,     // creates empty file with demo flag
    List {
        /// Path to the Flagfile
        #[arg(short = 'f', long = "flagfile", default_value = "Flagfile")]
        flagfile: String,
    },
    Validate {
        /// Path to the Flagfile to validate
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
    },
}

/// Parse a test line like: FF-name(key=val,key=val) == EXPECTED
fn parse_test_line(line: &str) -> Option<(&str, Vec<(&str, &str)>, &str)> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    // Find the flag name (up to '(')
    let paren_open = line.find('(')?;
    let flag_name = &line[..paren_open];

    // Find closing paren
    let paren_close = line.find(')')?;
    let params_str = &line[paren_open + 1..paren_close];

    // Parse key=value pairs
    let pairs: Vec<(&str, &str)> = params_str
        .split(',')
        .filter_map(|pair| {
            let pair = pair.trim();
            let eq_pos = pair.find('=')?;
            Some((&pair[..eq_pos], &pair[eq_pos + 1..]))
        })
        .collect();

    // Find == and extract expected value
    let rest = &line[paren_close + 1..];
    let eq_pos = rest.find("==")?;
    let expected = rest[eq_pos + 2..].trim();

    Some((flag_name, pairs, expected))
}

/// Evaluate a flag against context, returning the matched FlagReturn
fn evaluate_flag(rules: &[Rule], context: &Context) -> Option<FlagReturn> {
    for rule in rules {
        match rule {
            Rule::BoolExpressionValue(expr, return_val) => {
                if let Ok(true) = eval(expr, context) {
                    return Some(return_val.clone());
                }
            }
            Rule::Value(return_val) => {
                return Some(return_val.clone());
            }
        }
    }
    None
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
    }
}

const INIT_FLAGFILE: &str = r#"// Simple on/off flag
FF-welcome-banner -> true

// Feature with rules based on context
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

fn run_list(flagfile_path: &str) {
    let flagfile_content = match std::fs::read_to_string(flagfile_path) {
        Ok(content) => content,
        Err(_) => {
            eprintln!("{} does not exist", flagfile_path);
            process::exit(1);
        }
    };

    let (remainder, flag_values) = match parse_flagfile(&flagfile_content) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Parsing failed: {}", e);
            process::exit(1);
        }
    };

    if !remainder.trim().is_empty() {
        eprintln!("Parsing failed: unexpected content near: {}",
            remainder.trim().lines().next().unwrap_or(""));
        process::exit(1);
    }

    for fv in &flag_values {
        for (name, _) in fv.iter() {
            println!("{}", name);
        }
    }
}

fn run_validate(flagfile_path: &str) {
    let flagfile_content = match std::fs::read_to_string(flagfile_path) {
        Ok(content) => content,
        Err(_) => {
            eprintln!("{} does not exist", flagfile_path);
            process::exit(1);
        }
    };

    let (remainder, flag_values) = match parse_flagfile(&flagfile_content) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Parsing failed: {}", e);
            process::exit(1);
        }
    };

    if !remainder.trim().is_empty() {
        eprintln!("Parsing failed: unexpected content near: {}",
            remainder.trim().lines().next().unwrap_or(""));
        process::exit(1);
    }

    let mut total_flags = 0;
    let mut total_rules = 0;
    for fv in &flag_values {
        for (name, rules) in fv.iter() {
            total_flags += 1;
            total_rules += rules.len();
            println!("  {} ({} rules)", name, rules.len());
        }
    }

    println!();
    println!("{} valid, {} flags, {} rules", flagfile_path, total_flags, total_rules);
}

fn run_tests(flagfile_path: &str, testfile_path: &str) {
    // 1. Read Flagfile
    let flagfile_content = match std::fs::read_to_string(flagfile_path) {
        Ok(content) => content,
        Err(_) => {
            eprintln!("{} does not exist", flagfile_path);
            process::exit(1);
        }
    };

    // 2. Parse Flagfile
    let (remainder, flag_values) = match parse_flagfile(&flagfile_content) {
        Ok(result) => result,
        Err(_) => {
            eprintln!("Flagfile parsing failed");
            process::exit(1);
        }
    };

    if !remainder.trim().is_empty() {
        eprintln!("Flagfile parsing failed");
        process::exit(1);
    }

    // Merge all FlagValue entries into a single map
    let mut flags: HashMap<&str, Vec<Rule>> = HashMap::new();
    for fv in &flag_values {
        for (name, rules) in fv.iter() {
            flags.insert(name, rules.clone());
        }
    }

    // 3. Read test file
    let tests_content = match std::fs::read_to_string(testfile_path) {
        Ok(content) => content,
        Err(_) => {
            eprintln!("{} does not exist", testfile_path);
            process::exit(1);
        }
    };

    // 4. Parse and run each test
    let mut passed = 0;
    let mut failed = 0;
    let mut total = 0;

    for line in tests_content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }

        let Some((flag_name, pairs, expected)) = parse_test_line(line) else {
            eprintln!("SKIP  Invalid test line: {}", line);
            continue;
        };

        total += 1;

        // Build context
        let context: Context = pairs
            .iter()
            .map(|(k, v)| (*k, Atom::from(*v)))
            .collect();

        // Look up flag
        let Some(rules) = flags.get(flag_name) else {
            println!("FAIL  {} - flag not found", line);
            failed += 1;
            continue;
        };

        // Evaluate
        let result = evaluate_flag(rules, &context);

        match result {
            Some(ref ret) if result_matches(ret, expected) => {
                println!("PASS  {}", line);
                passed += 1;
            }
            Some(_) => {
                println!("FAIL  {}", line);
                failed += 1;
            }
            None => {
                println!("FAIL  {} - no rule matched", line);
                failed += 1;
            }
        }
    }

    // 6. Summary
    println!();
    println!("{} passed, {} failed out of {} tests", passed, failed, total);

    if failed > 0 {
        process::exit(1);
    }
}

fn main() {
    let cli = Args::parse();
    match cli.cmd {
        Command::Init => run_init(),
        Command::List { flagfile } => run_list(&flagfile),
        Command::Validate { flagfile } => run_validate(&flagfile),
        Command::Test { flagfile, testfile } => run_tests(&flagfile, &testfile),
    }
}
