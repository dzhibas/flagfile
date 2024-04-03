# <img src="https://github.com/dzhibas/flagfile/blob/main/public/ff.png?raw=true" width=50px/> Flagfile

![Build and Tests](https://github.com/dzhibas/flagfile/actions/workflows/rust.yml/badge.svg)

it's developer friendly feature flagging solution where you define all your flags in Flagfile in this format: [Flagfile.example](Flagfile.example)

its boolean expression parser library which was initially written in pest.rs (https://github.com/dzhibas/bool_expr_parser) and later rewrote everything in Nom rust lib

Feature rules can be describe in a expresions similar to all developers and DevOps and does not need any intermediate json format to express these
```
country == NL and created > 2024-02-15 and userId not in (122133, 122132323, 2323423)
```

```rust
let rule = "country == NL and created > 2024-02-15 and userId not in (122133, 122132323, 2323423)";
let (i, expr) = parse(&rule).expect("parse error");
let flag_value = eval(&expr, &HashMap::from([("country", "NL"), ("userId", "2132321"), ("created", "2024-02-02")]);
dbg!(flag_value);
```

eventually this lib compiles into wasm and used in UI to validate and parse rules, and with FFI exported into other languages to parse and evaluate rules

## Flagfile

it's a flagfile in your application root folder to control behaviour and feature flagging in your app

Flagfile.example (with comments):

```cpp
// once you dont have rules you can use short notation to return boolean
FF-feature-flat-on-off -> true

// you can return non-boolean in this example json. or empty json object json({})
FF-feature-json-variant -> json({"success": true})

// features are forced to start with FF- case-sensitive as
// it allows you later to find all flags through the codebase
FF-feature-name-specifics -> false

// you can have feature with multiple rules in it with default flag value returned in the end
// you can have comments or comment blocks with // or /* comment */
FF-feature-y {
    // if country is NL return True
    countryCode == NL: true
    // else default to false
    false
}

// you can also return different variations (non-boolean) as example json
FF-testing {
    // default variant
    json({"success": true})
}

// and have more complex feature with multiple rules in it and some rules multiline rule, which at the end defaults to false
// aswel capitalize for visibility boolean TRUE/FALSE
FF-feature-complex-ticket-234234 {
    // complex bool expression
    a = b and c=d and (dd not in (1,2,3) or z == "demo car"): TRUE

    // another one
    z == "demo car": FALSE

    // with checking more
    g in (4,5,6) and z == "demo car": TRUE

    // and multi-line rule works
    model in (ms,mx,m3,my) and created >= 2024-01-01
        and demo == false: TRUE

    FALSE
}

// different kind of comments inside
FF-feature1 {
    /* comment like this */
    true
    a == "something": false
    false
    json({})
}

/* this is multi-line commented feature
FF-timer-feature {
    // turn on only on evaluation time after 22nd feb
    NOW() > 2024-02-22: true
    false
}
*/
```
