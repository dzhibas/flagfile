use chrono::{NaiveDate, NaiveDateTime};
use nom::{
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_until},
    character::complete::{alpha1, alphanumeric1, char, digit1, multispace0},
    combinator::{cut, map, opt, recognize},
    error::ParseError,
    multi::{many0, many0_count, separated_list0},
    sequence::{delimited, pair, tuple},
    IResult,
};

use crate::ast::{ArrayOp, AstNode, Atom, ComparisonOp, FnCall, LogicOp, MatchOp};

/// Took from nom recipes
pub fn ws<'a, F, O, E: ParseError<&'a str>>(
    inner: F,
) -> impl FnMut(&'a str) -> IResult<&'a str, O, E>
where
    F: Fn(&'a str) -> IResult<&'a str, O, E> + 'a,
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

pub fn parse_boolean(i: &str) -> IResult<&str, Atom> {
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

fn parse_datetime(i: &str) -> IResult<&str, Atom> {
    let parser = recognize(tuple((
        digit1,
        char('-'),
        digit1,
        char('-'),
        digit1,
        char('T'),
        digit1,
        char(':'),
        digit1,
        char(':'),
        digit1,
        opt(char('Z')),
    )));

    map(parser, |dt_str: &str| {
        let clean = dt_str.strip_suffix('Z').unwrap_or(dt_str);
        let dt = NaiveDateTime::parse_from_str(clean, "%Y-%m-%dT%H:%M:%S")
            .expect("Invalid datetime format");
        Atom::DateTime(dt)
    })(i)
}

fn parse_semver(i: &str) -> IResult<&str, Atom> {
    let parser = tuple((digit1, char('.'), digit1, char('.'), digit1));
    map(
        parser,
        |(major, _, minor, _, patch): (&str, _, &str, _, &str)| {
            Atom::Semver(
                major.parse().unwrap(),
                minor.parse().unwrap(),
                patch.parse().unwrap(),
            )
        },
    )(i)
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
        parse_datetime,
        parse_date,
        parse_string,
        parse_boolean,
        parse_semver,
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
    alt((
        parse_coalesce,
        parse_nullary_function,
        parse_variable_node_modifier,
        parse_variable_node,
    ))(i)
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

fn parse_coalesce_arg(i: &str) -> IResult<&str, AstNode> {
    alt((parse_variable_node, parse_constant))(i)
}

fn parse_coalesce(i: &str) -> IResult<&str, AstNode> {
    let (i, _) = tag_no_case("coalesce")(i)?;
    let (i, _) = ws(char('('))(i)?;

    let (i, args) = separated_list0(ws(char(',')), ws(parse_coalesce_arg))(i)?;

    if args.len() < 2 {
        return Err(nom::Err::Error(nom::error::Error::new(
            i,
            nom::error::ErrorKind::Many1,
        )));
    }

    let (i, _) = ws(char(')'))(i)?;
    Ok((i, AstNode::Coalesce(args)))
}

pub(crate) fn parse_segment_name(i: &str) -> IResult<&str, &str> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        many0_count(alt((alphanumeric1, tag("_"), tag("-")))),
    ))(i)
}

fn parse_segment_call(i: &str) -> IResult<&str, AstNode> {
    let (i, _) = tag_no_case("segment")(i)?;
    let (i, _) = char('(')(i)?;
    let (i, _) = multispace0(i)?;
    let (i, name) = parse_segment_name(i)?;
    let (i, _) = multispace0(i)?;
    let (i, _) = char(')')(i)?;
    Ok((i, AstNode::Segment(name.to_string())))
}

fn parse_nullary_function(i: &str) -> IResult<&str, AstNode> {
    let (i, _) = tag_no_case("now()")(i)?;
    Ok((i, AstNode::Function(FnCall::Now, Box::new(AstNode::Void))))
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

fn parse_regex_literal(i: &str) -> IResult<&str, Atom> {
    let (i, _) = tag("/")(i)?;
    let (i, pattern) = take_until("/")(i)?;
    let (i, _) = tag("/")(i)?;
    Ok((i, Atom::Regex(pattern.to_string())))
}

fn parse_match_op(i: &str) -> IResult<&str, MatchOp> {
    alt((
        map(tag("!^~"), |_| MatchOp::NotStartsWith),
        map(tag("!~$"), |_| MatchOp::NotEndsWith),
        map(tag("!~"), |_| MatchOp::NotContains),
        map(tag("^~"), |_| MatchOp::StartsWith),
        map(tag("~$"), |_| MatchOp::EndsWith),
        map(tag("~"), |_| MatchOp::Contains),
    ))(i)
}

fn parse_match_rhs(i: &str) -> IResult<&str, AstNode> {
    alt((map(parse_regex_literal, AstNode::Constant), parse_constant))(i)
}

fn parse_match_expr(i: &str) -> IResult<&str, AstNode> {
    let parser = tuple((
        parse_variable_node_or_modified,
        ws(parse_match_op),
        parse_match_rhs,
    ));
    map(parser, |(var, op, val)| {
        AstNode::Match(Box::new(var), op, Box::new(val))
    })(i)
}

fn parse_reverse_array_expr(i: &str) -> IResult<&str, AstNode> {
    let parser = tuple((parse_constant, ws(parse_array_op), parse_variable_node));
    map(parser, |(val, op, var)| {
        AstNode::Array(Box::new(val), op, Box::new(var))
    })(i)
}

fn parse_null_check(i: &str) -> IResult<&str, AstNode> {
    let (i, var) = parse_variable_node_or_modified(i)?;
    let (i, _) = multispace0(i)?;
    let (i, _) = tag_no_case("is")(i)?;
    let (i, _) = multispace0(i)?;
    let (i, negated) = opt(tuple((tag_no_case("not"), multispace0)))(i)?;
    let (i, _) = tag_no_case("null")(i)?;
    // Ensure 'null' is not followed by word characters
    if i.starts_with(|c: char| c.is_alphanumeric() || c == '_') {
        return Err(nom::Err::Error(nom::error::Error::new(
            i,
            nom::error::ErrorKind::Tag,
        )));
    }
    Ok((
        i,
        AstNode::NullCheck {
            variable: Box::new(var),
            is_null: negated.is_none(),
        },
    ))
}

fn parse_compare_or_array_expr(i: &str) -> IResult<&str, AstNode> {
    alt((
        parse_null_check,
        parse_array_expr,
        parse_reverse_array_expr,
        parse_match_expr,
        parse_compare_expr,
    ))(i)
}

fn parse_logic_expr(i: &str) -> IResult<&str, AstNode> {
    // a=b AND b not in (1,2,3)
    let parser = tuple((
        alt((parse_compare_or_array_expr, parse_parenthesized_expr)),
        ws(parse_logic_op),
        alt((parse_compare_or_array_expr, parse_parenthesized_expr)),
    ));
    map(parser, |(var, op, val)| {
        AstNode::Logic(Box::new(var), op, Box::new(val))
    })(i)
}

fn parse_percentage_salt(i: &str) -> IResult<&str, String> {
    let (i, _) = ws(char(','))(i)?;
    let i = i.trim_start();
    let (i, salt_val) = recognize(pair(
        alt((alpha1, tag("_"))),
        many0_count(alt((alphanumeric1, tag("_"), tag("-")))),
    ))(i)?;
    Ok((i, salt_val.to_string()))
}

fn parse_percentage(i: &str) -> IResult<&str, AstNode> {
    let (i, _) = tag_no_case("percentage")(i)?;
    let (i, _) = ws(char('('))(i)?;

    // Parse rate: number followed by '%'
    let (i, rate_str) = recognize(pair(
        opt(alt((char('+'), char('-')))),
        alt((
            recognize(tuple((digit1, pair(char('.'), opt(digit1))))),
            recognize(tuple((char('.'), digit1))),
            digit1,
        )),
    ))(i)?;
    let (i, _) = char('%')(i)?;
    let rate: f64 = rate_str.parse().unwrap();

    // Parse comma and field name
    let (i, _) = ws(char(','))(i)?;
    let (i, field) = ws(parse_variable_node)(i)?;

    // Parse optional salt (third argument)
    let (i, salt) = opt(parse_percentage_salt)(i)?;

    let (i, _) = ws(char(')'))(i)?;

    Ok((
        i,
        AstNode::Percentage {
            rate,
            field: Box::new(field),
            salt,
        },
    ))
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
        parse_percentage,
        parse_segment_call,
        parse_logic_expr,
        parse_compare_or_array_expr,
        parse_constant,
    ))(input)?;

    let (i, tail) = many0(pair(
        ws(parse_logic_op),
        alt((
            parse_percentage,
            parse_segment_call,
            parse_compare_or_array_expr,
            parse_parenthesized_expr,
        )),
    ))(i)?;

    for (op, expr) in tail {
        head = AstNode::Logic(Box::new(head.clone()), op.clone(), Box::new(expr.clone()));
    }

    Ok((i, head.clone()))
}

