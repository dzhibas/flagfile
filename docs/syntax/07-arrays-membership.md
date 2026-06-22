# 7. Arrays and membership

[← Back to index](README.md)

Check whether a value is one of several options using `in` and `not in`.

## `in` — value in a list

The list is a parenthesized, comma-separated set of values. Items can be
numbers, quoted strings, or bare words:

```flagfile
FF-checkout-upsell {
    userId in (20, 21, 22) -> true
    false
}
```

```flagfile
model in (ms, mx, m3, my) -> true
```

## `not in`

The negation — true when the value is absent from the list:

```flagfile
dd not in (1, 2, 3) -> true
```

## Reverse form — value in an array variable

The other direction is just as useful: check whether a literal is contained in
an **array** supplied by the context. Here the context provides
`roles = ["viewer", "editor", "admin"]`:

```flagfile
FF-admin-panel {
    // roles is an array: ["viewer", "editor", "admin"]
    "admin" in roles -> true
    false
}
```

So `in` works both ways:

- `variable in (literal, list)` — is the variable one of these values?
- `"literal" in variable` — does this array variable contain the literal?

Next: [Null checks →](08-null-checks.md)
