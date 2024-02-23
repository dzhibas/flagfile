use std::collections::HashMap;

use nom::{
    branch::alt,
    bytes::complete::{is_not, tag, take_until},
    character::complete::{alpha1, alphanumeric1, char},
    combinator::{map, opt, recognize, value},
    multi::{many0, many0_count},
    sequence::{delimited, pair, preceded, terminated, tuple},
    IResult,
};
use serde_json::Value;

use crate::{
    ast::Atom,
    parse::{parse_boolean, ws},
};

#[derive(Debug, Clone)]
pub enum FlagReturn {
    OnOff(bool),
    Json(Value),
}

pub type FlagValue<'a> = HashMap<&'a str, FlagReturn>;

// Parses and throws away: // comment EOL
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

fn parse_flag_name(i: &str) -> IResult<&str, &str> {
    recognize(pair(
        tag("FF-"),
        many0_count(alt((alphanumeric1, tag("-"), tag("_")))),
    ))(i)
}

fn parse_anononymous_func(i: &str) -> IResult<&str, FlagValue> {
    let parser = tuple((
        ws(parse_flag_name),
        ws(tag("->")),
        alt((parse_bool, parse_json)),
    ));
    map(parser, |(n, _, v)| HashMap::from([(n, v)]))(i)
}

pub fn parse_flagfile(i: &str) -> IResult<&str, Vec<FlagValue>> {
    let parser = preceded(
        many0(alt((parse_comment, multiline_comment))),
        parse_anononymous_func,
    );
    let rest = terminated(parser, many0(alt((parse_comment, multiline_comment))));
    many0(rest)(i)
}

mod tests {
    use super::*;
    #[test]
    fn full_flag_file_test() {
        let data = include_str!("../Flagfile.example");

        let (i, v) = parse_flagfile(data).unwrap();
        dbg!(i, v);
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
