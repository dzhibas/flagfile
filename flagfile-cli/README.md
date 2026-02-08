# flagfile-cli

Command-line tool for working with Flagfile feature flags.

## Getting started

Initialize a new project with a sample Flagfile and test file:

```bash
ff init
```

This creates `Flagfile` and `Flagfile.tests` in the current directory with example flags and tests to get you started.

## Validating and testing

Validate your Flagfile syntax:

```bash
ff validate
ff validate -f path/to/Flagfile
```

Run your test assertions from `Flagfile.tests`:

```bash
ff test
ff test -f path/to/Flagfile -t path/to/Flagfile.tests
```

## Evaluating flags

DevOps and developers can evaluate a single flag with context to debug or verify behavior:

```bash
# Simple flag with no context
ff eval FF-welcome-banner

# Flag with context key=value pairs
ff eval FF-premium-feature plan=premium beta=true

# Custom flagfile
ff eval -f Flagfile.example FF-sdk-upgrade appVersion=6.0.0
```

## Discovering flags

List all flags defined in the Flagfile:

```bash
ff list
ff list -f path/to/Flagfile
```

Find all flag references across your source code (respects `.gitignore`):

```bash
ff find
ff find src/
```

Output is in grep-style `file:line:content` format.

---

## TODO

- [x] ff init
- [x] ff validate
- [x] ff test
- [x] ff list
- [x] ff eval
- [x] ff find
- [ ] ff fmt — formats Flagfile
- [ ] ff edit — opens browser UI with simple editor of Flagfile
