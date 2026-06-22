# 9. Functions

[← Back to index](README.md)

Functions transform or produce values inside an expression. They wrap a variable
(or take no argument) and can be used anywhere a value is expected — on either
side of a comparison or match.

## `lower()` / `upper()`

Normalize case before comparing or matching. Great for making conditions
case-insensitive:

```flagfile
FF-feature-y {
    lower(countryCode) == nl -> true
    false
}
```

```flagfile
UPPER(name) ~ /.*OLA.*/ -> true
```

Function names themselves are case-insensitive — `lower`, `LOWER`, `upper`,
`UPPER` all work.

## `now()`

Returns the current time at evaluation, with no arguments. Compare it against a
date or datetime to build time-based flags:

```flagfile
FF-timer-feature {
    // turn on only after 7th Feb 2026
    NOW() > 2026-02-07 -> true
    false
}
```

```flagfile
FF-launch-event {
    now() > 2025-06-15T09:00:00Z and now() < 2027-01-15T18:00:00Z -> true
    false
}
```

## `coalesce()`

Returns the **first non-null argument** — exactly like SQL's `COALESCE`. Takes
two or more arguments; the last is usually a literal default. Use it to fall
back across context variables:

```flagfile
FF-geo-features {
    // use countryCode if present, fall back to region, then "unknown"
    coalesce(countryCode, region, "unknown") == "NL" -> true
    false
}
```

With this flag: `countryCode=NL` → matches; only `region=NL` → matches; neither
provided → resolves to `"unknown"` and falls through to the default.

Two more function-like constructs deserve their own pages because they pull in
extra concepts:

- [`percentage()`](10-percentage-rollouts.md) — gradual rollout
- [`segment()`](11-segments.md) — reference a named, reusable condition

Next: [Percentage rollouts →](10-percentage-rollouts.md)
