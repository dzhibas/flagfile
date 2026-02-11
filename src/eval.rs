use std::collections::HashMap;

use chrono::Local;

use regex::Regex;

use sha1::{Digest, Sha1};

use crate::ast::{ArrayOp, AstNode, Atom, ComparisonOp, FnCall, LogicOp, MatchOp};

pub type Segments = HashMap<String, AstNode>;

pub type Context<'a> = HashMap<&'a str, Atom>;

fn get_variable_value_from_context<'a>(
    variable: &'a AstNode,
    context: &'a Context,
) -> Option<Atom> {
    let res = match variable {
        AstNode::Variable(Atom::Variable(v)) => context.get(v.as_str()),
        AstNode::Constant(Atom::Variable(v)) => context.get(v.as_str()),
        AstNode::Function(op, v) => {
            match op {
                FnCall::Now => {
                    return Some(Atom::DateTime(Local::now().naive_local()));
                }
                _ => {
                    let value = get_variable_value_from_context(v, context);
                    if let Some(v) = value {
                        let vv = match op {
                            FnCall::Upper => Atom::String(v.to_string().to_uppercase()),
                            FnCall::Lower => Atom::String(v.to_string().to_lowercase()),
                            FnCall::Now => unreachable!(),
                        };
                        return Some(vv);
                    }
                }
            }
            None
        }
        AstNode::Coalesce(args) => {
            for arg in args {
                match arg {
                    AstNode::Variable(Atom::Variable(v)) => {
                        if let Some(val) = context.get(v.as_str()) {
                            return Some(val.clone());
                        }
                    }
                    AstNode::Constant(atom) => {
                        return Some(atom.clone());
                    }
                    _ => {}
                }
            }
            return None;
        }
        _ => None,
    };
    res.cloned()
}

pub fn eval_with_segments<'a>(
    expr: &AstNode,
    context: &Context,
    flag_name: Option<&str>,
    segments: &Segments,
) -> Result<bool, &'a str> {
    eval_impl(expr, context, flag_name, Some(segments))
}

pub fn eval<'a>(
    expr: &AstNode,
    context: &Context,
    flag_name: Option<&str>,
) -> Result<bool, &'a str> {
    eval_impl(expr, context, flag_name, None)
}

