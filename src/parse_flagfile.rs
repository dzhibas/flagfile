use std::collections::HashMap;

use nom::{
    branch::alt,
    bytes::complete::{is_not, tag, take_until},
    character::complete::alphanumeric1,
    combinator::{map, recognize, value},
    multi::{many0, many0_count, many1},
    sequence::{delimited, pair, preceded, terminated, tuple},
    IResult,
};
use serde_json::Value;

use crate::{
    ast::{AstNode, Atom},
    parse::{parse, parse_boolean, ws},
};

// Dependency
// Flagfile -> Vec<Feature> -> Feature -> Vec<Rule> -> Rule -> Expr -> Return

#[derive(Debug, Clone)]
pub enum FlagReturn {
    OnOff(bool),
    Json(Value),
    Integer(i64),
    Str(String),
}

impl From<FlagReturn> for bool {
    fn from(val: FlagReturn) -> Self {
        match val {
            FlagReturn::OnOff(b) => b,
            _ => panic!("cannot convert non-boolean FlagReturn to bool"),
        }
    }
}

impl From<FlagReturn> for String {
    fn from(val: FlagReturn) -> Self {
        match val {
            FlagReturn::OnOff(b) => b.to_string(),
            FlagReturn::Integer(n) => n.to_string(),
            FlagReturn::Str(s) => s,
            FlagReturn::Json(v) => v.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Rule {
    Value(FlagReturn),
    BoolExpressionValue(AstNode, FlagReturn),
}

pub type FlagValue<'a> = HashMap<&'a str, Vec<Rule>>;

#[derive(Debug, Clone, PartialEq)]
pub struct TestAnnotation {
    pub assertion: String,
    pub line_number: usize, // 1-based
}

/// Extract the assertion from a comment line containing `@test`.
/// Strips leading `*` decoration (from block comments) and whitespace.
fn extract_test_from_comment_line(line: &str) -> Option<String> {
    let trimmed = line.trim().trim_start_matches('*').trim();
    if let Some(rest) = trimmed.strip_prefix("@test ") {
        let assertion = rest.trim();
        if !assertion.is_empty() {
            return Some(assertion.to_string());
        }
    }
    None
}

/// Scan raw file content for `@test` annotations in both `//` and `/* */` comments.
/// Returns a list of test assertions with their 1-based line numbers.
pub fn extract_test_annotations(content: &str) -> Vec<TestAnnotation> {
    let mut results = Vec::new();
    let mut in_block_comment = false;

    for (idx, line) in content.lines().enumerate() {
        let line_number = idx + 1;

        if in_block_comment {
            if let Some(end_pos) = line.find("*/") {
                // Check the part of the line before `*/`
                let before_close = &line[..end_pos];
                if let Some(assertion) = extract_test_from_comment_line(before_close) {
                    results.push(TestAnnotation {
                        assertion,
                        line_number,
                    });
                }
                in_block_comment = false;
            } else if let Some(assertion) = extract_test_from_comment_line(line) {
                results.push(TestAnnotation {
                    assertion,
                    line_number,
                });
            }
            continue;
        }

        // Check for line comment
        if let Some(pos) = line.find("//") {
            let comment_body = &line[pos + 2..];
            if let Some(assertion) = extract_test_from_comment_line(comment_body) {
                results.push(TestAnnotation {
                    assertion,
                    line_number,
                });
            }
        }

        // Check for block comment opening
        if let Some(start_pos) = line.find("/*") {
            if let Some(end_pos) = line[start_pos + 2..].find("*/") {
                // Block comment opens and closes on the same line
                let block_body = &line[start_pos + 2..start_pos + 2 + end_pos];
                if let Some(assertion) = extract_test_from_comment_line(block_body) {
                    results.push(TestAnnotation {
                        assertion,
                        line_number,
                    });
                }
            } else {
                // Block comment opens but doesn't close on this line
                let after_open = &line[start_pos + 2..];
                if let Some(assertion) = extract_test_from_comment_line(after_open) {
                    results.push(TestAnnotation {
                        assertion,
                        line_number,
                    });
                }
                in_block_comment = true;
            }
        }
    }

    results
}

/// Parses and throws away: // comment EOL
fn parse_comment(i: &str) -> IResult<&str, ()> {
    value((), pair(ws(tag("//")), is_not("\n\r")))(i)
}

/// Parses and throws away: /* comment */
fn multiline_comment(i: &str) -> IResult<&str, ()> {
    value((), delimited(tag("/*"), take_until("*/"), tag("*/")))(i)
}

fn parse_json(i: &str) -> IResult<&str, FlagReturn> {
    let parser = delimited(ws(tag("json(")), take_until(")"), ws(tag(")")));
    map(parser, |v| {
        FlagReturn::Json(serde_json::from_str(v).unwrap())
    })(i)
}

fn parse_bool(i: &str) -> IResult<&str, FlagReturn> {
    map(parse_boolean, |v| match v {
        Atom::Boolean(v) => FlagReturn::OnOff(v),
        _ => unreachable!(),
    })(i)
}

fn parse_integer_return(i: &str) -> IResult<&str, FlagReturn> {
    let parser = nom::combinator::recognize(pair(
        nom::combinator::opt(tag("-")),
        nom::character::complete::digit1,
    ));
    map(parser, |num: &str| {
        FlagReturn::Integer(num.parse().unwrap())
    })(i)
}

fn parse_string_return(i: &str) -> IResult<&str, FlagReturn> {
    let parser_a = delimited(tag("\""), take_until("\""), tag("\""));
    let parser_b = delimited(tag("'"), take_until("'"), tag("'"));
    map(alt((parser_a, parser_b)), |s: &str| {
        FlagReturn::Str(s.to_string())
    })(i)
}

/// Opinionated feature flag name
/// it should always start with "FF-" < as this allows later auditing of the code and find all
/// flags
fn parse_flag_name(i: &str) -> IResult<&str, &str> {
    recognize(pair(
        alt((tag("FF-"), tag("FF_"))),
        many0_count(alt((alphanumeric1, tag("-"), tag("_")))),
    ))(i)
}

fn parse_return_val(i: &str) -> IResult<&str, FlagReturn> {
    alt((
        ws(parse_bool),
        ws(parse_json),
        ws(parse_string_return),
        ws(parse_integer_return),
    ))(i)
}

fn parse_anonymous_func(i: &str) -> IResult<&str, FlagValue<'_>> {
    let parser = tuple((ws(parse_flag_name), ws(tag("->")), parse_return_val));
    map(parser, |(n, _, v)| {
        HashMap::from([(n, vec![Rule::Value(v)])])
    })(i)
}

fn parse_rule_expr(i: &str) -> IResult<&str, Rule> {
    let parser = tuple((parse, ws(tag("->")), parse_return_val));
    map(parser, |(e, _, v)| Rule::BoolExpressionValue(e, v))(i)
}

fn parse_rule_static(i: &str) -> IResult<&str, Rule> {
    map(parse_return_val, Rule::Value)(i)
}

fn parse_rules(i: &str) -> IResult<&str, Rule> {
    alt((parse_rule_expr, parse_rule_static))(i)
}

fn parse_rules_or_comments(i: &str) -> IResult<&str, Rule> {
    terminated(
        preceded(many0(alt((parse_comment, multiline_comment))), parse_rules),
        many0(alt((parse_comment, multiline_comment))),
    )(i)
}

fn parse_rules_list(i: &str) -> IResult<&str, Vec<Rule>> {
    many1(parse_rules_or_comments)(i)
}

fn parse_function(i: &str) -> IResult<&str, FlagValue<'_>> {
    let parser = pair(
        ws(parse_flag_name),
        delimited(ws(tag("{")), parse_rules_list, ws(tag("}"))),
    );
    map(parser, |(flag_name, rules)| {
        HashMap::from([(flag_name, rules)])
    })(i)
}

pub fn parse_flagfile(i: &str) -> IResult<&str, Vec<FlagValue<'_>>> {
    let parser = preceded(
        many0(alt((parse_comment, multiline_comment))),
        alt((parse_anonymous_func, parse_function)),
    );
    let rest = terminated(parser, many0(alt((parse_comment, multiline_comment))));
    many0(rest)(i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rules() {
        let res = parse_rule_expr("countryCode == NL -> true");
        assert_eq!(true, res.is_ok());
        let res = parse_rule_static("\n     false\n");
        assert_eq!(true, res.is_ok());

        let data = r###"FF-feature-y {
    countryCode == NL -> true
    false
}"###;
        let (i, v) = parse_function(data).unwrap();
        assert_eq!(true, v.len() == 1);
        assert_eq!(i, "");
    }

