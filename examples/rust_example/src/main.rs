use flagfile_lib::{Context, ast::Atom, ff};
use std::collections::HashMap;

fn main() {
    flagfile_lib::init_with_env("stage");

    let ctx: Context = HashMap::from([("tier", "premium".into()), ("country", "nl".into())]);
    let flag: bool = ff("FF-feature-y", &ctx).expect("Flag not found").into();

    if flag {
        println!("FF-feature-y flag is on");
    } else {
        println!("FF-feature-y flag is off");
    }

    let l = vec!["viewer".into(), "editor".into(), "admin".into()];
    let ctx = HashMap::from([("roles", Atom::List(l))]);
    if ff("FF-admin-panel", &ctx).expect("error").into() {
        println!("FF-admin-panel flag is ON");
    }
}