pub fn parse(i: &str) -> IResult<&str, AstNode> {
    alt((ws(parse_expr), ws(parse_parenthesized_expr)))(i)
}

#[cfg(test)]
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
        let (_i, v) = parse_comparison_op("<>").unwrap();
        assert_eq!(v, ComparisonOp::NotEq);

        let (i, v) = parse_comparison_op("==").unwrap();
        assert_eq!(v, ComparisonOp::Eq);
        assert_eq!(i, "");
    }

    #[test]
    fn test_logic_op() {
        let (_i, v) = parse_logic_op("&& this").unwrap();
        assert_eq!(v, LogicOp::And);
    }

    #[test]
    fn test_compare_expr() {
        let (i, _v) = parse_compare_expr("_demo >= 10").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_logic_expr() {
        let (i, _v) =
            parse_logic_expr("_demo >= 10 && demo == \"something more than that\"").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_parse_list() {
        let (i, _v) = parse_list("(1,2, 34, \"demo\", -10, -3.14)").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_logic_expression_with_list() {
        let e = "a = 2 and b in  (1,2.2, \"demo\")";
        let (i, _v) = parse_logic_expr(e).unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_more_complex_not_in() {
        assert_eq!(
            parse_logic_expr("a=3 && c = 3 || d not in (2,4,5)").is_ok(),
            true
        );
        let (i, _v) = parse_expr("a=3 && c = 3 || d not in (2,4,5) and this<>34.43").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_list_bug() {
        // this should not be allowed as array should have either in () or not in ()
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
    fn test_parse_datetime() {
        let a = "2025-06-15T09:00:00Z";
        let res = parse_datetime(a);
        assert!(res.is_ok());
        if let Ok((i, v)) = res {
            assert_eq!(i, "");
            let expected =
                NaiveDateTime::parse_from_str("2025-06-15T09:00:00", "%Y-%m-%dT%H:%M:%S").unwrap();
            assert_eq!(v, Atom::DateTime(expected));
        }
    }

    #[test]
    fn test_parse_datetime_without_z() {
        let a = "2025-06-15T09:00:00";
        let res = parse_datetime(a);
        assert!(res.is_ok());
        if let Ok((i, _v)) = res {
            assert_eq!(i, "");
        }
    }

    #[test]
    fn test_datetime_before_date_in_atom() {
        // DateTime should be parsed as DateTime, not Date
        let (i, v) = parse_atom("2025-06-15T09:00:00Z").unwrap();
        assert_eq!(i, "");
        assert!(matches!(v, Atom::DateTime(_)));

        // Plain date should still be parsed as Date
        let (i2, v2) = parse_atom("2025-06-15").unwrap();
        assert_eq!(i2, "");
        assert!(matches!(v2, Atom::Date(_)));
    }

    #[test]
    fn test_datetime_comparison_expr() {
        let (i, _v) = parse_compare_expr("now() > 2025-06-15T09:00:00Z").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_datetime_range_expr() {
        let (i, _v) =
            parse("now() > 2025-06-15T09:00:00Z and now() < 2025-06-15T18:00:00Z").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_single_quote_string() {
        let a = "a='demo demo'";
        let res = parse_compare_expr(a);
        assert_eq!(res.is_ok(), true);
        if let Ok((i, _v)) = res {
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
    fn test_parse_semver() {
        let (i, v) = parse_semver("5.3.42").unwrap();
        assert_eq!(v, Atom::Semver(5, 3, 42));
        assert_eq!(i, "");

        let (i, v) = parse_semver("0.1.0").unwrap();
        assert_eq!(v, Atom::Semver(0, 1, 0));
        assert_eq!(i, "");

        // 2-component is not semver, should fail
        assert!(parse_semver("4.32").is_err());
    }

    #[test]
    fn test_semver_atom() {
        let (_, v) = parse_atom("5.3.42").unwrap();
        assert_eq!(v, Atom::Semver(5, 3, 42));

        // 2-component still parses as float
        let (_, v) = parse_atom("4.32").unwrap();
        assert_eq!(v, Atom::Float(4.32));
    }

    #[test]
    fn test_semver_comparison_expr() {
        let (i, _v) = parse_compare_expr("version > 5.3.42").unwrap();
        assert_eq!(i, "");

        let (i, _v) = parse_compare_expr("appVersion <= 4.32.0").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_extreme_logic_test() {
        let expression = r###"a = b and c=d and something not in (1,2,3) or lower(z) == "demo car" or
    z == "demo car" or
    g in (4,5,6) and z == "demo car" or
    model in (ms,mx,m3,my) and !(created >= 2024-01-01
        and demo == false) and ((a=2) and not (c=3))"###;
        let (i, _v) = parse(expression).unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_parse_match_contains() {
        let (i, _v) = parse("name ~ Nik").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_parse_match_not_contains() {
        let (i, _v) = parse("name !~ Nik").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_parse_match_regex() {
        let (i, _v) = parse("name ~ /.*ola.*/").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_parse_match_regex_with_function_call() {
        let (i, _v) = parse("upper(name) ~ /.*OLA.*/").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_parse_match_not_regex() {
        let (i, _v) = parse("name !~ /.*ola.*/").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_parse_match_in_logic_expr() {
        let (i, _v) = parse("name ~ Nik and age > 18").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_parse_match_starts_with() {
        let (i, v) = parse("path ^~ \"/admin\"").unwrap();
        assert_eq!(i, "");
        assert_eq!(
            v,
            AstNode::Match(
                Box::new(AstNode::Variable(Atom::Variable("path".into()))),
                MatchOp::StartsWith,
                Box::new(AstNode::Constant(Atom::String("/admin".into()))),
            )
        );
    }

    #[test]
    fn test_parse_match_ends_with() {
        let (i, v) = parse("email ~$ \"@company.com\"").unwrap();
        assert_eq!(i, "");
        assert_eq!(
            v,
            AstNode::Match(
                Box::new(AstNode::Variable(Atom::Variable("email".into()))),
                MatchOp::EndsWith,
                Box::new(AstNode::Constant(Atom::String("@company.com".into()))),
            )
        );
    }

    #[test]
    fn test_parse_match_not_starts_with() {
        let (i, v) = parse("name !^~ \"test\"").unwrap();
        assert_eq!(i, "");
        assert_eq!(
            v,
            AstNode::Match(
                Box::new(AstNode::Variable(Atom::Variable("name".into()))),
                MatchOp::NotStartsWith,
                Box::new(AstNode::Constant(Atom::String("test".into()))),
            )
        );
    }

    #[test]
    fn test_parse_match_not_ends_with() {
        let (i, v) = parse("name !~$ \".tmp\"").unwrap();
        assert_eq!(i, "");
        assert_eq!(
            v,
            AstNode::Match(
                Box::new(AstNode::Variable(Atom::Variable("name".into()))),
                MatchOp::NotEndsWith,
                Box::new(AstNode::Constant(Atom::String(".tmp".into()))),
            )
        );
    }

    #[test]
    fn test_parse_starts_with_in_logic_expr() {
        let (i, _v) = parse("path ^~ \"/api\" and method == \"GET\"").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_parse_coalesce() {
        let (i, v) = parse("coalesce(a, b, \"default\") == \"test\"").unwrap();
        assert_eq!(i, "");
        assert!(matches!(v, AstNode::Compare(_, _, _)));
    }

    #[test]
    fn test_parse_coalesce_in_logic() {
        let (i, _v) = parse("coalesce(x, y, \"none\") == \"val\" and z > 5").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_parse_ends_with_with_function() {
        let (i, _v) = parse("lower(name) ^~ \"admin\"").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_parse_reverse_in() {
        let (i, _v) = parse("\"admin\" in roles").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_parse_reverse_not_in() {
        let (i, _v) = parse("\"admin\" not in roles").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_parse_reverse_in_logic() {
        let (i, _v) = parse("\"admin\" in roles or \"editor\" in roles").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_parse_reverse_in_combined_with_comparison() {
        let (i, _v) = parse("\"admin\" in roles and plan == \"premium\"").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_parse_null_check_is_null() {
        let (i, v) = parse("userId is null").unwrap();
        assert_eq!(i, "");
        assert_eq!(
            v,
            AstNode::NullCheck {
                variable: Box::new(AstNode::Variable(Atom::Variable("userId".into()))),
                is_null: true,
            }
        );
    }

    #[test]
    fn test_parse_null_check_is_not_null() {
        let (i, v) = parse("userId is not null").unwrap();
        assert_eq!(i, "");
        assert_eq!(
            v,
            AstNode::NullCheck {
                variable: Box::new(AstNode::Variable(Atom::Variable("userId".into()))),
                is_null: false,
            }
        );
    }

    #[test]
    fn test_parse_null_check_case_insensitive() {
        let (i, _) = parse("userId IS NULL").unwrap();
        assert_eq!(i, "");
        let (i, _) = parse("userId IS NOT NULL").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_parse_null_check_in_logic() {
        let (i, _) = parse("userId is null or plan == premium").unwrap();
        assert_eq!(i, "");
        let (i, _) = parse("userId is not null and plan == premium").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_parse_segment_call() {
        let (i, v) = parse("segment(beta_users)").unwrap();
        assert_eq!(i, "");
        assert_eq!(v, AstNode::Segment("beta_users".to_string()));
    }

    #[test]
    fn test_parse_segment_call_with_hyphens() {
        let (i, v) = parse("segment(premium-users)").unwrap();
        assert_eq!(i, "");
        assert_eq!(v, AstNode::Segment("premium-users".to_string()));
    }

    #[test]
    fn test_parse_segment_call_in_logic() {
        let (i, _v) = parse("segment(beta_users) and plan == premium").unwrap();
        assert_eq!(i, "");
    }

    #[test]
    fn test_parse_segment_call_combined() {
        let (i, _v) = parse("country == US or segment(enterprise)").unwrap();
        assert_eq!(i, "");
    }
}
