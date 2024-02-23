use nom::{
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_until},
    character::complete::{alpha1, alphanumeric1, digit1, multispace0},
    combinator::{map, opt, recognize},
    error::ParseError,
    multi::{many0, many0_count, separated_list0},
    number::complete::double,
    sequence::{delimited, pair, tuple},
    IResult,
};

use crate::ast::{AstNode, Atom, ComparisonOp, LogicOp};

/// Took from nom recipes
fn ws<'a, F: 'a, O, E: ParseError<&'a str>>(
    inner: F,
) -> impl FnMut(&'a str) -> IResult<&'a str, O, E>
where
    F: Fn(&'a str) -> IResult<&'a str, O, E>,
{
    delimited(multispace0, inner, multispace0)
}

fn parse_number(i: &str) -> IResult<&str, Atom> {
    let parser = recognize(pair(opt(tag("-")), digit1));
    map(parser, |num: &str| Atom::Number(num.parse().unwrap()))(i)
}
fn parse_float(i: &str) -> IResult<&str, Atom> {
    map(double, Atom::Float)(i)
}

fn parse_boolean(i: &str) -> IResult<&str, Atom> {
    let parser = alt((
        map(tag_no_case("true"), |_| true),
        map(tag_no_case("false"), |_| false),
    ));
    map(parser, Atom::Boolean)(i)
}

fn parse_string(i: &str) -> IResult<&str, Atom> {
    let parser = delimited(tag("\""), take_until("\""), tag("\""));
    map(parser, |s: &str| Atom::String(s.to_string()))(i)
}

fn parse_variable(i: &str) -> IResult<&str, Atom> {
    let parser = recognize(pair(
        alt((alpha1, tag("_"))),
        many0_count(alt((alphanumeric1, tag("_")))),
    ));
    map(parser, |v: &str| Atom::Variable(v.to_string()))(i)
}

fn parse_atom(i: &str) -> IResult<&str, Atom> {
    alt((
        parse_string,
        parse_boolean,
        parse_float,
        parse_number,
        parse_variable,
    ))(i)
}

fn parse_comparison_op(i: &str) -> IResult<&str, ComparisonOp> {
    alt((
        map(alt((tag("!="), tag("<>"))), |_| ComparisonOp::NotEq),
        map(alt((tag("=="), tag("="))), |_| ComparisonOp::Eq),
        map(tag("<="), |_| ComparisonOp::LessEq),
        map(tag("<"), |_| ComparisonOp::Less),
        map(tag(">="), |_| ComparisonOp::MoreEq),
        map(tag(">"), |_| ComparisonOp::More),
    ))(i)
}

fn parse_logic_op(i: &str) -> IResult<&str, LogicOp> {
    alt((
        map(alt((tag("&&"), tag_no_case("and"))), |_| LogicOp::And),
        map(alt((tag("||"), tag_no_case("or"))), |_| LogicOp::Or),
    ))(i)
}

fn parse_list(i: &str) -> IResult<&str, AstNode> {
    let parser = delimited(
        tag("("),
        separated_list0(tag(","), ws(parse_atom)),
        tag(")"),
    );
    map(parser, AstNode::List)(i)
}

fn parse_compare_expr(i: &str) -> IResult<&str, AstNode> {
    let parser = tuple((parse_variable, ws(parse_comparison_op), parse_atom));
    map(parser, |(var, op, val)| {
        AstNode::Compare(
            Box::new(AstNode::Variable(var)),
            op,
            Box::new(AstNode::Constant(val)),
        )
    })(i)
}

fn parse_logic_expr(i: &str) -> IResult<&str, AstNode> {
    let parser = tuple((parse_compare_expr, ws(parse_logic_op), parse_compare_expr));
    map(parser, |(var, op, val)| {
        AstNode::Logic(Box::new(var), op, Box::new(val))
    })(i)
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

    #[test]
    fn test_parse_numbers() {
        let (_, v) = parse_number("-10").unwrap();
        assert_eq!(v, Atom::Number(-10));

        let (_, v) = parse_number("199").unwrap();
        assert_eq!(v, Atom::Number(199));
    }

    #[test]
    fn test_float() {
        let (_,v) = parse_atom("3.14").unwrap();
        assert_eq!(v, Atom::Float(3.14));
    }

    #[test]
    fn test_parse_string() {
        let (_, v) = parse_string("\"this is demo\"").unwrap();
        assert_eq!(v, Atom::String("this is demo".to_string()));
    }

    #[test]
    fn test_parse_atom() {
        let (_, v) = parse_atom("_demo_demo").unwrap();
        assert_eq!(v, Atom::Variable("_demo_demo".to_string()));
    }

    #[test]
    fn test_comparison_op() {
        let (i, v) = parse_comparison_op("<>").unwrap();
        assert_eq!(v, ComparisonOp::NotEq);

        let (i, v) = parse_comparison_op("==").unwrap();
        assert_eq!(v, ComparisonOp::Eq);
        assert_eq!(i, "");
    }

    #[test]
    fn test_logic_op() {
        let (i, v) = parse_logic_op("&& this").unwrap();
        assert_eq!(v, LogicOp::And);
    }

    #[test]
    fn test_compare_expr() {
        let (i, v) = parse_compare_expr("_demo >= 10").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_logic_expr() {
        let (i, v) = parse_logic_expr("_demo >= 10 && demo == \"something more than that\"").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_parse_list() {
        let (i, v) = parse_list("(1,2, 34, \"demo\", -10, -3.14)").unwrap();
        assert_eq!(i, "");
    }
}
