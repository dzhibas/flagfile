# 13. Annotations

[← Back to index](README.md)

Annotations attach **metadata** to a flag — ownership, lifecycle, dependencies,
documentation. They go on the lines immediately **before** the flag definition,
in any order, and you can stack as many as you need.

```flagfile
@ticket "JIRA-1234"
@description "GEO feature based on country or region"
FF-geo-features {
    coalesce(countryCode, region, "unknown") == "NL" -> true
    false
}
```

## Available annotations

| Annotation     | Value             | Purpose |
|----------------|-------------------|---------|
| `@owner`       | quoted string     | who owns the flag |
| `@description` | string / rest of line | what the flag does |
| `@ticket`      | quoted string     | tracking-system reference |
| `@type`        | bare identifier   | category, e.g. `experiment`, `release` |
| `@expires`     | date              | intended removal date |
| `@deprecated`  | quoted string     | deprecation note, often a replacement |
| `@requires`    | flag name         | prerequisite flag (repeatable) |

## Examples

Ownership and expiry:

```flagfile
@expires 2027-01-01
@owner "Nikolajus"
FF-sdk-upgrade { ... }
```

Type:

```flagfile
@type experiment
FF-launch-event { ... }
```

Deprecation, paired with an expiry:

```flagfile
@deprecated "Use FF-new-checkout instead"
@expires 2027-01-01
FF-old-checkout -> true
```

## Dependencies with `@requires`

`@requires` declares that a flag depends on another flag being on. It's
**repeatable** — list it once per dependency. The dependent flag only takes
effect when its prerequisites are satisfied:

```flagfile
FF-dep-root-new-checkout -> true

// this flag only evaluates if FF-dep-root-new-checkout is true
@requires FF-dep-root-new-checkout
FF-checkout-upsell {
    userId in (20, 21, 22) -> true
    false
}
```

The prerequisite must itself be a valid `FF-` / `FF_` flag name.

## Annotations in comments

You'll also see annotations written inside comments (e.g. `// @author …`). These
are documentary and are not parsed as flag metadata — the bare `@annotation`
form above is the one the parser reads. The one exception worth its own page is
`@test`, covered next.

Next: [Tests →](14-tests.md)
