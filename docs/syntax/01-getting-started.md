# 1. Getting started

[← Back to index](README.md)

The simplest flag is a name and a value, joined by an arrow `->`. No braces, no
rules — just an on/off switch.

```flagfile
FF-new-ui -> true
FF-beta-features -> false
FF-maintenance-mode -> false
```

That's it. `FF-new-ui` always returns `true`; the others always return `false`.
This short form is what you reach for when a flag has no conditions yet.

## Flag names

Every flag name **must** start with `FF-` or `FF_`. This prefix is
case-sensitive (`ff-` is not valid) and is intentional: it makes flags trivial
to find across a codebase with flagfile-cli: `ff find` in repository

After the prefix, names can use letters, digits, `-`, and `_`, in whatever
casing convention you like:

```flagfile
FF-feature-name-specifics -> false        // kebab-case
FF_feature_can_be_snake_case_213213 -> FALSE   // snake_case
FF_featureOneOrTwo -> FALSE                // camelCase
FF_Feature23432 -> TRUE                    // PascalCase
```

One subtlety: a `-` directly before the arrow is ambiguous with `->`, so keep a
character (or a space) between them. Both of these are fine:

```flagfile
FF-dep-root-new-checkout -> true
FF-dep-root-new-checkout->true
```

## Comments

Use comments freely to explain intent. Two styles are supported, and both can
appear almost anywhere — before a flag, before a rule, or inside a block.

```flagfile
// a single-line comment

/* a block comment on one line */

/*
   a block comment
   spanning multiple lines
*/
```

## Whitespace

Whitespace is flexible. Spaces, tabs, and newlines are insignificant outside of
quoted strings, so you can format for readability. The arrow may or may not have
spaces around it, and rules may wrap across lines (you'll see multi-line rules
later in [Rules and defaults](03-rules-and-defaults.md)).

Next: [Return types →](02-return-types.md)
