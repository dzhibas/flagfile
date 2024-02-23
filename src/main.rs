use nom::{
    branch::alt,
    bytes::complete::{tag, take_until},
    character::complete::{alpha1, alphanumeric1, line_ending, multispace0, space0, char},
    combinator::{eof, recognize},
    complete::take,
    multi::{many0, many0_count, many_till},
    sequence::{pair, preceded, tuple},
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

fn parse_variable_clean_spaces(input: &str) -> IResult<&str, &str> {
    preceded(multispace0, parse_variable)(input)
}

fn parse_equal(input: &str) -> IResult<&str, (&str, &str, &str)> {
    tuple((space0, tag("="), space0))(input)
}

fn parse_string_value(i: &str) -> IResult<&str, &str> {
    let (tail, (_, str, _)) = tuple((char('"'), take_until("\""), char('"')))(i)?;
    Ok((tail, str))
}

fn parse_assignment(input: &str) -> IResult<&str, Pair> {
    let res = tuple((parse_variable_clean_spaces, parse_equal, parse_string_value))(input);
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
    let content = r##"street_name ="Random this or that"
countryCode = "NL"
demo="demo4""##;

    let res = parse_main(content)?;

    println!("Trying to parse: {}", content);
    dbg!(res);

    Ok(())
}
