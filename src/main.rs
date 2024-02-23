use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{alpha1, alphanumeric1, line_ending, space0},
    combinator::{eof, recognize},
    complete::take,
    multi::{many0, many0_count},
    sequence::{pair, tuple},
    Err, IResult,
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

fn parse_assignment(input: &str) -> IResult<&str, Pair> {
    let res = tuple((parse_variable, parse_equal, parse_value))(input);
    match res {
        Ok((input, (var, _, val))) => {
            Ok((input, HashMap::from([(var.to_string(), val.to_string())])))
        }
        Err(e) => Err(e),
    }
}

fn parse_main(input: &str) -> IResult<&str, Vec<Pair>> {
    many0(parse_assignment)(input)
}

fn main() -> Result<(), AppError> {
    let content = r##"street_name =Random
countryCode =  NL
demo=demo4"##;

    let res = parse_main(content)?;

    println!("Trying to parse: {}", content);
    dbg!(res);

    Ok(())
}
