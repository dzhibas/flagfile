use std::collections::HashMap;

use flagfile_lib::ast::Atom;
use flagfile_lib::eval::Context;
use flagfile_lib::parse_flagfile::parse_flagfile;
use flagfile_lib::{self, eval::eval, parse::parse};

#[test]
fn test_hashmap_into() {
    let out: Context = HashMap::from([("demo", "demo".into())]);
    assert_eq!(true, out.get("demo").is_some());
    assert_eq!(Atom::String("demo".into()), *out.get("demo").unwrap());
}

#[test]
fn test_parsing() {
    let (_i, expr) = parse("!(a=b and c=d) and z=3").unwrap();
    assert_eq!(
        true,
        eval(
            &expr,
            &HashMap::from([("a", "d".into()), ("c", "b".into()), ("z", "3".into()),]),
            None,
        )
        .unwrap()
    );
}

#[test]
fn test_evaluation() {
    let rule = r###"
accountRole in (Admin,admin,"Admin/Order Manager")
    and upper(account_country_code) in (LT , NL, DE, GB, US)
    and account_uuid in ("543987b0-e69f-41ec-9a68-cfc5cfb15afe", "6133b8d6-4078-4270-9a68-fa0ac78bf512")
    and accountType in ("Some Corporate & Managament Type", Corporate , Managament)
    and user_id <= 2032313"###;

    let context = HashMap::from([
        ("accountRole", Atom::String("Admin/Order Manager".into())),
        ("account_country_code", Atom::String("lt".into())),
        (
            "account_uuid",
            Atom::String("543987b0-e69f-41ec-9a68-cfc5cfb15afe".into()),
        ),
        (
            "accountType",
            Atom::String("Some Corporate & Managament Type".into()),
        ),
        ("user_id", Atom::Number(2032312)),
    ]);

    let (i, expr) = parse(&rule).unwrap();
    let val = eval(&expr, &context, None).unwrap();
    assert_eq!(val, true);
    assert_eq!(i, ""); // empty remainder of parsed string
}

#[test]
fn scoped_test_case() {
    let rule = r###"(accountRole in (Admin, "Admin/Order Manager")) and
    ((lower(account_country_code) == lt or account_uuid = 32434) and accountType="Some Corporate & Management Type") and user_id == 2032312"###;

    let context = HashMap::from([
        ("accountRole", Atom::String("Admin/Order Manager".into())),
        ("account_country_code", Atom::String("LT".into())),
        (
            "account_uuid",
            Atom::String("543987b0-e69f-41ec-9a68-cfc5cfb15afe".into()),
        ),
        (
            "accountType",
            Atom::String("Some Corporate & Management Type".into()),
        ),
        ("user_id", Atom::Number(2032312)),
    ]);

    let (i, expr) = parse(&rule).unwrap();
    let val = eval(&expr, &context, None).unwrap();
    assert_eq!(val, true);
    assert_eq!(i, ""); // empty remainder of parsed string
}

#[test]
fn scopes_bug_test() {
    let rule = "(a=1 or b=2) and ((c=3 or d=4) and e=5)";
    let context = HashMap::from([
        ("a", Atom::Number(1)),
        ("b", Atom::Number(2)),
        ("c", Atom::Number(3)),
        ("d", Atom::Number(4)),
        ("e", Atom::Number(5)),
    ]);
    let (i, expr) = parse(rule).unwrap();
    let val = eval(&expr, &context, None).unwrap();
    assert_eq!(val, true);
    assert_eq!(i, "");
}

#[test]
fn scopes_bug_with_new_lines_around_test() {
    let rule = r###"
            (a=1 or b=2) and ((c=3 or d=4) and e=5)
"###;
    let context = HashMap::from([
        ("a", Atom::Number(1)),
        ("b", Atom::Number(2)),
        ("c", Atom::Number(3)),
        ("d", Atom::Number(4)),
        ("e", Atom::Number(5)),
    ]);
    let (i, expr) = parse(rule).unwrap();
    let val = eval(&expr, &context, None).unwrap();
    assert_eq!(val, true);
    assert_eq!(i, "");
}

#[test]
fn semver_comparison_test() {
    // Test: version > 5.3.42
    let rule = "appVersion > 5.3.42 and platform == ios";
    let context = HashMap::from([
        ("appVersion", Atom::Semver(6, 0, 0)),
        ("platform", Atom::String("ios".into())),
    ]);
    let (i, expr) = parse(rule).unwrap();
    assert_eq!(i, "");
    assert_eq!(eval(&expr, &context, None).unwrap(), true);

    // version 5.3.42 is NOT > 5.3.42
    let context_equal = HashMap::from([
        ("appVersion", Atom::Semver(5, 3, 42)),
        ("platform", Atom::String("ios".into())),
    ]);
    assert_eq!(eval(&expr, &context_equal, None).unwrap(), false);

    // Test: version < 4.32.0
    let rule2 = "appVersion < 4.32.0";
    let (i2, expr2) = parse(rule2).unwrap();
    assert_eq!(i2, "");
    assert_eq!(
        eval(
            &expr2,
            &HashMap::from([("appVersion", Atom::Semver(4, 31, 9))]),
            None,
        )
        .unwrap(),
        true
    );
    assert_eq!(
        eval(
            &expr2,
            &HashMap::from([("appVersion", Atom::Semver(4, 32, 0))]),
            None,
        )
        .unwrap(),
        false
    );
    assert_eq!(
        eval(
            &expr2,
            &HashMap::from([("appVersion", Atom::Semver(5, 0, 0))]),
            None,
        )
        .unwrap(),
        false
    );
}

