use nom::{
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_until},
    character::complete::{alpha1, alphanumeric1, char, line_ending, multispace0, one_of, space0},
    combinator::{eof, map, map_res, recognize},
    complete::take,
    error::VerboseError,
    multi::{many0, many0_count, many_till},
    sequence::{delimited, pair, preceded, separated_pair, tuple},
    Err, IResult,
};
use wasm_bindgen::prelude::wasm_bindgen;

pub mod ast;
pub mod eval;
pub mod parse;

#[wasm_bindgen]
pub fn parse_wasm(i: &str) -> String {
    let Ok((i, tree)) = parse::parse(i) else {
        todo!()
    };
    let b = format!("{:?}", tree);
    b.to_string()
}
