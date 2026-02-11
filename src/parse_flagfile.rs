use std::collections::HashMap;

use chrono::NaiveDate;
use nom::{
    branch::alt,
    bytes::complete::{is_not, tag, take_till, take_until},
    character::complete::{alphanumeric1, multispace0},
    combinator::{map, recognize, value},
    multi::{many0, many0_count, many1},
    sequence::{delimited, pair, preceded, terminated, tuple},
    IResult,
};
use serde_json::Value;

use crate::{
    ast::{AstNode, Atom, FlagMetadata},
    eval::Segments,
    parse::{parse, parse_boolean, parse_segment_name, ws},
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
    EnvRule { env: String, rules: Vec<Rule> },
}

#[derive(Debug, Clone)]
pub struct FlagDefinition {
    pub rules: Vec<Rule>,
    pub metadata: FlagMetadata,
}

pub type FlagValue<'a> = HashMap<&'a str, FlagDefinition>;

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

// ── Annotation parsing ───────────────────────────────────────────

#[derive(Debug, Clone)]
enum Annotation {
    Owner(String),
    Expires(NaiveDate),
    Ticket(String),
    Description(String),
    FlagType(String),
    Deprecated(String),
    Requires(String),
    Test(String),
}

fn parse_quoted_string(i: &str) -> IResult<&str, &str> {
    alt((
        delimited(tag("\""), take_until("\""), tag("\"")),
        delimited(tag("'"), take_until("'"), tag("'")),
    ))(i)
}

fn parse_annotation_owner(i: &str) -> IResult<&str, Annotation> {
    let (rest, _) = ws(tag("@owner"))(i)?;
    let (rest, val) = ws(parse_quoted_string)(rest)?;
    Ok((rest, Annotation::Owner(val.to_string())))
}

fn parse_annotation_expires(i: &str) -> IResult<&str, Annotation> {
    let (rest, _) = ws(tag("@expires"))(i)?;
    let (rest, _) = multispace0(rest)?;
    let (rest, date_str) = recognize(tuple((
        nom::character::complete::digit1,
        tag("-"),
        nom::character::complete::digit1,
        tag("-"),
        nom::character::complete::digit1,
    )))(rest)?;
    let (rest, _) = multispace0(rest)?;
    let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .map_err(|_| nom::Err::Error(nom::error::Error::new(i, nom::error::ErrorKind::Tag)))?;
    Ok((rest, Annotation::Expires(date)))
}

fn parse_annotation_ticket(i: &str) -> IResult<&str, Annotation> {
    let (rest, _) = ws(tag("@ticket"))(i)?;
    let (rest, val) = ws(parse_quoted_string)(rest)?;
    Ok((rest, Annotation::Ticket(val.to_string())))
}

fn parse_annotation_description(i: &str) -> IResult<&str, Annotation> {
    let (rest, _) = ws(tag("@description"))(i)?;
    let (rest, val) = ws(parse_quoted_string)(rest)?;
    Ok((rest, Annotation::Description(val.to_string())))
}

fn parse_annotation_type(i: &str) -> IResult<&str, Annotation> {
    let (rest, _) = ws(tag("@type"))(i)?;
    let (rest, _) = multispace0(rest)?;
    let (rest, val) = recognize(many1(alt((alphanumeric1, tag("-"), tag("_")))))(rest)?;
    let (rest, _) = multispace0(rest)?;
    Ok((rest, Annotation::FlagType(val.to_string())))
}

fn parse_annotation_deprecated(i: &str) -> IResult<&str, Annotation> {
    let (rest, _) = ws(tag("@deprecated"))(i)?;
    let (rest, val) = ws(parse_quoted_string)(rest)?;
    Ok((rest, Annotation::Deprecated(val.to_string())))
}

fn parse_annotation_requires(i: &str) -> IResult<&str, Annotation> {
    let (rest, _) = ws(tag("@requires"))(i)?;
    let (rest, _) = multispace0(rest)?;
    let (rest, flag_name) = parse_flag_name(rest)?;
    let (rest, _) = multispace0(rest)?;
    Ok((rest, Annotation::Requires(flag_name.to_string())))
}

fn parse_annotation_test(i: &str) -> IResult<&str, Annotation> {
    let (rest, _) = ws(tag("@test"))(i)?;
    let (rest, _) = multispace0(rest)?;
    // Take the rest of the line as the assertion
    let (rest, assertion) = take_till(|c| c == '\n' || c == '\r')(rest)?;
    let assertion = assertion.trim();
    if assertion.is_empty() {
        return Err(nom::Err::Error(nom::error::Error::new(
            i,
            nom::error::ErrorKind::Tag,
        )));
    }
    Ok((rest, Annotation::Test(assertion.to_string())))
}

