# 3. Rules and defaults

[← Back to index](README.md)

When a flag needs to depend on context — who the user is, where they are, what
plan they're on — swap the short form for a **block**. A block uses braces and
holds a list of rules:

```flagfile
FF-feature-y {
    // if country is NL return true
    lower(countryCode) == nl -> true
    // else default to false
    false
}
```

## How a block evaluates

- Each rule is `condition -> value`.
- Rules are evaluated **top to bottom**; the **first** rule whose condition is
  true wins and its value is returned. No later rules are checked.
- A bare value with no `condition ->` always matches. Put one at the end as the
  **default** (fallthrough) — it's what the flag returns when nothing else
  matched.

So in `FF-feature-y` above: if `countryCode` lowercases to `nl`, the flag is
`true`; otherwise it falls through to `false`.

## A block can be just a default

If you only have a default, that's a valid block too — useful when you know more
rules are coming:

```flagfile
FF-testing {
    // default variant
    json({"success": true})
}
```

## Multiple rules and mixed variants

Rules are tried in order, and each may return a different value — including
different JSON shapes. Conditions can be as rich as you like (the operators are
covered in the next pages):

```flagfile
FF-feature-complex-ticket-234234 {
    @name "segment match"
    segment(complex_segment_test) -> TRUE

    // @name demo car opt-out
    z == "demo car" -> FALSE

    // with checking more
    g in (4,5,6) and z == "demo car" -> json({"success": true})

    // and a multi-line rule works
    model in (ms,mx,m3,my) and created >= 2024-01-01
        and demo == false -> TRUE

    FALSE
}
```

Two things to notice:

- **Multi-line rules** — a condition can wrap across lines; it ends at the `->`.
- **Order matters** — because the first match wins, put more specific rules
  above more general ones.

## Naming a rule with `@name`

Annotate a rule to document what it's for. Both forms work and are equivalent:

```flagfile
@name "segment match"
segment(complex_segment_test) -> TRUE

// @name demo car opt-out
z == "demo car" -> FALSE
```

`@name` applies to the rule that follows it. It's metadata only — it doesn't
change evaluation — and is meant for conditional rules rather than the bare
default.

Next: [Comparisons →](04-comparisons.md)
