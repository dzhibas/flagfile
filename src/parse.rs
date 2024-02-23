use chrono::NaiveDate;
use nom::{
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_until, take_while_m_n},
    character::{
        complete::{alpha1, alphanumeric1, char, digit1, multispace0},
        is_digit,
    },
    combinator::{cut, map, map_res, opt, recognize},
    error::ParseError,
    multi::{many0, many0_count, many_m_n, separated_list0},
    number::complete::double,
    sequence::{delimited, pair, preceded, tuple},
    IResult,
};

use crate::ast::{ArrayOp, AstNode, Atom, ComparisonOp, FnCall, LogicOp};

/// Took from nom recipes
pub fn ws<'a, F: 'a, O, E: ParseError<&'a str>>(
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

/// modified original double parser to always have "." for floats
fn parse_float(i: &str) -> IResult<&str, Atom> {
    let parser = recognize(tuple((
        opt(alt((char('+'), char('-')))),
        alt((
            map(tuple((digit1, pair(char('.'), opt(digit1)))), |_| ()),
            map(tuple((char('.'), digit1)), |_| ()),
        )),
        opt(tuple((
            alt((char('e'), char('E'))),
            opt(alt((char('+'), char('-')))),
            cut(digit1),
        ))),
    )));

    map(parser, |n: &str| Atom::Float(n.parse().unwrap()))(i)
}

fn parse_boolean(i: &str) -> IResult<&str, Atom> {
    let parser = alt((
        map(tag_no_case("true"), |_| true),
        map(tag_no_case("false"), |_| false),
    ));
    map(parser, Atom::Boolean)(i)
}

fn parse_date(i: &str) -> IResult<&str, Atom> {
    let parser = recognize(tuple((digit1, char('-'), digit1, char('-'), digit1)));

    map(parser, |date_str: &str| {
        let dt = NaiveDate::parse_from_str(date_str, "%Y-%m-%d").expect("Invalid date format");
        Atom::Date(dt)
    })(i)
}

fn parse_string(i: &str) -> IResult<&str, Atom> {
    let parser_a = delimited(tag("\""), take_until("\""), tag("\""));
    let parser_b = delimited(tag("\'"), take_until("\'"), tag("\'"));
    let parser = alt((parser_a, parser_b));
    map(parser, |s: &str| Atom::String(s.to_string()))(i)
}

fn parse_variable(i: &str) -> IResult<&str, Atom> {
    let parser = recognize(pair(
        alt((alpha1, tag("_"))),
        many0_count(alt((alphanumeric1, tag("_")))),
    ));
    map(parser, |v: &str| Atom::Variable(v.to_string()))(i)
}