fn parse_annotation(i: &str) -> IResult<&str, Annotation> {
    alt((
        parse_annotation_owner,
        parse_annotation_expires,
        parse_annotation_ticket,
        parse_annotation_description,
        parse_annotation_type,
        parse_annotation_deprecated,
        parse_annotation_requires,
        parse_annotation_test,
    ))(i)
}

fn parse_metadata_block(i: &str) -> IResult<&str, FlagMetadata> {
    let (rest, annotations) = many0(preceded(
        many0(alt((parse_comment, multiline_comment))),
        parse_annotation,
    ))(i)?;
    let mut metadata = FlagMetadata::default();
    for ann in annotations {
        match ann {
            Annotation::Owner(v) => metadata.owner = Some(v),
            Annotation::Expires(v) => metadata.expires = Some(v),
            Annotation::Ticket(v) => metadata.ticket = Some(v),
            Annotation::Description(v) => metadata.description = Some(v),
            Annotation::FlagType(v) => metadata.flag_type = Some(v),
            Annotation::Deprecated(v) => metadata.deprecated = Some(v),
            Annotation::Requires(v) => metadata.requires.push(v),
            Annotation::Test(v) => metadata.tests.push(v),
        }
    }
    Ok((rest, metadata))
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
        HashMap::from([(n, FlagDefinition {
            rules: vec![Rule::Value(v)],
            metadata: FlagMetadata::default(),
        })])
    })(i)
}

fn parse_rule_expr(i: &str) -> IResult<&str, Rule> {
    let parser = tuple((parse, ws(tag("->")), parse_return_val));
    map(parser, |(e, _, v)| Rule::BoolExpressionValue(e, v))(i)
}

fn parse_rule_static(i: &str) -> IResult<&str, Rule> {
    map(parse_return_val, Rule::Value)(i)
}

fn parse_env_name(i: &str) -> IResult<&str, &str> {
    recognize(many1(alt((alphanumeric1, tag("-"), tag("_")))))(i)
}

fn parse_env_rule_simple(i: &str) -> IResult<&str, Rule> {
    let (rest, _) = ws(tag("@env"))(i)?;
    let (rest, _) = multispace0(rest)?;
    let (rest, env_name) = parse_env_name(rest)?;
    let (rest, _) = ws(tag("->"))(rest)?;
    let (rest, val) = parse_return_val(rest)?;
    Ok((rest, Rule::EnvRule {
        env: env_name.to_string(),
        rules: vec![Rule::Value(val)],
    }))
}

fn parse_env_rule_block(i: &str) -> IResult<&str, Rule> {
    let (rest, _) = ws(tag("@env"))(i)?;
    let (rest, _) = multispace0(rest)?;
    let (rest, env_name) = parse_env_name(rest)?;
    let (rest, rules) = delimited(ws(tag("{")), parse_rules_list, ws(tag("}")))(rest)?;
    Ok((rest, Rule::EnvRule {
        env: env_name.to_string(),
        rules,
    }))
}

fn parse_env_rule(i: &str) -> IResult<&str, Rule> {
    alt((parse_env_rule_block, parse_env_rule_simple))(i)
}

fn parse_rules(i: &str) -> IResult<&str, Rule> {
    alt((parse_env_rule, parse_rule_expr, parse_rule_static))(i)
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
        HashMap::from([(flag_name, FlagDefinition {
            rules,
            metadata: FlagMetadata::default(),
        })])
    })(i)
}

fn parse_flag_entry(i: &str) -> IResult<&str, FlagValue<'_>> {
    let (rest, _) = many0(alt((parse_comment, multiline_comment)))(i)?;
    let (rest, metadata) = parse_metadata_block(rest)?;
    let (rest, _) = many0(alt((parse_comment, multiline_comment)))(rest)?;
    let (rest, mut fv) = alt((parse_anonymous_func, parse_function))(rest)?;
    // Attach collected metadata to the flag definition
    if metadata != FlagMetadata::default() {
        for (_, def) in fv.iter_mut() {
            def.metadata = metadata.clone();
        }
    }
    let (rest, _) = many0(alt((parse_comment, multiline_comment)))(rest)?;
    Ok((rest, fv))
}

#[derive(Debug, Clone)]
pub struct ParsedFlagfile<'a> {
    pub flags: Vec<FlagValue<'a>>,
    pub segments: Segments,
}