fn eval_impl<'a>(
    expr: &AstNode,
    context: &Context,
    flag_name: Option<&str>,
    segments: Option<&Segments>,
) -> Result<bool, &'a str> {
    let result = match expr {
        // true || false
        AstNode::Constant(var) => {
            let mut result = false;
            if let Atom::Boolean(v) = var {
                result = *v;
            }
            if let Atom::Variable(_v) = var {
                let context_val = get_variable_value_from_context(expr, context);
                if let Some(Atom::Boolean(inner)) = context_val {
                    result = inner;
                }
            }
            result
        }
        // a == 3
        // a < 3
        AstNode::Compare(var, op, val) => {
            let context_val = get_variable_value_from_context(var, context);
            let val_content = match val.as_ref() {
                AstNode::Constant(a) => Some(a),
                _ => None,
            }
            .unwrap();

            if let Some(c_val) = &context_val {
                match op {
                    ComparisonOp::More => c_val > val_content,
                    ComparisonOp::MoreEq => c_val >= val_content,
                    ComparisonOp::Less => c_val < val_content,
                    ComparisonOp::LessEq => c_val <= val_content,
                    ComparisonOp::Eq => c_val == val_content,
                    ComparisonOp::NotEq => c_val != val_content,
                }
            } else {
                false
            }
        }
        // x in (1, 2, 3) OR "value" in variable
        AstNode::Array(left_expr, op, right_expr) => {
            // Case 1: variable in (literal_list)
            if let AstNode::List(vec_list) = right_expr.as_ref() {
                let var_value = get_variable_value_from_context(left_expr, context);
                if let Some(search_value) = &var_value {
                    match op {
                        ArrayOp::In => {
                            for i in vec_list.iter() {
                                if search_value == i {
                                    return Ok(true);
                                }
                            }
                        }
                        ArrayOp::NotIn => {
                            let found = vec_list.iter().any(|i| search_value == i);
                            return Ok(!found);
                        }
                    }
                }
                false
            }
            // Case 2: "literal" in variable (variable resolves to List in context)
            else {
                let search_value = match left_expr.as_ref() {
                    AstNode::Constant(atom) if !matches!(atom, Atom::Variable(_)) => {
                        Some(atom.clone())
                    }
                    _ => get_variable_value_from_context(left_expr, context),
                };
                let list_value = get_variable_value_from_context(right_expr, context);
                if let (Some(needle), Some(Atom::List(items))) = (&search_value, &list_value) {
                    match op {
                        ArrayOp::In => items.iter().any(|item| needle == item),
                        ArrayOp::NotIn => !items.iter().any(|item| needle == item),
                    }
                } else {
                    false
                }
            }
        }
        AstNode::Match(var, op, rhs) => {
            let context_val = get_variable_value_from_context(var, context);
            if let Some(c_val) = &context_val {
                let haystack = c_val.to_string();
                let rhs_atom = match rhs.as_ref() {
                    AstNode::Constant(a) => a,
                    _ => return Ok(false),
                };
                let needle = match rhs_atom {
                    Atom::Regex(pattern) => {
                        let re_matched = match Regex::new(pattern) {
                            Ok(re) => re.is_match(&haystack),
                            Err(_) => false,
                        };
                        return Ok(match op {
                            MatchOp::Contains => re_matched,
                            MatchOp::NotContains => !re_matched,
                            _ => false,
                        });
                    }
                    other => other.to_string(),
                };
                match op {
                    MatchOp::Contains => haystack.contains(&needle),
                    MatchOp::NotContains => !haystack.contains(&needle),
                    MatchOp::StartsWith => haystack.starts_with(&needle),
                    MatchOp::NotStartsWith => !haystack.starts_with(&needle),
                    MatchOp::EndsWith => haystack.ends_with(&needle),
                    MatchOp::NotEndsWith => !haystack.ends_with(&needle),
                }
            } else {
                false
            }
        }
        AstNode::Logic(expr1, op, expr2) => {
            let expr1_eval = eval_impl(expr1, context, flag_name, segments).unwrap();
            let expr2_eval = eval_impl(expr2, context, flag_name, segments).unwrap();
            match op {
                LogicOp::And => expr1_eval && expr2_eval,
                LogicOp::Or => expr1_eval || expr2_eval,
            }
        }
        AstNode::Scope { expr, negate } => {
            let res = eval_impl(expr, context, flag_name, segments).unwrap();
            match negate {
                true => !res,
                false => res,
            }
        }
        AstNode::Segment(name) => {
            if let Some(segs) = segments {
                if let Some(seg_expr) = segs.get(name.as_str()) {
                    eval_impl(seg_expr, context, flag_name, segments).unwrap_or(false)
                } else {
                    false
                }
            } else {
                false
            }
        }
        AstNode::Percentage { rate, field, salt } => {
            let bucket_key = get_variable_value_from_context(field, context);
            let bucket_key_str = match bucket_key {
                Some(v) => v.to_string(),
                None => return Ok(false),
            };

            let flag = flag_name.unwrap_or("unknown");

            let input = match salt {
                Some(s) => format!("{}.{}.{}", flag, s, bucket_key_str),
                None => format!("{}.{}", flag, bucket_key_str),
            };

            let mut hasher = Sha1::new();
            hasher.update(input.as_bytes());
            let hash = hasher.finalize();
            let hex = format!("{:x}", hash);

            let substr = &hex[..15];
            let value = u64::from_str_radix(substr, 16).unwrap_or(0);
            let bucket = value % 100_000;
            let threshold = (rate * 1000.0) as u64;
            bucket < threshold
        }
        _ => false,
    };
    Ok(result)
}

#[cfg(test)]
mod tests {
    use crate::{ast::Atom, parse::parse};

    use super::*;

