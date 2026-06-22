# 6. String matching

[← Back to index](README.md)

Beyond exact equality, you can match strings by substring, prefix, suffix, or
regular expression. Each operator has a negated counterpart.

| Operator | Meaning            | Negated |
|----------|--------------------|---------|
| `~`      | contains           | `!~`    |
| `^~`     | starts with        | `!^~`   |
| `~$`     | ends with          | `!~$`   |

## Contains

```flagfile
FF-contains-feature-check {
    lower(name) ~ nik -> true   // contains
    name !~ Nik -> false        // does not contain
    false
}
```

## Starts with / ends with

```flagfile
FF-admin-path-check {
    path ^~ "/admin" -> true    // starts with
    false
}

FF-email-domain-check {
    email ~$ "@company.com" -> true   // ends with
    false
}
```

## Regex

The `~` and `!~` operators also accept a **regex literal**, written between
slashes (`/ … /`). Regex works *only* with `~` and `!~`, not with the
prefix/suffix operators.

```flagfile
FF-regexp-feature-check {
    UPPER(name) ~ /.*OLA.*/ -> true   // matches regex
    name !~ /.*ola.*/ -> false        // does not match
    false
}
```

## Notes

- The right-hand side can be a quoted string, a bare word (treated as text), or
  a `/regex/`.
- Matching against a variable that isn't in context is false.
- Combine with `lower()` / `upper()` (see [Functions](09-functions.md)) for
  case-insensitive matching, as in the `lower(name) ~ nik` example above.

Next: [Arrays and membership →](07-arrays-membership.md)