fn parse_segment_definition(i: &str) -> IResult<&str, (String, AstNode)> {
    let (rest, _) = many0(alt((parse_comment, multiline_comment)))(i)?;
    let (rest, _) = ws(tag("@segment"))(rest)?;
    let (rest, name) = ws(parse_segment_name)(rest)?;
    let name = name.to_string();
    let (rest, expr) = delimited(ws(tag("{")), parse, ws(tag("}")))(rest)?;
    let (rest, _) = many0(alt((parse_comment, multiline_comment)))(rest)?;
    Ok((rest, (name, expr)))
}

enum FlagfileEntry<'a> {
    Flag(FlagValue<'a>),
    Segment(String, AstNode),
}

fn parse_flagfile_entry(i: &str) -> IResult<&str, FlagfileEntry<'_>> {
    alt((
        map(parse_segment_definition, |(name, expr)| {
            FlagfileEntry::Segment(name, expr)
        }),
        map(parse_flag_entry, FlagfileEntry::Flag),
    ))(i)
}

pub fn parse_flagfile_with_segments(i: &str) -> IResult<&str, ParsedFlagfile<'_>> {
    let (rest, entries) = many0(parse_flagfile_entry)(i)?;
    let mut flags = Vec::new();
    let mut segments = Segments::new();
    for entry in entries {
        match entry {
            FlagfileEntry::Flag(fv) => flags.push(fv),
            FlagfileEntry::Segment(name, expr) => {
                segments.insert(name, expr);
            }
        }
    }
    Ok((rest, ParsedFlagfile { flags, segments }))
}

