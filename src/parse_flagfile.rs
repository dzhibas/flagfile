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
}

#[derive(Debug, Clone)]
pub enum Rule {
    Value(FlagReturn),
    BoolExpressionValue(AstNode, FlagReturn),
}

pub type FlagValue<'a> = HashMap<&'a str, Vec<Rule>>;

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
        FlagReturn::Json(serde_json::from_str(&v).unwrap())
    })(i)
}

fn parse_bool(i: &str) -> IResult<&str, FlagReturn> {
    map(parse_boolean, |v| match v {
        Atom::Boolean(v) => FlagReturn::OnOff(v),
        _ => unreachable!(),
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
    alt((ws(parse_bool), ws(parse_json)))(i)
}

fn parse_anonymous_func(i: &str) -> IResult<&str, FlagValue> {
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
    map(parse_return_val, |v| Rule::Value(v))(i)
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

fn parse_function(i: &str) -> IResult<&str, FlagValue> {
    let parser = pair(
        ws(parse_flag_name),
        delimited(ws(tag("{")), parse_rules_list, ws(tag("}"))),
    );
    map(parser, |(flag_name, rules)| {
        HashMap::from([(flag_name, rules)])
    })(i)
}

pub fn parse_flagfile(i: &str) -> IResult<&str, Vec<FlagValue>> {
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
    fn full_flag_file_test() {
        let data = include_str!("../Flagfile.example");

        let (i, v) = parse_flagfile(data).unwrap();
        dbg!(i, &v);
        assert_eq!(true, v.len() > 0);
        assert_eq!(i.to_string().trim(), "");
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