pub fn parse_atom(i: &str) -> IResult<&str, Atom> {
    alt((
        parse_date,
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
fn parse_variable_node(i: &str) -> IResult<&str, AstNode> {
    map(parse_variable, AstNode::Variable)(i)
}

fn parse_variable_node_modifier(i: &str) -> IResult<&str, AstNode> {
    let parser = tuple((
        ws(parse_function_names),
        delimited(tag("("), ws(parse_variable_node), tag(")")),
    ));
    map(parser, |(fn_call, expr)| {
        AstNode::Function(fn_call, Box::new(expr))
    })(i)
}

fn parse_variable_node_or_modified(i: &str) -> IResult<&str, AstNode> {
    alt((parse_variable_node_modifier, parse_variable_node))(i)
}

fn parse_constant(i: &str) -> IResult<&str, AstNode> {
    map(parse_atom, AstNode::Constant)(i)
}

fn parse_array_op(i: &str) -> IResult<&str, ArrayOp> {
    alt((
        map(tag_no_case("not in"), |_| ArrayOp::NotIn),
        map(tag_no_case("in"), |_| ArrayOp::In),
    ))(i)
}

fn parse_function_names(i: &str) -> IResult<&str, FnCall> {
    alt((
        map(tag_no_case("upper"), |_| FnCall::Upper),
        map(tag_no_case("lower"), |_| FnCall::Lower),
    ))(i)
}

fn parse_array_expr(i: &str) -> IResult<&str, AstNode> {
    let parser = tuple((
        parse_variable_node_or_modified,
        ws(parse_array_op),
        parse_list,
    ));
    map(parser, |(var, op, val)| {
        AstNode::Array(Box::new(var), op, Box::new(val))
    })(i)
}

fn parse_compare_expr(i: &str) -> IResult<&str, AstNode> {
    let parser = tuple((
        parse_variable_node_or_modified,
        ws(parse_comparison_op),
        parse_constant,
    ));
    map(parser, |(var, op, val)| {
        AstNode::Compare(Box::new(var), op, Box::new(val))
    })(i)
}

fn parse_compare_or_array_expr(i: &str) -> IResult<&str, AstNode> {
    alt((parse_array_expr, parse_compare_expr))(i)
}

fn parse_logic_expr(i: &str) -> IResult<&str, AstNode> {
    /// a=b AND b not in (1,2,3)
    let parser = tuple((
        alt((parse_compare_or_array_expr, parse_parenthesized_expr)),
        ws(parse_logic_op),
        alt((parse_compare_or_array_expr, parse_parenthesized_expr)),
    ));
    map(parser, |(var, op, val)| {
        AstNode::Logic(Box::new(var), op, Box::new(val))
    })(i)
}

fn parse_parenthesized_expr(i: &str) -> IResult<&str, AstNode> {
    let parser = tuple((
        opt(alt((tag_no_case("not"), tag("!")))),
        delimited(ws(char('(')), parse_expr, ws(char(')'))),
    ));

    map(parser, |(not, expr)| AstNode::Scope {
        expr: Box::new(expr),
        negate: not.is_some(),
    })(i)
}

fn parse_expr(input: &str) -> IResult<&str, AstNode> {
    let (i, mut head) = alt((
        parse_parenthesized_expr,
        parse_logic_expr,
        parse_compare_or_array_expr,
        parse_constant,
    ))(input)
    .expect("parse failed");

    let (i, tail) = many0(pair(
        ws(parse_logic_op),
        alt((parse_compare_or_array_expr, parse_parenthesized_expr)),
    ))(i)
    .expect("Parse failed");

    for (op, expr) in tail {
        head = AstNode::Logic(Box::new(head.clone()), op.clone(), Box::new(expr.clone()));
    }

    Ok((i, head.clone()))
}

pub fn parse(i: &str) -> IResult<&str, AstNode> {
    alt((ws(parse_expr), ws(parse_parenthesized_expr)))(i)
}

mod tests {
    use super::*;

    #[test]
    fn parse_constant_test() {
        let res = parse("True");
        assert_eq!(true, res.is_ok());
        assert_eq!(AstNode::Constant(Atom::Boolean(true)), res.unwrap().1);
    }

    #[test]
    fn test_parse_bool() {
        let (_, v) = parse_boolean("True").unwrap();
        assert_eq!(v, Atom::Boolean(true));

        let (i, v) = parse_boolean("false and true").unwrap();
        assert_eq!(v, Atom::Boolean(false));
        assert_eq!(i, " and true");
    }

    #[test]
    fn parse_boolean_test() {
        let test_cases = vec![
            ("true", Ok(("", Atom::Boolean(true)))),
            ("TRUE", Ok(("", Atom::Boolean(true)))),
            ("false", Ok(("", Atom::Boolean(false)))),
            ("FALSE", Ok(("", Atom::Boolean(false)))),
            (
                "1",
                Err(nom::Err::Error(nom::error::Error {
                    input: "1",
                    code: nom::error::ErrorKind::Tag,
                })),
            ),
            (
                "hello",
                Err(nom::Err::Error(nom::error::Error {
                    input: "hello",
                    code: nom::error::ErrorKind::Tag,
                })),
            ),
        ];

        for (input, expected) in test_cases {
            let result = parse_boolean(input);
            assert_eq!(result, expected);
        }
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
        let (_, v) = parse_atom("3.14").unwrap();
        assert_eq!(v, Atom::Float(3.14));
    }

    #[test]
    fn test_float_bug() {
        let (_, v) = parse_atom("3").unwrap();
        assert_eq!(v, Atom::Number(3));
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
        let (i, v) =
            parse_logic_expr("_demo >= 10 && demo == \"something more than that\"").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_parse_list() {
        let (i, v) = parse_list("(1,2, 34, \"demo\", -10, -3.14)").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_logic_expresion_with_list() {
        let e = "a = 2 and b in  (1,2.2, \"demo\")";
        let (i, v) = parse_logic_expr(e).unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_more_complext_not_in() {
        assert_eq!(
            parse_logic_expr("a=3 && c = 3 || d not in (2,4,5)").is_ok(),
            true
        );
        let (i, v) = parse_expr("a=3 && c = 3 || d not in (2,4,5) and this<>34.43").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_list_bug() {
        /// this should not be allowed as array should have either in () or not in ()
        let a = "a == 2 and b >= (1,2,3)";
        let res = parse_logic_expr(a);
        assert_eq!(res.is_err(), true);
    }

    #[test]
    fn test_parse_date() {
        let a = "2004-12-23";
        let res = parse_date(a);
        assert_eq!(res.is_ok(), true);
        if let Ok((i, v)) = res {
            assert_eq!(i, "");
            assert_eq!(
                v,
                Atom::Date(NaiveDate::from_ymd_opt(2004, 12, 23).unwrap())
            );
        }
    }

    #[test]
    fn test_single_quote_string() {
        let a = "a='demo demo'";
        let res = parse_compare_expr(a);
        assert_eq!(res.is_ok(), true);
        if let Ok((i, v)) = res {
            assert_eq!(i, "");
        }
    }

    #[test]
    fn test_scopes() {
        let res = parse("not (a=b and c=d)");
        assert_eq!(res.is_ok(), true);
    }

    #[test]
    fn test_fn_modifiers() {
        let res = parse("UPPER(_demo) == 'DEMO DEMO'");
        assert_eq!(res.is_ok(), true);
    }

    #[test]
    fn test_extreme_logic_test() {
        let expression = r###"a = b and c=d and something not in (1,2,3) or lower(z) == "demo car" or
    z == "demo car" or
    g in (4,5,6) and z == "demo car" or
    model in (ms,mx,m3,my) and !(created >= 2024-01-01
        and demo == false) and ((a=2) and not (c=3))"###;
        let (i, v) = parse(expression).unwrap();
        assert_eq!(i, "");
    }
}