pub fn parse_flagfile(i: &str) -> IResult<&str, Vec<FlagValue<'_>>> {
    many0(parse_flag_entry)(i)
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
        let def = v.get("FF-api-timeout").unwrap();
        assert_eq!(def.rules.len(), 1);
        assert!(matches!(&def.rules[0], Rule::Value(FlagReturn::Integer(5000))));
    }

    #[test]
    fn test_parse_string_return() {
        let data = r#"FF-log-level -> "debug""#;
        let (i, v) = parse_anonymous_func(data).unwrap();
        assert_eq!(i, "");
        let def = v.get("FF-log-level").unwrap();
        assert_eq!(def.rules.len(), 1);
        assert!(matches!(&def.rules[0], Rule::Value(FlagReturn::Str(s)) if s == "debug"));
    }

    #[test]
    fn test_parse_integer_in_block() {
        let data = r#"FF-timeout {
    plan == premium -> 10000
    5000
}"#;
        let (i, v) = parse_function(data).unwrap();
        assert_eq!(i, "");
        let def = v.get("FF-timeout").unwrap();
        assert_eq!(def.rules.len(), 2);
        assert!(matches!(&def.rules[1], Rule::Value(FlagReturn::Integer(5000))));
    }

    #[test]
    fn full_flag_file_test() {
        let data = include_str!("../Flagfile.example");

        let (i, v) = parse_flagfile_with_segments(data).unwrap();
        dbg!(i, &v);
        assert_eq!(true, v.flags.len() > 0);
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
        // Comment-based @test annotations only (standalone @test are parsed as metadata)
        assert_eq!(annotations.len() > 10, true);
    }
    #[test]
    fn test_parse_metadata_owner() {
        let data = r#"@owner "payments-team"
FF-pay -> true"#;
        let (i, v) = parse_flagfile(data).unwrap();
        assert_eq!(i.trim(), "");
        let def = v[0].get("FF-pay").unwrap();
        assert_eq!(def.metadata.owner, Some("payments-team".to_string()));
    }

    #[test]
    fn test_parse_metadata_expires() {
        let data = "@expires 2026-06-01\nFF-temp -> true";
        let (i, v) = parse_flagfile(data).unwrap();
        assert_eq!(i.trim(), "");
        let def = v[0].get("FF-temp").unwrap();
        assert_eq!(
            def.metadata.expires,
            Some(NaiveDate::from_ymd_opt(2026, 6, 1).unwrap())
        );
    }

    #[test]
    fn test_parse_metadata_multiple() {
        let data = r#"@owner "payments-team"
@expires 2026-06-01
@ticket "JIRA-1234"
@description "New 3DS2 auth flow"
@type release
FF-3ds2-auth {
    percentage(50%, userId) -> true
    false
}"#;
        let (i, v) = parse_flagfile(data).unwrap();
        assert_eq!(i.trim(), "");
        let def = v[0].get("FF-3ds2-auth").unwrap();
        assert_eq!(def.metadata.owner, Some("payments-team".to_string()));
        assert_eq!(
            def.metadata.expires,
            Some(NaiveDate::from_ymd_opt(2026, 6, 1).unwrap())
        );
        assert_eq!(def.metadata.ticket, Some("JIRA-1234".to_string()));
        assert_eq!(def.metadata.description, Some("New 3DS2 auth flow".to_string()));
        assert_eq!(def.metadata.flag_type, Some("release".to_string()));
        assert_eq!(def.rules.len(), 2);
    }

    #[test]
    fn test_parse_metadata_deprecated() {
        let data = r#"@deprecated "Use FF-new-checkout instead"
@expires 2026-04-01
FF-old-checkout -> true"#;
        let (i, v) = parse_flagfile(data).unwrap();
        assert_eq!(i.trim(), "");
        let def = v[0].get("FF-old-checkout").unwrap();
        assert_eq!(
            def.metadata.deprecated,
            Some("Use FF-new-checkout instead".to_string())
        );
        assert_eq!(
            def.metadata.expires,
            Some(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap())
        );
    }

    #[test]
    fn test_parse_no_metadata_backward_compat() {
        let data = "FF-simple -> true";
        let (i, v) = parse_flagfile(data).unwrap();
        assert_eq!(i.trim(), "");
        let def = v[0].get("FF-simple").unwrap();
        assert_eq!(def.metadata, FlagMetadata::default());
    }

    #[test]
    fn test_parse_mixed_metadata_and_no_metadata() {
        let data = r#"FF-no-meta -> true

@owner "team-a"
FF-with-meta -> false

FF-also-no-meta -> true"#;
        let (i, v) = parse_flagfile(data).unwrap();
        assert_eq!(i.trim(), "");
        assert_eq!(v.len(), 3);
        let def1 = v[0].get("FF-no-meta").unwrap();
        assert_eq!(def1.metadata, FlagMetadata::default());
        let def2 = v[1].get("FF-with-meta").unwrap();
        assert_eq!(def2.metadata.owner, Some("team-a".to_string()));
        let def3 = v[2].get("FF-also-no-meta").unwrap();
        assert_eq!(def3.metadata, FlagMetadata::default());
    }

    #[test]
    fn test_parse_requires_single() {
        let data = "@requires FF-new-checkout\nFF-checkout-upsell -> true";
        let (i, v) = parse_flagfile(data).unwrap();
        assert_eq!(i.trim(), "");
        let def = v[0].get("FF-checkout-upsell").unwrap();
        assert_eq!(def.metadata.requires, vec!["FF-new-checkout".to_string()]);
    }

    #[test]
    fn test_parse_requires_multiple() {
        let data = "@requires FF-base\n@requires FF-premium\nFF-advanced -> true";
        let (i, v) = parse_flagfile(data).unwrap();
        assert_eq!(i.trim(), "");
        let def = v[0].get("FF-advanced").unwrap();
        assert_eq!(
            def.metadata.requires,
            vec!["FF-base".to_string(), "FF-premium".to_string()]
        );
    }

    #[test]
    fn test_parse_requires_with_other_metadata() {
        let data = r#"@owner "team-a"
@requires FF-base
@type release
FF-feature -> true"#;
        let (i, v) = parse_flagfile(data).unwrap();
        assert_eq!(i.trim(), "");
        let def = v[0].get("FF-feature").unwrap();
        assert_eq!(def.metadata.owner, Some("team-a".to_string()));
        assert_eq!(def.metadata.flag_type, Some("release".to_string()));
        assert_eq!(def.metadata.requires, vec!["FF-base".to_string()]);
    }

    #[test]
    fn test_parse_no_requires_backward_compat() {
        let data = "FF-simple -> true";
        let (i, v) = parse_flagfile(data).unwrap();
        assert_eq!(i.trim(), "");
        let def = v[0].get("FF-simple").unwrap();
        assert!(def.metadata.requires.is_empty());
    }

    #[test]
    fn test_parse_metadata_with_comments() {
        let data = r#"// A comment about this flag
@owner "devops"
@type ops
FF-ops-flag -> true"#;
        let (i, v) = parse_flagfile(data).unwrap();
        assert_eq!(i.trim(), "");
        let def = v[0].get("FF-ops-flag").unwrap();
        assert_eq!(def.metadata.owner, Some("devops".to_string()));
        assert_eq!(def.metadata.flag_type, Some("ops".to_string()));
    }

    // ── Segment definition tests ──────────────────────────────────

    #[test]
    fn test_parse_segment_definition() {
        let data = r#"@segment beta_users {
    plan == beta
}

