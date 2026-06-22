# 4. Comparisons

[← Back to index](README.md)

The heart of a rule's condition is a comparison: take a **context variable** on
the left, compare it against a **value** on the right.

```flagfile
FF-checkout-upsell {
    userId in (20, 21, 22) -> true
    false
}
```

A context variable (like `userId`, `countryCode`, `appVersion`) is supplied by
your application when it evaluates the flag. If a variable isn't provided, a
comparison against it is simply false (see [Null checks](08-null-checks.md) to
test for presence explicitly).

## Operators

| Operator     | Meaning              |
|--------------|----------------------|
| `==` or `=`  | equal                |
| `!=` or `<>` | not equal            |
| `>`          | greater than         |
| `>=`         | greater or equal     |
| `<`          | less than            |
| `<=`         | less or equal        |

```flagfile
appVersion >= 5.3.42 -> true
appVersion < 4.32.0  -> false
```

## Value types you can compare against

The right-hand side can be any of these literal types:

| Type      | Example                  | Notes |
|-----------|--------------------------|-------|
| String    | `"premium"` / `'premium'`| double or single quotes |
| Number    | `5000`, `-3`             | integers |
| Float     | `3.14`                   | requires a decimal point |
| Boolean   | `true`, `FALSE`          | case-insensitive |
| Date      | `2024-01-01`             | `YYYY-MM-DD` |
| DateTime  | `2025-06-15T09:00:00Z`   | `YYYY-MM-DDTHH:MM:SS`, optional `Z` |
| Semver    | `5.3.42`                 | three dot-separated integers |

Dates and datetimes compare chronologically, which makes time windows easy:

```flagfile
FF-feature-x {
    created > 2024-02-02 and created <= 2024-02-13 -> true
    false
}
```

## Semantic versioning

Three-part versions (`major.minor.patch`) compare component-by-component, not as
text — so `5.3.42 < 5.10.0` as you'd expect:

```flagfile
FF-sdk-upgrade {
    // enable new SDK for apps on 5.3.42 or higher
    appVersion >= 5.3.42 -> true
    // disable for apps below 4.32.0
    appVersion < 4.32.0 -> false
    false
}
```

A note on coercion: a two-part number like `5.4` is a float, and it compares
against a semver as `5.4.0`. So `appVersion >= 5.3.42` is true when
`appVersion` is `5.4`.

Next: [Logic and grouping →](05-logic-and-grouping.md)
