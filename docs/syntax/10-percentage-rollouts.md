# 10. Percentage rollouts

[← Back to index](README.md)

To ship a feature to a fraction of users — a canary or gradual rollout — use
`percentage()`.

```flagfile
FF-new-checkout {
    percentage(50%, userId) -> true
    false
}
```

This turns the flag on for roughly 50% of users, bucketed by `userId`.

## Signature

```
percentage( rate% , field [, "salt"] )
```

- **`rate%`** — the share to include, written with a `%` (e.g. `50%`, `10%`,
  `0%`, `100%`).
- **`field`** — the context variable used for bucketing (e.g. `userId`,
  `orgId`). The same field value always lands in the same bucket.
- **`salt`** *(optional)* — a quoted string that shifts the bucketing, so two
  flags rolling out at the same rate over the same field can affect different
  subsets.

## Deterministic and stable

Bucketing is a hash of the flag name plus the field value (and salt, if given),
so it's **deterministic**: the same user gets the same answer every time, across
processes and restarts. `0%` is always false; `100%` is always true.

## Combine with other conditions

Because it's just a boolean expression, `percentage()` composes with everything
else. Roll out to premium org members first, then a smaller slice of everyone:

```flagfile
FF-gradual-migration {
    percentage(50%, orgId) and plan == "premium" -> true
    percentage(10%, orgId) -> true
    false
}
```

Next: [Segments →](11-segments.md)
