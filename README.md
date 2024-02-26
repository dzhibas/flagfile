# ![logo](https://github.com/dzhibas/flagfile/blob/main/public/ff.png?raw=true) Flagfile

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
