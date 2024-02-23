use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{alpha1, alphanumeric1, line_ending, space0},
    combinator::{eof, recognize},
    complete::take,
    multi::many0_count,
    sequence::{pair, tuple},
    IResult,
};
use std::{collections::HashMap, error::Error};

type Pair = HashMap<String, String>;
type AppError = Box<dyn Error>;

fn parse_variable(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        many0_count(alt((alphanumeric1, tag("_")))),
    ))(input)
}
fn parse_equal(input: &str) -> IResult<&str, (&str, &str, &str)> {
    tuple((space0, tag("="), space0))(input)
}
fn parse_value(input: &str) -> IResult<&str, &str> {
    let (i, (v, _)) = tuple((alphanumeric1, alt((line_ending, eof))))(input)?;
    Ok((i, v))
}

fn parse_main(input: &str) -> IResult<&str, Vec<Pair>> {
    let mut v = Vec::new();
    let (input, (var, _, val)) = tuple((parse_variable, parse_equal, parse_value))(input)?;
    v.push(HashMap::from([(var.into(), val.into())]));
    let (input, (var, _, val)) = tuple((parse_variable, parse_equal, parse_value))(input)?;
    v.push(HashMap::from([(var.into(), val.into())]));
    Ok((input, v))
}

fn main() -> Result<(), AppError> {
    let content = r"street_name=Random
countryCode=NL";

    let res = parse_main(content)?;

    println!("Trying to parse: {}", content);
    dbg!(res);

    Ok(())
}
