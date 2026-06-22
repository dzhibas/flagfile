# 11. Segments

[← Back to index](README.md)

A **segment** is a named, reusable condition. Define it once, then reference it
from any flag — handy when the same audience definition shows up in several
flags.

## Define a segment

Use `@segment name { … }` at the top level of the file. The body is a single
boolean expression, using all the operators you've already seen:

```flagfile
@segment complex_segment_test {
    a = b and c = d and (dd not in (1,2,3) or z == "demo car")
}
```

Segment names allow letters, digits, `_`, and `-`.

## Use a segment

Reference it inside a rule with `segment(name)`. It evaluates to the segment's
boolean result against the current context:

```flagfile
FF-feature-complex-ticket-234234 {
    @name "segment match"
    segment(complex_segment_test) -> TRUE
    FALSE
}
```

## Compose with other logic

`segment(...)` is an ordinary boolean expression, so combine it freely with
comparisons, `percentage()`, and logic operators:

```flagfile
segment(premium_users) and percentage(50%, userId) -> true
```

If a referenced segment isn't defined, it evaluates to false.

Next: [Environments →](12-environments.md)
