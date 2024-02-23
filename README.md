# Boolean expression parser in Nom for feature flagging solution

Same as pest parser written with pest.rs here https://github.com/dzhibas/bool_expr_parser but parsed with NOM

Lets say you have activation rule likes this:

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

### Vision for Flagfile example

Please take a look at format and supported features in [Flagfile.example](Flagfile.example)
