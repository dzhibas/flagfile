use nom::{
    branch::alt, bytes::complete::tag_no_case, character::complete::multispace0, combinator::map,
    complete::tag, error::ParseError, sequence::delimited, IResult,
};

use crate::ast::Atom;

/// A combinator that takes a parser `inner` and produces a parser that also consumes both leading and
/// trailing whitespace, returning the output of `inner`.
fn ws<'a, F: 'a, O, E: ParseError<&'a str>>(
    inner: F,
) -> impl FnMut(&'a str) -> IResult<&'a str, O, E>
where
    F: Fn(&'a str) -> IResult<&'a str, O, E>,
{
    delimited(multispace0, inner, multispace0)
}

fn parse_boolean(i: &str) -> IResult<&str, Atom> {
    let parser = alt((
        map(tag_no_case("true"), |_| true),
        map(tag_no_case("false"), |_| false),
    ));
    map(parser, Atom::Boolean)(i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bool() {
        let (_, v) = parse_boolean("True").unwrap();
        assert_eq!(v, Atom::Boolean(true));

        let (i, v) = parse_boolean("false and true").unwrap();
        assert_eq!(v, Atom::Boolean(false));
        assert_eq!(i, " and true");
    }
}
