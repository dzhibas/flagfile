use nom::{
    bytes::complete::{is_not, tag, take_until},
    character::complete::char,
    combinator::value,
    sequence::{delimited, pair},
    IResult,
};

use crate::parse::ws;

// Parses and throws away: // comment EOL
fn parse_comment(i: &str) -> IResult<&str, ()> {
    value((), pair(ws(tag("//")), is_not("\n\r")))(i)
}

/// Parses and throws away: /* comment */
fn multiline_comment(i: &str) -> IResult<&str, ()> {
    value((), delimited(tag("/*"), take_until("*/"), tag("*/")))(i)
}

// feature-name
// -> arrow function
// { expr } function
// expr : Atom
// return bool
// return json(...)
// return grpc(...)
// comment
// parse_flagfile -> opt(comment) feature func rules default return
