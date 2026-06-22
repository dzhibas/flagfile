# 5. Logic and grouping

[← Back to index](README.md)

Combine several comparisons into one condition with logical operators.

## `and` / `or`

Both have word and symbol forms — pick whichever reads better:

- `and` or `&&`
- `or` or `||`

```flagfile
FF-gradual-migration {
    percentage(50%, orgId) and plan == "premium" -> true
    percentage(10%, orgId) -> true
    false
}
```

## Negation: `not` / `!`

Flip a condition with `not` or `!`. It's typically applied to a parenthesized
group:

```flagfile
not (countryCode == LT)
!(plan == premium)
```

## Grouping with parentheses

Parentheses group sub-expressions and override the default ordering. Nest them
as deeply as you need:

```flagfile
@segment complex_segment_test {
    a = b and c = d and (dd not in (1,2,3) or z == "demo car")
}
```

## Precedence — important

Logic operators chain **left to right** and are *not* split by `and`-before-`or`
precedence the way some languages do. So this:

```flagfile
a = b and c = d or e = f
```

parses as `(a = b and c = d) or e = f`. When you mix `and` and `or`, **add
parentheses** to make the grouping explicit and unambiguous:

```flagfile
a = b and (c = d or e = f)
```

For the full picture, expressions bind in this order — innermost first:

> atoms → functions → comparisons / arrays / matches → negation / parentheses → logic

Next: [String matching →](06-string-matching.md)