    #[test]
    fn logic_test() {
        let (_i, expr) = parse("x=1 and y=2").unwrap();
        assert_eq!(
            true,
            eval(
                &expr,
                &HashMap::from([("x", Atom::Number(1)), ("y", Atom::Number(2))]),
                None,
            )
            .unwrap()
        );

        let (_i, expr) = parse("x=1 || y=2").unwrap();
        assert_eq!(
            true,
            eval(
                &expr,
                &HashMap::from([("x", Atom::Number(12)), ("y", Atom::Number(2))]),
                None,
            )
            .unwrap()
        );

        let (_i, expr) = parse("countryCode==LT && city='Palanga'").unwrap();
        assert_eq!(
            true,
            eval(
                &expr,
                &HashMap::from([
                    ("countryCode", Atom::String("LT".to_string())),
                    ("city", Atom::String("Palanga".to_string()))
                ]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn testing_function_calls() {
        let (_i, expr) = parse("lower(countryCode)==lt && upper(city)='PALANGA'").unwrap();
        assert_eq!(
            true,
            eval(
                &expr,
                &HashMap::from([
                    ("countryCode", Atom::String("LT".to_string())),
                    ("city", Atom::String("Palanga".to_string()))
                ]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn simple_scope_test() {
        let (_i, expr) = parse("!(country=LT)").unwrap();
        assert_eq!(
            false,
            eval(
                &expr,
                &HashMap::from([("country", Atom::String("LT".to_string()))]),
                None,
            )
            .unwrap()
        );

        // scope inside scope
        let (_i, expr) = parse("(not (country == Lithuania))").unwrap();
        assert_eq!(
            false,
            eval(
                &expr,
                &HashMap::from([("country", Atom::String("Lithuania".to_string()))]),
                None,
            )
            .unwrap()
        );

        let (_i, expr) = parse("((lower(country) == netherlands))").unwrap();
        assert_eq!(
            true,
            eval(
                &expr,
                &HashMap::from([("country", Atom::String("Netherlands".to_string()))]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn array_eval_test() {
        let context = HashMap::from([
            ("x", Atom::Number(10)),
            ("y", Atom::String("tree".to_string())),
        ]);
        let (_i, expr) = parse("y in ('one', 'two', 'tree')").unwrap();
        let res = eval(&expr, &context, None).unwrap();
        assert_eq!(res, true);

        assert_eq!(
            false,
            eval(
                &expr,
                &HashMap::from([("y", Atom::String("four".to_string())),]),
                None,
            )
            .unwrap()
        );

        assert_eq!(
            true,
            eval(
                &parse("y not in ('one','two','tree')").unwrap().1,
                &HashMap::from([("y", Atom::String("four".to_string())),]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn compare_variable_with_string_in_array_test() {
        assert_eq!(
            true,
            eval(
                &parse("y in (one,two,tree)").unwrap().1,
                &HashMap::from([("y", Atom::String("two".to_string())),]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_comparison_expr_eval() {
        let context = HashMap::from([
            ("a", Atom::Number(3)),
            ("b", Atom::String("demo".to_string())),
        ]);

        assert_eq!(
            eval(&parse("a < 4").unwrap().1, &context, None).unwrap(),
            true
        );
        assert_eq!(
            eval(&parse("a < 3.3").unwrap().1, &context, None).unwrap(),
            true
        );
        assert_eq!(
            eval(
                &parse("a > 3.15").unwrap().1,
                &HashMap::from([("a", Atom::Float(3.14))]),
                None,
            )
            .unwrap(),
            false
        );
        assert_eq!(
            eval(
                &parse("a < 3.1415").unwrap().1,
                &HashMap::from([("a", Atom::Float(3.0))]),
                None,
            )
            .unwrap(),
            true
        );
        assert_eq!(
            eval(&parse("a>4").unwrap().1, &context, None).unwrap(),
            false
        );
        assert_eq!(
            eval(&parse("a<=4").unwrap().1, &context, None).unwrap(),
            true
        );
        assert_eq!(
            eval(&parse("a>=3").unwrap().1, &context, None).unwrap(),
            true
        );
        assert_eq!(
            eval(&parse("a!=4").unwrap().1, &context, None).unwrap(),
            true
        );
        assert_eq!(
            eval(&parse("a==4").unwrap().1, &context, None).unwrap(),
            false
        );
        assert_eq!(
            eval(&parse("a==3").unwrap().1, &context, None).unwrap(),
            true
        );
    }
    #[test]
    fn test_compare_string_expr_eval() {
        assert_eq!(
            eval(
                &parse("car!='Tesla'").unwrap().1,
                &HashMap::from([("car", Atom::String("BMW".into()))]),
                None,
            )
            .unwrap(),
            true
        );
        assert_eq!(
            eval(
                &parse("car=='Tesla'").unwrap().1,
                &HashMap::from([("car", Atom::String("Tesla".into()))]),
                None,
            )
            .unwrap(),
            true
        );
    }

    #[test]
    fn simple_constant_eval_test() {
        assert_eq!(
            false,
            eval(&parse("false").unwrap().1, &HashMap::from([]), None).unwrap()
        );
        assert_eq!(
            true,
            eval(&parse("TRUE").unwrap().1, &HashMap::from([]), None).unwrap()
        );
    }

    #[test]
    fn testing_date_comparison_evaluation() {
        let (_i, expr) = parse("created > 2024-02-02 and created <= 2024-02-13").unwrap();
        assert_eq!(
            true,
            eval(
                &expr,
                &HashMap::from([("created", "2024-02-12".into())]),
                None
            )
            .unwrap()
        );

        assert_eq!(
            false,
            eval(
                &parse("created < 2024-02-02").unwrap().1,
                &HashMap::from([("created", "2024-02-02".into())]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_semver_comparison_eval() {
        // version > 5.3.42
        assert_eq!(
            true,
            eval(
                &parse("version > 5.3.42").unwrap().1,
                &HashMap::from([("version", Atom::Semver(6, 0, 0))]),
                None,
            )
            .unwrap()
        );
        assert_eq!(
            false,
            eval(
                &parse("version > 5.3.42").unwrap().1,
                &HashMap::from([("version", Atom::Semver(5, 3, 42))]),
                None,
            )
            .unwrap()
        );
        assert_eq!(
            true,
            eval(
                &parse("version > 5.3.42").unwrap().1,
                &HashMap::from([("version", Atom::Semver(5, 3, 43))]),
                None,
            )
            .unwrap()
        );

        // version < 4.32.0
        assert_eq!(
            true,
            eval(
                &parse("version < 4.32.0").unwrap().1,
                &HashMap::from([("version", Atom::Semver(4, 31, 9))]),
                None,
            )
            .unwrap()
        );
        assert_eq!(
            false,
            eval(
                &parse("version < 4.32.0").unwrap().1,
                &HashMap::from([("version", Atom::Semver(4, 32, 0))]),
                None,
            )
            .unwrap()
        );

        // equality
        assert_eq!(
            true,
            eval(
                &parse("version == 1.2.3").unwrap().1,
                &HashMap::from([("version", Atom::Semver(1, 2, 3))]),
                None,
            )
            .unwrap()
        );

        // >= and <=
        assert_eq!(
            true,
            eval(
                &parse("version >= 2.0.0").unwrap().1,
                &HashMap::from([("version", Atom::Semver(2, 0, 0))]),
                None,
            )
            .unwrap()
        );
        assert_eq!(
            true,
            eval(
                &parse("version <= 2.0.0").unwrap().1,
                &HashMap::from([("version", Atom::Semver(1, 9, 99))]),
                None,
            )
            .unwrap()
        );

        // 2-component version (Float) compared against 3-component Semver
        // 5.4 as Float should coerce to 5.4.0 for semver comparison
        assert_eq!(
            true,
            eval(
                &parse("version > 5.3.42").unwrap().1,
                &HashMap::from([("version", Atom::Float(5.4))]),
                None,
            )
            .unwrap()
        );
        assert_eq!(
            false,
            eval(
                &parse("version > 5.3.42").unwrap().1,
                &HashMap::from([("version", Atom::Float(5.3))]),
                None,
            )
            .unwrap()
        );
        assert_eq!(
            true,
            eval(
                &parse("version == 5.4.0").unwrap().1,
                &HashMap::from([("version", Atom::Float(5.4))]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn testing_datetime_comparison_evaluation() {
        use chrono::NaiveDateTime;

        let dt = NaiveDateTime::parse_from_str("2025-06-15T12:00:00", "%Y-%m-%dT%H:%M:%S").unwrap();

        // DateTime > DateTime
        let (_i, expr) = parse("ts > 2025-06-15T09:00:00Z").unwrap();
        assert_eq!(
            true,
            eval(
                &expr,
                &HashMap::from([("ts", Atom::DateTime(dt))]),
                None
            )
            .unwrap()
        );

        // DateTime < DateTime
        let (_i, expr) = parse("ts < 2025-06-15T18:00:00Z").unwrap();
        assert_eq!(
            true,
            eval(
                &expr,
                &HashMap::from([("ts", Atom::DateTime(dt))]),
                None
            )
            .unwrap()
        );

        // DateTime == DateTime
        let (_i, expr) = parse("ts == 2025-06-15T12:00:00Z").unwrap();
        assert_eq!(
            true,
            eval(
                &expr,
                &HashMap::from([("ts", Atom::DateTime(dt))]),
                None
            )
            .unwrap()
        );

        // DateTime range: now() > start and now() < end
        let (_i, expr) = parse("ts > 2025-06-15T09:00:00Z and ts < 2025-06-15T18:00:00Z").unwrap();
        assert_eq!(
            true,
            eval(
                &expr,
                &HashMap::from([("ts", Atom::DateTime(dt))]),
                None
            )
            .unwrap()
        );

        // DateTime outside range
        let late_dt = NaiveDateTime::parse_from_str("2025-06-15T20:00:00", "%Y-%m-%dT%H:%M:%S").unwrap();
        assert_eq!(
            false,
            eval(
                &expr,
                &HashMap::from([("ts", Atom::DateTime(late_dt))]),
                None
            )
            .unwrap()
        );
    }

    #[test]
    fn testing_now_returns_datetime() {
        // now() should return a DateTime, which is comparable to DateTime literals
        let (_i, expr) = parse("now() > 2020-01-01T00:00:00Z").unwrap();
        assert_eq!(
            true,
            eval(&expr, &HashMap::from([]), None).unwrap()
        );
    }

    #[test]
    fn testing_datetime_vs_date_comparison() {
        use chrono::NaiveDateTime;

        // DateTime compared with Date (Date treated as midnight)
        let dt = NaiveDateTime::parse_from_str("2025-06-15T12:00:00", "%Y-%m-%dT%H:%M:%S").unwrap();
        let (_i, expr) = parse("ts > 2025-06-15").unwrap();
        assert_eq!(
            true,
            eval(
                &expr,
                &HashMap::from([("ts", Atom::DateTime(dt))]),
                None
            )
            .unwrap()
        );

        // DateTime at midnight == Date
        let midnight = NaiveDateTime::parse_from_str("2025-06-15T00:00:00", "%Y-%m-%dT%H:%M:%S").unwrap();
        let (_i, expr) = parse("ts == 2025-06-15").unwrap();
        assert_eq!(
            true,
            eval(
                &expr,
                &HashMap::from([("ts", Atom::DateTime(midnight))]),
                None
            )
            .unwrap()
        );
    }

    #[test]
    fn test_match_contains() {
        assert_eq!(
            true,
            eval(
                &parse("name ~ Nik").unwrap().1,
                &HashMap::from([("name", Atom::String("Nikolajus".into()))]),
                None,
            )
            .unwrap()
        );
        assert_eq!(
            false,
            eval(
                &parse("name ~ Nik").unwrap().1,
                &HashMap::from([("name", Atom::String("John".into()))]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_match_not_contains() {
        assert_eq!(
            true,
            eval(
                &parse("name !~ Nik").unwrap().1,
                &HashMap::from([("name", Atom::String("John".into()))]),
                None,
            )
            .unwrap()
        );
        assert_eq!(
            false,
            eval(
                &parse("name !~ Nik").unwrap().1,
                &HashMap::from([("name", Atom::String("Nikolajus".into()))]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_match_regex() {
        assert_eq!(
            true,
            eval(
                &parse("name ~ /.*ola.*/").unwrap().1,
                &HashMap::from([("name", Atom::String("Nikolajus".into()))]),
                None,
            )
            .unwrap()
        );
        assert_eq!(
            false,
            eval(
                &parse("name ~ /.*ola.*/").unwrap().1,
                &HashMap::from([("name", Atom::String("John".into()))]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_match_not_regex() {
        assert_eq!(
            true,
            eval(
                &parse("name !~ /.*ola.*/").unwrap().1,
                &HashMap::from([("name", Atom::String("John".into()))]),
                None,
            )
            .unwrap()
        );
        assert_eq!(
            false,
            eval(
                &parse("name !~ /.*ola.*/").unwrap().1,
                &HashMap::from([("name", Atom::String("Nikolajus".into()))]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_match_missing_variable() {
        assert_eq!(
            false,
            eval(&parse("name ~ Nik").unwrap().1, &HashMap::from([]), None).unwrap()
        );
    }

    #[test]
    fn test_match_starts_with() {
        // startsWith: true when string starts with prefix
        assert_eq!(
            true,
            eval(
                &parse("path ^~ \"/admin\"").unwrap().1,
                &HashMap::from([("path", Atom::String("/admin/settings".into()))]),
                None,
            )
            .unwrap()
        );
        // startsWith: false when string does not start with prefix
        assert_eq!(
            false,
            eval(
                &parse("path ^~ \"/admin\"").unwrap().1,
                &HashMap::from([("path", Atom::String("/user/profile".into()))]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_match_ends_with() {
        // endsWith: true when string ends with suffix
        assert_eq!(
            true,
            eval(
                &parse("email ~$ \"@company.com\"").unwrap().1,
                &HashMap::from([("email", Atom::String("user@company.com".into()))]),
                None,
            )
            .unwrap()
        );
        // endsWith: false when string does not end with suffix
        assert_eq!(
            false,
            eval(
                &parse("email ~$ \"@company.com\"").unwrap().1,
                &HashMap::from([("email", Atom::String("user@other.com".into()))]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_match_not_starts_with() {
        // notStartsWith: true when string does not start with prefix
        assert_eq!(
            true,
            eval(
                &parse("name !^~ \"test\"").unwrap().1,
                &HashMap::from([("name", Atom::String("production".into()))]),
                None,
            )
            .unwrap()
        );
        // notStartsWith: false when string starts with prefix
        assert_eq!(
            false,
            eval(
                &parse("name !^~ \"test\"").unwrap().1,
                &HashMap::from([("name", Atom::String("testing123".into()))]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_match_not_ends_with() {
        // notEndsWith: true when string does not end with suffix
        assert_eq!(
            true,
            eval(
                &parse("name !~$ \".tmp\"").unwrap().1,
                &HashMap::from([("name", Atom::String("file.txt".into()))]),
                None,
            )
            .unwrap()
        );
        // notEndsWith: false when string ends with suffix
        assert_eq!(
            false,
            eval(
                &parse("name !~$ \".tmp\"").unwrap().1,
                &HashMap::from([("name", Atom::String("data.tmp".into()))]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_starts_ends_with_edge_empty_string() {
        // empty string startsWith empty string → true
        assert_eq!(
            true,
            eval(
                &parse("name ^~ \"\"").unwrap().1,
                &HashMap::from([("name", Atom::String("".into()))]),
                None,
            )
            .unwrap()
        );
        // empty string endsWith empty string → true
        assert_eq!(
            true,
            eval(
                &parse("name ~$ \"\"").unwrap().1,
                &HashMap::from([("name", Atom::String("".into()))]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_starts_ends_with_exact_match() {
        // exact match: string equals prefix entirely → true
        assert_eq!(
            true,
            eval(
                &parse("name ^~ \"hello\"").unwrap().1,
                &HashMap::from([("name", Atom::String("hello".into()))]),
                None,
            )
            .unwrap()
        );
        // exact match: string equals suffix entirely → true
        assert_eq!(
            true,
            eval(
                &parse("name ~$ \"hello\"").unwrap().1,
                &HashMap::from([("name", Atom::String("hello".into()))]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_starts_with_combined_with_logic() {
        // path ^~ "/api" and method == "GET"
        assert_eq!(
            true,
            eval(
                &parse("path ^~ \"/api\" and method == \"GET\"").unwrap().1,
                &HashMap::from([
                    ("path", Atom::String("/api/users".into())),
                    ("method", Atom::String("GET".into()))
                ]),
                None,
            )
            .unwrap()
        );
        assert_eq!(
            false,
            eval(
                &parse("path ^~ \"/api\" and method == \"GET\"").unwrap().1,
                &HashMap::from([
                    ("path", Atom::String("/home".into())),
                    ("method", Atom::String("GET".into()))
                ]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_starts_with_combined_with_function() {
        // lower(name) ^~ "admin"
        assert_eq!(
            true,
            eval(
                &parse("lower(name) ^~ \"admin\"").unwrap().1,
                &HashMap::from([("name", Atom::String("ADMIN_USER".into()))]),
                None,
            )
            .unwrap()
        );
        assert_eq!(
            false,
            eval(
                &parse("lower(name) ^~ \"admin\"").unwrap().1,
                &HashMap::from([("name", Atom::String("USER_ADMIN".into()))]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_starts_ends_with_missing_variable() {
        assert_eq!(
            false,
            eval(
                &parse("name ^~ \"test\"").unwrap().1,
                &HashMap::from([]),
                None
            )
            .unwrap()
        );
        assert_eq!(
            false,
            eval(
                &parse("name ~$ \"test\"").unwrap().1,
                &HashMap::from([]),
                None
            )
            .unwrap()
        );
    }

    #[test]
    fn testing_logical_expression() {
        assert_eq!(
            true,
            eval(
                &parse("a=b and (c=d or e=f)").unwrap().1,
                &HashMap::from([
                    ("a", "b".into()),
                    ("c", "non-exiting".into()),
                    ("e", "f".into())
                ]),
                None,
            )
            .unwrap()
        );
        assert_eq!(
            true,
            eval(
                &parse("a=b and (c=d or e=f)").unwrap().1,
                &HashMap::from([("a", "b".into()), ("c", "d".into()), ("e", "f-non".into())]),
                None,
            )
            .unwrap()
        );
        assert_eq!(
            true,
            eval(
                &parse("a=b and c=d or e=f").unwrap().1,
                &HashMap::from([
                    ("a", "non".into()),
                    ("c", "non-exiting".into()),
                    ("e", "f".into())
                ]),
                None,
            )
            .unwrap()
        );

        assert_eq!(
            true,
            eval(
                &parse("a=b and c=d or e=f").unwrap().1,
                &HashMap::from([("a", "non".into()), ("c", "non".into()), ("e", "f".into())]),
                None,
            )
            .unwrap()
        );
        assert_eq!(
            false,
            eval(
                &parse("a=b and c=d or e=f").unwrap().1,
                &HashMap::from([("a", "non".into()), ("c", "d".into()), ("e", "non".into())]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_coalesce_first_present() {
        assert_eq!(
            true,
            eval(
                &parse("coalesce(countryCode, region, \"unknown\") == \"NL\"").unwrap().1,
                &HashMap::from([
                    ("countryCode", Atom::String("NL".into())),
                    ("region", Atom::String("EU".into()))
                ]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_coalesce_first_missing_second_present() {
        assert_eq!(
            true,
            eval(
                &parse("coalesce(countryCode, region, \"unknown\") == \"EU\"").unwrap().1,
                &HashMap::from([("region", Atom::String("EU".into()))]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_coalesce_all_missing_falls_to_default() {
        assert_eq!(
            true,
            eval(
                &parse("coalesce(countryCode, region, \"unknown\") == \"unknown\"").unwrap().1,
                &HashMap::from([]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_coalesce_in_comparison() {
        // coalesce with number comparison
        assert_eq!(
            true,
            eval(
                &parse("coalesce(priority, \"low\") == \"high\"").unwrap().1,
                &HashMap::from([("priority", Atom::String("high".into()))]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_coalesce_parse() {
        let (i, _) = parse("coalesce(a, b, \"default\") == \"test\"").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_percentage_parse() {
        let (i, _) = parse("percentage(50%, userId)").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_percentage_with_salt_parse() {
        let (i, _) = parse("percentage(25%, orgId, experiment_1)").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_percentage_combined_with_logic() {
        let (i, _) = parse("percentage(50%, orgId) and plan == premium").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_percentage_deterministic() {
        let (_, expr) = parse("percentage(50%, userId)").unwrap();
        let ctx = HashMap::from([("userId", Atom::String("alice".into()))]);
        let r1 = eval(&expr, &ctx, Some("FF-test")).unwrap();
        let r2 = eval(&expr, &ctx, Some("FF-test")).unwrap();
        assert_eq!(r1, r2);
    }

    #[test]
    fn test_percentage_cross_language_vectors() {
        // These test vectors MUST produce identical results in TypeScript.
        // SHA-1 hex → first 15 chars → parse base-16 → mod 100000 = bucket → bucket < rate*1000

        let (_, expr50) = parse("percentage(50%, userId)").unwrap();

        // Vector 1: SHA-1("FF-test-rollout.user-123") = 60feafb1513ee86...
        // bucket = 436826052989546118 % 100000 = 46118 < 50000 → true
        let ctx1 = HashMap::from([("userId", Atom::String("user-123".into()))]);
        assert_eq!(eval(&expr50, &ctx1, Some("FF-test-rollout")).unwrap(), true);

        // Vector 2: SHA-1("FF-test-rollout.user-456") = 66438f4ed936777...
        // bucket = 460555686507669367 % 100000 = 69367 >= 50000 → false
        let ctx2 = HashMap::from([("userId", Atom::String("user-456".into()))]);
        assert_eq!(
            eval(&expr50, &ctx2, Some("FF-test-rollout")).unwrap(),
            false
        );

        // Vector 3: SHA-1("FF-new-checkout.user-789") = 57fc354f1e45f99...
        // bucket = 396250061834837913 % 100000 = 37913 < 50000 → true
        let (_, expr50_2) = parse("percentage(50%, userId)").unwrap();
        let ctx3 = HashMap::from([("userId", Atom::String("user-789".into()))]);
        assert_eq!(
            eval(&expr50_2, &ctx3, Some("FF-new-checkout")).unwrap(),
            true
        );

        // Vector 4: with salt: SHA-1("FF-test-rollout.exp1.alice") = 8f91f05372579e5...
        // bucket = 646582128764877285 % 100000 = 77285 >= 50000 → false
        let (_, expr_salt) = parse("percentage(50%, userId, exp1)").unwrap();
        let ctx4 = HashMap::from([("userId", Atom::String("alice".into()))]);
        assert_eq!(
            eval(&expr_salt, &ctx4, Some("FF-test-rollout")).unwrap(),
            false
        );

        // Vector 5: rate=0% → always false
        let (_, expr_zero) = parse("percentage(0%, userId)").unwrap();
        assert_eq!(
            eval(&expr_zero, &ctx1, Some("FF-test-rollout")).unwrap(),
            false
        );

        // Vector 6: rate=100% → always true
        let (_, expr_full) = parse("percentage(100%, userId)").unwrap();
        assert_eq!(
            eval(&expr_full, &ctx1, Some("FF-test-rollout")).unwrap(),
            true
        );

        // Vector 7: SHA-1("FF-test.alice") = 76706ecbaa75e55...
        // bucket = 533402694680272469 % 100000 = 72469 >= 50000 → false
        assert_eq!(
            eval(
                &expr50,
                &HashMap::from([("userId", Atom::String("alice".into()))]),
                Some("FF-test")
            )
            .unwrap(),
            false
        );
    }

    #[test]
    fn test_coalesce_two_args() {
        assert_eq!(
            true,
            eval(
                &parse("coalesce(x, \"fallback\") == \"fallback\"")
                    .unwrap()
                    .1,
                &HashMap::from([]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_coalesce_in_logic_expr() {
        assert_eq!(
            true,
            eval(
                &parse("coalesce(countryCode, \"unknown\") == \"NL\" and plan == \"premium\"")
                    .unwrap()
                    .1,
                &HashMap::from([
                    ("countryCode", Atom::String("NL".into())),
                    ("plan", Atom::String("premium".into()))
                ]),
                None,
            )
            .unwrap()
        );
    }

    // ── Reverse 'in' operator tests ─────────────────────────────────

    #[test]
    fn test_reverse_in_found() {
        assert_eq!(
            true,
            eval(
                &parse("\"admin\" in roles").unwrap().1,
                &HashMap::from([(
                    "roles",
                    Atom::List(vec![
                        Atom::String("viewer".into()),
                        Atom::String("editor".into()),
                        Atom::String("admin".into()),
                    ])
                )]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_reverse_in_not_found() {
        assert_eq!(
            false,
            eval(
                &parse("\"superadmin\" in roles").unwrap().1,
                &HashMap::from([(
                    "roles",
                    Atom::List(vec![
                        Atom::String("viewer".into()),
                        Atom::String("editor".into()),
                        Atom::String("admin".into()),
                    ])
                )]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_reverse_not_in() {
        assert_eq!(
            true,
            eval(
                &parse("\"superadmin\" not in roles").unwrap().1,
                &HashMap::from([(
                    "roles",
                    Atom::List(vec![
                        Atom::String("viewer".into()),
                        Atom::String("editor".into()),
                    ])
                )]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_reverse_not_in_found() {
        assert_eq!(
            false,
            eval(
                &parse("\"admin\" not in roles").unwrap().1,
                &HashMap::from([(
                    "roles",
                    Atom::List(vec![
                        Atom::String("admin".into()),
                        Atom::String("editor".into()),
                    ])
                )]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_reverse_in_combined_with_logic() {
        assert_eq!(
            true,
            eval(
                &parse("\"export-csv\" in entitlements or \"export-all\" in entitlements")
                    .unwrap()
                    .1,
                &HashMap::from([(
                    "entitlements",
                    Atom::List(vec![
                        Atom::String("export-csv".into()),
                        Atom::String("view".into()),
                    ])
                )]),
                None,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_reverse_in_missing_variable() {
        assert_eq!(
            false,
            eval(
                &parse("\"admin\" in roles").unwrap().1,
                &HashMap::from([]),
                None,
            )
            .unwrap()
        );
    }

    // ── Segment tests ─────────────────────────────────────────────

    #[test]
    fn test_segment_eval_true() {
        let seg_expr = parse("plan == premium").unwrap().1;
        let segments = HashMap::from([("premium_users".to_string(), seg_expr)]);
        let ctx = HashMap::from([("plan", Atom::String("premium".into()))]);
        assert_eq!(
            true,
            eval_with_segments(
                &parse("segment(premium_users)").unwrap().1,
                &ctx,
                None,
                &segments,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_segment_eval_false() {
        let seg_expr = parse("plan == premium").unwrap().1;
        let segments = HashMap::from([("premium_users".to_string(), seg_expr)]);
        let ctx = HashMap::from([("plan", Atom::String("free".into()))]);
        assert_eq!(
            false,
            eval_with_segments(
                &parse("segment(premium_users)").unwrap().1,
                &ctx,
                None,
                &segments,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_segment_missing_returns_false() {
        let segments = Segments::new();
        let ctx = HashMap::from([]);
        assert_eq!(
            false,
            eval_with_segments(
                &parse("segment(nonexistent)").unwrap().1,
                &ctx,
                None,
                &segments,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_segment_in_logic_expr() {
        let seg_expr = parse("country == US").unwrap().1;
        let segments = HashMap::from([("us_users".to_string(), seg_expr)]);
        let ctx = HashMap::from([
            ("country", Atom::String("US".into())),
            ("plan", Atom::String("premium".into())),
        ]);
        assert_eq!(
            true,
            eval_with_segments(
                &parse("segment(us_users) and plan == premium").unwrap().1,
                &ctx,
                None,
                &segments,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_segment_without_segments_returns_false() {
        // eval (without segments) should return false for segment() calls
        assert_eq!(
            false,
            eval(
                &parse("segment(anything)").unwrap().1,
                &HashMap::from([]),
                None,
            )
            .unwrap()
        );
    }
}
