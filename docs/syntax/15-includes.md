# 15. Includes

[← Back to index](README.md)

A Flagfile can be split across multiple files with `@include`. The directive
is replaced by the referenced file's content, merging everything into a single
flag set:

```flagfile
FF-root-feature -> true

@include Flagfile.demo
@include cua/Flagfile
```

Paths are resolved relative to the directory of the file containing the
`@include`. In the example above, if the main file is `/tmp/Flagfile`, then
`cua/Flagfile` resolves to `/tmp/cua/Flagfile`. Included files can contain
their own `@include` directives, resolved relative to *their* directory.

## Sandbox rules

Includes can never reach outside the directory of the including Flagfile:

- **No absolute paths** — `@include /etc/hosts` is rejected.
- **No `..` components** — `@include ../shared.ff` is rejected, even if the
  file exists.
- **No cycles** — a file that includes itself, directly or through a chain,
  is rejected.

## Validation, linting, and tests

`validate`, `lint`, `test`, and `check` all resolve includes first and **fail
if an included file is missing** or violates the sandbox rules.

The `test` command also runs the test sources of every included file:

- a sibling tests file next to the included file (e.g. `cua/Flagfile` →
  `cua/Flagfile.tests`), and
- inline `// @test` annotations inside the included file, reported with that
  file's own line numbers.

All assertions evaluate against the merged flag set, so tests in an included
file can reference flags from anywhere in the final result.

```bash
flagfile validate -f Flagfile   # lists resolved includes
flagfile check -f Flagfile      # validate + lint + test, includes included
```

## Notes

- Duplicate flag names across included files are caught by the
  `duplicate_flags` lint.
- `@include` inside `//` or `/* */` comments is ignored.
- In library code, includes are resolved when loading from a file path
  (`init()`, `init().file(...)`, `init_with_env()`). `init_from_str()` cannot
  resolve includes because a string has no directory.

Head [back to the index](README.md) for the full learning path.
