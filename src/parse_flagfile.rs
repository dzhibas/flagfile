use nom::{
    bytes::complete::{is_not, tag},
    character::complete::char,
    combinator::value,
    error::ParseError,
    sequence::pair,
    IResult, Parser,
};

use crate::parse::ws;

fn parse_comment(i: &str) -> IResult<&str, ()> {
    value((), pair(ws(tag("//")), is_not("\n\r"))).parse(i)
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
