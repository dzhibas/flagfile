# Flagfile Syntax Guide

A learning path for the **Flagfile** feature-flag DSL — start at the top and
work down. Each page builds on the previous one, from the simplest possible
flag (`FF_simple -> true`) up to segments, environments, and metadata.

Every snippet here is drawn from the canonical, test-backed
[`Flagfile.example`](../../Flagfile.example), so what you read is real, valid
syntax. The same grammar is implemented identically in the Rust library and the
TypeScript port — there is one Flagfile language.

## Learning path

1. [Getting started](01-getting-started.md) — your first flag, naming rules, comments
2. [Return types](02-return-types.md) — booleans, numbers, strings, JSON variants
3. [Rules and defaults](03-rules-and-defaults.md) — block form, conditions, fallthrough
4. [Comparisons](04-comparisons.md) — operators and value types (dates, semver, …)
5. [Logic and grouping](05-logic-and-grouping.md) — `and` / `or` / `not`, parentheses
6. [String matching](06-string-matching.md) — contains, starts/ends-with, regex
7. [Arrays and membership](07-arrays-membership.md) — `in` / `not in`
8. [Null checks](08-null-checks.md) — `is null` / `is not null`
9. [Functions](09-functions.md) — `lower`, `upper`, `now`, `coalesce`
10. [Percentage rollouts](10-percentage-rollouts.md) — gradual, deterministic rollout
11. [Segments](11-segments.md) — named, reusable conditions
12. [Environments](12-environments.md) — per-environment behavior with `@env`
13. [Annotations](13-annotations.md) — metadata: owner, expiry, dependencies, …
14. [Tests](14-tests.md) — `@test` assertions and running them
15. [Includes](15-includes.md) — composing a Flagfile from multiple files with `@include`

## Mental model

A Flagfile is a list of **flags**. A flag either returns a value directly:

```flagfile
FF-new-ui -> true
```

…or opens a block of **rules** evaluated top-to-bottom, where the first matching
rule wins and a bare value at the end is the default:

```flagfile
FF-feature-y {
    lower(countryCode) == nl -> true
    false
}
```

A rule is `condition -> value`. Conditions are boolean **expressions** built from
context variables, operators, and functions. Expressions bind in this order:

> atoms → functions → comparisons / arrays / matches → negation / parentheses → logic

## Operator cheat-sheet

| Category    | Operators |
|-------------|-----------|
| Comparison  | `==` `=` &nbsp; `!=` `<>` &nbsp; `>` `>=` `<` `<=` |
| Logic       | `and` `&&` &nbsp; `or` `\|\|` &nbsp; `not` `!` |
| Membership  | `in` &nbsp; `not in` |
| String match| `~` (contains) &nbsp; `!~` &nbsp; `^~` (starts) &nbsp; `!^~` &nbsp; `~$` (ends) &nbsp; `!~$` |
| Null        | `is null` &nbsp; `is not null` |
| Functions   | `lower()` `upper()` `now()` `coalesce()` `segment()` `percentage()` |
| Grouping    | `( … )` &nbsp; `not ( … )` &nbsp; `!( … )` |

Next: [Getting started →](01-getting-started.md)