FF-beta-feature {
    segment(beta_users) -> true
    false
}"#;
        let (i, parsed) = parse_flagfile_with_segments(data).unwrap();
        assert_eq!(i.trim(), "");
        assert_eq!(parsed.segments.len(), 1);
        assert!(parsed.segments.contains_key("beta_users"));
        assert_eq!(parsed.flags.len(), 1);
        assert!(parsed.flags[0].contains_key("FF-beta-feature"));
    }

    #[test]
    fn test_parse_multiple_segments() {
        let data = r#"@segment premium {
    plan == premium
}

@segment us_users {
    country == US
}

FF-us-premium {
    segment(premium) and segment(us_users) -> true
    false
}"#;
        let (i, parsed) = parse_flagfile_with_segments(data).unwrap();
        assert_eq!(i.trim(), "");
        assert_eq!(parsed.segments.len(), 2);
        assert!(parsed.segments.contains_key("premium"));
        assert!(parsed.segments.contains_key("us_users"));
    }

    #[test]
    fn test_parse_segments_mixed_with_flags() {
        let data = r#"FF-simple -> true

@segment internal {
    email ~$ "@mycompany.com"
}

FF-internal-feature {
    segment(internal) -> true
    false
}

FF-another -> false"#;
        let (i, parsed) = parse_flagfile_with_segments(data).unwrap();
        assert_eq!(i.trim(), "");
        assert_eq!(parsed.segments.len(), 1);
        assert_eq!(parsed.flags.len(), 3);
    }

    #[test]
    fn test_parse_no_segments_backward_compat() {
        let data = "FF-flag1 -> true\nFF-flag2 -> false";
        let (i, parsed) = parse_flagfile_with_segments(data).unwrap();
        assert_eq!(i.trim(), "");
        assert_eq!(parsed.segments.len(), 0);
        assert_eq!(parsed.flags.len(), 2);
    }

    // ── @env rule tests ─────────────────────────────────────────────

    #[test]
    fn test_parse_env_simple() {
        let data = r#"FF-debug {
    @env dev -> true
    @env prod -> false
}"#;
        let (i, v) = parse_flagfile(data).unwrap();
        assert_eq!(i.trim(), "");
        let def = v[0].get("FF-debug").unwrap();
        assert_eq!(def.rules.len(), 2);
        assert!(matches!(&def.rules[0], Rule::EnvRule { env, rules } if env == "dev" && rules.len() == 1));
        assert!(matches!(&def.rules[1], Rule::EnvRule { env, rules } if env == "prod" && rules.len() == 1));
    }

    #[test]
    fn test_parse_env_block() {
        let data = r#"FF-search {
    @env prod {
        percentage(25%, userId) -> true
        false
    }
    true
}"#;
        let (i, v) = parse_flagfile(data).unwrap();
        assert_eq!(i.trim(), "");
        let def = v[0].get("FF-search").unwrap();
        assert_eq!(def.rules.len(), 2);
        assert!(matches!(&def.rules[0], Rule::EnvRule { env, rules } if env == "prod" && rules.len() == 2));
        assert!(matches!(&def.rules[1], Rule::Value(FlagReturn::OnOff(true))));
    }

    #[test]
    fn test_parse_env_mixed_with_regular_rules() {
        let data = r#"FF-feature {
    @env dev -> true
    @env stage -> true
    @env prod {
        plan == premium -> true
        false
    }
    false
}"#;
        let (i, v) = parse_flagfile(data).unwrap();
        assert_eq!(i.trim(), "");
        let def = v[0].get("FF-feature").unwrap();
        assert_eq!(def.rules.len(), 4);
        assert!(matches!(&def.rules[0], Rule::EnvRule { env, .. } if env == "dev"));
        assert!(matches!(&def.rules[1], Rule::EnvRule { env, .. } if env == "stage"));
        assert!(matches!(&def.rules[2], Rule::EnvRule { env, .. } if env == "prod"));
        assert!(matches!(&def.rules[3], Rule::Value(FlagReturn::OnOff(false))));
    }

    #[test]
    fn test_parse_env_with_metadata() {
        let data = r#"@owner "platform-team"
FF-logging {
    @env dev -> true
    false
}"#;
        let (i, v) = parse_flagfile(data).unwrap();
        assert_eq!(i.trim(), "");
        let def = v[0].get("FF-logging").unwrap();
        assert_eq!(def.metadata.owner, Some("platform-team".to_string()));
        assert_eq!(def.rules.len(), 2);
        assert!(matches!(&def.rules[0], Rule::EnvRule { env, .. } if env == "dev"));
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
