use std::collections::HashMap;
use std::sync::OnceLock;

use wasm_bindgen::prelude::wasm_bindgen;

pub mod ast;
pub mod eval;
pub mod parse;
pub mod parse_flagfile;

pub use eval::Context;
pub use parse_flagfile::{FlagReturn, Rule, TestAnnotation, extract_test_annotations};

static FLAGS: OnceLock<HashMap<String, Vec<Rule>>> = OnceLock::new();

/// Reads and parses a `Flagfile` from the current directory, storing the
/// result in global state for later use with [`ff`].
///
/// Panics if the file cannot be read or parsed.
#[cfg(not(target_arch = "wasm32"))]
pub fn init() {
    init_from_str(
        &std::fs::read_to_string("Flagfile")
            .expect("Could not read 'Flagfile' in current directory"),
    );
}

/// Parses flagfile content from a string and stores the result in global
/// state. Useful when the content is already in memory or in WASM contexts.
///
/// Panics if parsing fails.
pub fn init_from_str(content: &str) {
    let (remainder, flag_values) =
        parse_flagfile::parse_flagfile(content).expect("Failed to parse Flagfile");
    if !remainder.trim().is_empty() {
        panic!(
            "Flagfile parsing failed: unexpected content near: {}",
            remainder.trim().lines().next().unwrap_or("")
        );
    }
    let mut flags: HashMap<String, Vec<Rule>> = HashMap::new();
    for fv in flag_values {
        for (name, rules) in fv {
            flags.insert(name.to_string(), rules);
        }
    }
    FLAGS
        .set(flags)
        .expect("init() or init_from_str() was called more than once");
}

/// Evaluates a flag by name against the given context.
///
/// Returns `Some(FlagReturn)` if the flag exists and a rule matched,
/// or `None` if the flag was not found or no rule matched.
///
/// Panics if [`init`] or [`init_from_str`] has not been called.
pub fn ff(flag_name: &str, context: &Context) -> Option<FlagReturn> {
    let flags = FLAGS
        .get()
        .expect("flagfile_lib::init() must be called before ff()");
    let rules = flags.get(flag_name)?;
    evaluate_rules(rules, context)
}

fn evaluate_rules(rules: &[Rule], context: &Context) -> Option<FlagReturn> {
    for rule in rules {
        match rule {
            Rule::BoolExpressionValue(expr, return_val) => {
                if let Ok(true) = eval::eval(expr, context) {
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

#[wasm_bindgen]
pub fn parse_wasm(i: &str) -> String {
    let Ok((_i, tree)) = parse::parse(i) else {
        todo!()
    };
    let b = format!("{:?}", tree);
    b.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Atom;

    #[test]
    fn test_evaluate_rules_bool_on() {
        let content = "FF-test-flag -> true";
        let (_, fvs) = parse_flagfile::parse_flagfile(content).unwrap();
        let rules = &fvs[0]["FF-test-flag"];
        let ctx = Context::new();
        let result = evaluate_rules(rules, &ctx);
        assert!(matches!(result, Some(FlagReturn::OnOff(true))));
    }

    #[test]
    fn test_evaluate_rules_bool_off() {
        let content = "FF-disabled -> false";
        let (_, fvs) = parse_flagfile::parse_flagfile(content).unwrap();
        let rules = &fvs[0]["FF-disabled"];
        let ctx = Context::new();
        let result = evaluate_rules(rules, &ctx);
        assert!(matches!(result, Some(FlagReturn::OnOff(false))));
    }

    #[test]
    fn test_evaluate_rules_with_context() {
        let content = r#"FF-premium {
    plan == premium -> true
    false
}"#;
        let (_, fvs) = parse_flagfile::parse_flagfile(content).unwrap();
        let rules = &fvs[0]["FF-premium"];

        // matching context
        let ctx: Context =
            HashMap::from([("plan", Atom::String("premium".to_string()))]);
        assert!(matches!(
            evaluate_rules(rules, &ctx),
            Some(FlagReturn::OnOff(true))
        ));

        // non-matching context falls through to default
        let ctx: Context =
            HashMap::from([("plan", Atom::String("free".to_string()))]);
        assert!(matches!(
            evaluate_rules(rules, &ctx),
            Some(FlagReturn::OnOff(false))
        ));
    }

    #[test]
    fn test_evaluate_rules_json_return() {
        let content = r#"FF-config -> json({"timeout": 30})"#;
        let (_, fvs) = parse_flagfile::parse_flagfile(content).unwrap();
        let rules = &fvs[0]["FF-config"];
        let ctx = Context::new();
        let result = evaluate_rules(rules, &ctx);
        assert!(matches!(result, Some(FlagReturn::Json(_))));
    }

    #[test]
    fn test_evaluate_rules_integer_return() {
        let content = "FF-timeout -> 5000";
        let (_, fvs) = parse_flagfile::parse_flagfile(content).unwrap();
        let rules = &fvs[0]["FF-timeout"];
        let ctx = Context::new();
        assert!(matches!(
            evaluate_rules(rules, &ctx),
            Some(FlagReturn::Integer(5000))
        ));
    }

    #[test]
    fn test_evaluate_rules_string_return() {
        let content = r#"FF-level -> "debug""#;
        let (_, fvs) = parse_flagfile::parse_flagfile(content).unwrap();
        let rules = &fvs[0]["FF-level"];
        let ctx = Context::new();
        let result = evaluate_rules(rules, &ctx);
        assert!(
            matches!(result, Some(FlagReturn::Str(ref s)) if s == "debug")
        );
    }

    #[test]
    fn test_evaluate_rules_no_match() {
        let content = r#"FF-strict {
    plan == enterprise -> true
}"#;
        let (_, fvs) = parse_flagfile::parse_flagfile(content).unwrap();
        let rules = &fvs[0]["FF-strict"];
        let ctx: Context =
            HashMap::from([("plan", Atom::String("free".to_string()))]);
        assert!(evaluate_rules(rules, &ctx).is_none());
    }

    #[test]
    fn test_flag_return_into_bool() {
        let val: bool = FlagReturn::OnOff(true).into();
        assert!(val);
        let val: bool = FlagReturn::OnOff(false).into();
        assert!(!val);
    }

    #[test]
    #[should_panic(expected = "cannot convert non-boolean FlagReturn to bool")]
    fn test_flag_return_into_bool_panics_on_non_bool() {
        let _: bool = FlagReturn::Integer(42).into();
    }

    #[test]
    fn test_init_from_str_and_ff() {
        // This test can only run once per process due to OnceLock.
        // If other tests already called init, this will panic, so we
        // guard it.
        if FLAGS.get().is_some() {
            return;
        }
        let content = r#"FF-hello -> true
FF-api-timeout -> 5000
FF-gated {
    tier == premium -> true
    false
}"#;
        init_from_str(content);
        let ctx = Context::new();
        assert!(matches!(
            ff("FF-hello", &ctx),
            Some(FlagReturn::OnOff(true))
        ));
        assert!(matches!(
            ff("FF-api-timeout", &ctx),
            Some(FlagReturn::Integer(5000))
        ));
        assert!(ff("FF-nonexistent", &ctx).is_none());

        let ctx: Context =
            HashMap::from([("tier", Atom::String("premium".to_string()))]);
        assert!(matches!(
            ff("FF-gated", &ctx),
            Some(FlagReturn::OnOff(true))
        ));

        let ctx: Context =
            HashMap::from([("tier", Atom::String("free".to_string()))]);
        assert!(matches!(
            ff("FF-gated", &ctx),
            Some(FlagReturn::OnOff(false))
        ));
    }
}