#[test]
fn semver_from_str_context_test() {
    // Semver values provided as strings via From<&str> should parse as Semver
    let context: Context = HashMap::from([("version", "2.1.0".into())]);
    assert_eq!(*context.get("version").unwrap(), Atom::Semver(2, 1, 0));

    let (_, expr) = parse("version >= 2.0.0").unwrap();
    assert_eq!(eval(&expr, &context, None).unwrap(), true);
}

#[test]
fn contains_and_regex_match_test() {
    // contains match
    let (i, expr) = parse("name ~ Nik").unwrap();
    assert_eq!(i, "");
    assert_eq!(
        eval(
            &expr,
            &HashMap::from([("name", Atom::String("Nikolajus".into()))]),
            None,
        )
        .unwrap(),
        true
    );

    // contains match with function call
    let (i, expr) = parse("lower(name) ~ nik").unwrap();
    assert_eq!(i, "");
    assert_eq!(
        eval(
            &expr,
            &HashMap::from([("name", Atom::String("NIKOLAJUS".into()))]),
            None,
        )
        .unwrap(),
        true
    );

    // not-contains match
    let (i2, expr2) = parse("name !~ Nik").unwrap();
    assert_eq!(i2, "");
    assert_eq!(
        eval(
            &expr2,
            &HashMap::from([("name", Atom::String("John".into()))]),
            None,
        )
        .unwrap(),
        true
    );

    // regex match
    let (i3, expr3) = parse("name ~ /.*ola.*/").unwrap();
    assert_eq!(i3, "");
    assert_eq!(
        eval(
            &expr3,
            &HashMap::from([("name", Atom::String("Nikolajus".into()))]),
            None,
        )
        .unwrap(),
        true
    );

    // not-regex match
    let (i4, expr4) = parse("lower(name) !~ /.*ola.*/").unwrap();
    assert_eq!(i4, "");
    assert_eq!(
        eval(
            &expr4,
            &HashMap::from([("name", Atom::String("Simonas".into()))]),
            None,
        )
        .unwrap(),
        true
    );

    // regex match with utf8 chars
    let (i4, expr4) = parse("lower(name) ~ /.*žolė.*/").unwrap();
    assert_eq!(i4, "");
    assert_eq!(
        eval(
            &expr4,
            &HashMap::from([("name", Atom::String("Kažkur ŽOLĖ žalesnė".into()))]),
            None,
        )
        .unwrap(),
        true
    );
}

#[test]
fn starts_with_and_ends_with_test() {
    // startsWith
    let (i, expr) = parse("path ^~ \"/admin\"").unwrap();
    assert_eq!(i, "");
    assert_eq!(
        eval(
            &expr,
            &HashMap::from([("path", Atom::String("/admin/settings".into()))]),
            None,
        )
        .unwrap(),
        true
    );
    assert_eq!(
        eval(
            &expr,
            &HashMap::from([("path", Atom::String("/user/profile".into()))]),
            None,
        )
        .unwrap(),
        false
    );

    // endsWith
    let (i2, expr2) = parse("email ~$ \"@company.com\"").unwrap();
    assert_eq!(i2, "");
    assert_eq!(
        eval(
            &expr2,
            &HashMap::from([("email", Atom::String("user@company.com".into()))]),
            None,
        )
        .unwrap(),
        true
    );
    assert_eq!(
        eval(
            &expr2,
            &HashMap::from([("email", Atom::String("user@other.com".into()))]),
            None,
        )
        .unwrap(),
        false
    );

    // notStartsWith
    let (_, expr3) = parse("name !^~ \"test\"").unwrap();
    assert_eq!(
        eval(
            &expr3,
            &HashMap::from([("name", Atom::String("production".into()))]),
            None,
        )
        .unwrap(),
        true
    );
    assert_eq!(
        eval(
            &expr3,
            &HashMap::from([("name", Atom::String("testing123".into()))]),
            None,
        )
        .unwrap(),
        false
    );

    // notEndsWith
    let (_, expr4) = parse("name !~$ \".tmp\"").unwrap();
    assert_eq!(
        eval(
            &expr4,
            &HashMap::from([("name", Atom::String("file.txt".into()))]),
            None,
        )
        .unwrap(),
        true
    );
    assert_eq!(
        eval(
            &expr4,
            &HashMap::from([("name", Atom::String("data.tmp".into()))]),
            None,
        )
        .unwrap(),
        false
    );

    // combined with function: lower(name) ^~ "admin"
    let (_, expr5) = parse("lower(name) ^~ \"admin\"").unwrap();
    assert_eq!(
        eval(
            &expr5,
            &HashMap::from([("name", Atom::String("ADMIN_USER".into()))]),
            None,
        )
        .unwrap(),
        true
    );
}

#[test]
fn flagfile_with_semver_parses() {
    let data = include_str!("../Flagfile.example");
    let (i, v) = parse_flagfile(data).unwrap();
    assert!(v.len() > 0);
    assert_eq!(i.to_string().trim(), "");
}
