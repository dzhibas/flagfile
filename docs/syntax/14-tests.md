# 14. Tests

[← Back to index](README.md)

You can keep tests for a flag right next to the flag, with `@test`. Each test
states a context and the value the flag should return — so the file documents
*and* verifies its own behavior.

```flagfile
@test FF-geo-features(region=NL) == true
@test FF-geo-features(countryCode=NL) == true
@test FF-geo-features() == false
FF-geo-features {
    coalesce(countryCode, region, "unknown") == "NL" -> true
    false
}
```

## Anatomy of a test

```
@test FlagName(param=value, param=value, …) == expected
```

- **`FlagName`** — the flag under test.
- **parameters** — the evaluation context, as `key=value` pairs. Empty
  parentheses `()` mean "no context", and a flag with no context at all can omit
  them entirely.
- **`expected`** — the value the flag should return (`true`, `false`, a string,
  a number, JSON, …).

Array parameters use bracket syntax, matching the reverse-`in` form:

```flagfile
@test FF-admin-panel(roles=["viewer", "editor", "admin"]) == true
```

## Where tests can live

`@test` works both as a bare annotation before the flag and inside either
comment style:

```flagfile
// @test FF-contains-feature-check(name="Nikolajus") == true
@test FF-contains-feature-check(name="Nikola") == true
FF-contains-feature-check { ... }
```

```flagfile
/**
 * test can also be in block comments
 * @test FF-flag-with-annotations-2 == false
 */
FF-flag-with-annotations-2 -> false
```

## Running tests

The CLI evaluates every `@test` in a file:

```bash
flagfile test -f Flagfile           # run all @test assertions
flagfile validate -f Flagfile       # check syntax only
flagfile lint -f Flagfile           # run lint rules
flagfile check -f Flagfile          # validate + test + lint together
```

You can also evaluate a single flag by hand:

```bash
flagfile eval FF-geo-features countryCode=NL
```

That's the whole language. Head [back to the index](README.md) for the full
learning path, or browse [`Flagfile.example`](../../Flagfile.example) to see
every feature together in one file.
