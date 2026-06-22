# 8. Null checks

[← Back to index](README.md)

Sometimes the question is simply *was this variable provided at all?* Use
`is null` and `is not null`.

```flagfile
FF-null-check-flag-demo {
    userId is null -> false
    userId is not null and plan == premium -> true
    false
}
```

## What it does

- `variable is null` — true when the variable is **absent** from the evaluation
  context.
- `variable is not null` — true when the variable **is present**.

Both keyword phrases are case-insensitive (`IS NULL`, `is not null`, …).

This pairs naturally with logic operators, as above: only grant the flag when
`userId` is present *and* the plan is premium. It's also the explicit way to
guard a rule, since a plain comparison against a missing variable is just false
rather than an error.

Next: [Functions →](09-functions.md)
