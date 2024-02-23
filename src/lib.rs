use wasm_bindgen::prelude::wasm_bindgen;

pub mod ast;
pub mod eval;
pub mod parse;
pub mod parse_flagfile;

#[wasm_bindgen]
pub fn parse_wasm(i: &str) -> String {
    let Ok((_i, tree)) = parse::parse(i) else {
        todo!()
    };
    let b = format!("{:?}", tree);
    b.to_string()
}