    #[test]
    fn test_parse_rule_snake_case() {
        let data = r###"FF_feature_y {
        FALSE
}"###;
        let (i, v) = parse_function(data).unwrap();
        assert_eq!(true, v.len() == 1);
        assert_eq!(i, "");
    }

    #[test]
    fn test_parse_integer_return() {
        let data = "FF-api-timeout -> 5000";
        let (i, v) = parse_anonymous_func(data).unwrap();
        assert_eq!(i, "");
        let rules = v.get("FF-api-timeout").unwrap();
        assert_eq!(rules.len(), 1);
        assert!(matches!(&rules[0], Rule::Value(FlagReturn::Integer(5000))));
    }

    #[test]
    fn test_parse_string_return() {
        let data = r#"FF-log-level -> "debug""#;
        let (i, v) = parse_anonymous_func(data).unwrap();
        assert_eq!(i, "");
        let rules = v.get("FF-log-level").unwrap();
        assert_eq!(rules.len(), 1);
        assert!(matches!(&rules[0], Rule::Value(FlagReturn::Str(s)) if s == "debug"));
    }

    #[test]
    fn test_parse_integer_in_block() {
        let data = r#"FF-timeout {
    plan == premium -> 10000
    5000
}"#;
        let (i, v) = parse_function(data).unwrap();
        assert_eq!(i, "");
        let rules = v.get("FF-timeout").unwrap();
        assert_eq!(rules.len(), 2);
        assert!(matches!(&rules[1], Rule::Value(FlagReturn::Integer(5000))));
    }

    #[test]
    fn full_flag_file_test() {
        let data = include_str!("../Flagfile.example");

        let (i, v) = parse_flagfile(data).unwrap();
        dbg!(i, &v);
        assert_eq!(true, v.len() > 0);
        assert_eq!(i.to_string().trim(), "");
    }

    #[test]
    fn test_extract_test_single_line_comment() {
        let content = "// @test FF-foo(x=1) == true\nFF-foo -> true\n";
        let annotations = extract_test_annotations(content);
        assert_eq!(annotations.len(), 1);
        assert_eq!(annotations[0].assertion, "FF-foo(x=1) == true");
        assert_eq!(annotations[0].line_number, 1);
    }

    #[test]
    fn test_extract_test_block_comment() {
        let content = "/**\n * @test FF-bar == false\n */\nFF-bar -> false\n";
        let annotations = extract_test_annotations(content);
        assert_eq!(annotations.len(), 1);
        assert_eq!(annotations[0].assertion, "FF-bar == false");
        assert_eq!(annotations[0].line_number, 2);
    }

    #[test]
    fn test_extract_test_multiple_annotations() {
        let content = r#"// @test FF-a == true
FF-a -> true
/**
 * @test FF-b(x=1) == false
 */
FF-b -> false
// @test FF-c() == true
FF-c -> true
"#;
        let annotations = extract_test_annotations(content);
        assert_eq!(annotations.len(), 3);
        assert_eq!(annotations[0].assertion, "FF-a == true");
        assert_eq!(annotations[1].assertion, "FF-b(x=1) == false");
        assert_eq!(annotations[2].assertion, "FF-c() == true");
    }

    #[test]
    fn test_extract_test_no_annotations() {
        let content = "// just a comment\nFF-foo -> true\n/* no tests here */\n";
        let annotations = extract_test_annotations(content);
        assert_eq!(annotations.len(), 0);
    }

    #[test]
    fn test_extract_test_flagfile_example() {
        let data = include_str!("../Flagfile.example");
        let annotations = extract_test_annotations(data);
        assert_eq!(annotations.len(), 3);
        assert_eq!(
            annotations[0].assertion,
            "FF-feature-y(countryCode=nl) == true"
        );
        assert_eq!(annotations[0].line_number, 41);
        assert_eq!(
            annotations[1].assertion,
            "FF-flag-with-annotations-2 == false"
        );
        assert_eq!(annotations[1].line_number, 101);
        assert_eq!(annotations[2].assertion, "FF-timer-feature() == true");
        assert_eq!(annotations[2].line_number, 108);
    }
}

// feature-name
// -> arrow function
// { expr } function
// expr : Atom
// return bool
// return json(...)
// comment
// parse_flagfile -> opt(comment) feature func rules default return
