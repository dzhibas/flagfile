use flagfile_lib::{Context, ff};
use std::collections::HashMap;

fn main() {
    flagfile_lib::init();

    let ctx: Context = HashMap::from([("tier", "premium".into()), ("country", "nl".into())]);

    let flag: bool = ff("FF-feature-y", &ctx).expect("Flag not found").into();

    if flag {
        println!("Flag is on");
    } else {
        println!("Flag is off");
    }
}
