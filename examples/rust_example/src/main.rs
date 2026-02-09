use flagfile_lib::{Context, FlagReturn, ff, init};
use std::collections::HashMap;

fn main() {
    flagfile_lib::init();

    let ctx: Context = HashMap::from([
        ("tier", "premium".into()),
        ("country", "NL".into()),
    ]);

    match ff("FF-feature-y", &ctx) {
        Some(FlagReturn::OnOff(true)) => println!("Flag is on"),
        Some(FlagReturn::OnOff(false)) => println!("Flag is off"),
        Some(FlagReturn::Json(v)) => println!("Config: {}", v),
        Some(FlagReturn::Integer(n)) => println!("Value: {}", n),
        Some(FlagReturn::Str(s)) => println!("String: {}", s),
        None => println!("Flag not found or no rule matched"),
    }
}
