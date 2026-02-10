# flagfile-cli

Command-line tool for working with Flagfile feature flags.

## Installation

```bash
brew install dzhibas/tap/flagfile-cli
```

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

# Filter by flag name substring
ff find src/ -s demo
ff find . --search premium
```

Output is in grep-style `file:line:content` format.

## Serving flags over HTTP

Start an HTTP server to evaluate flags via REST API:

```bash
ff serve
ff serve -f path/to/Flagfile -p 3000
```

### Configuration

The server reads an optional `ff.toml` config file (default path: `ff.toml` in the current directory). CLI flags override config values.

```toml
port = 8080
flagfile = "Flagfile"
```

You can specify a different config path with `-c`:

```bash
ff serve -c path/to/ff.toml
```

Defaults when no config or flags are provided: port `8080`, flagfile `Flagfile`.

### Endpoints

**`GET /flagfile`** — returns the raw Flagfile content as `text/plain`.

```bash
curl http://localhost:8080/flagfile
```

**`GET /eval/:flag_name`** — evaluates a flag using query parameters as context.

```bash
# Flag with no context
curl http://localhost:8080/eval/FF-welcome-banner
# {"flag":"FF-welcome-banner","value":true}

# Flag with context
curl "http://localhost:8080/eval/FF-premium-feature?plan=premium"
# {"flag":"FF-premium-feature","value":true}
```

Returns `404` if the flag is not found, `422` if no rule matched the given context.

---

## TODO

- [x] ff init
- [x] ff validate
- [x] ff test
- [x] ff list
- [x] ff eval
- [x] ff find
- [x] ff serve
- [ ] ff merge -- if we want to merge per environment flagfile values? aka Flagfile.stage vs Flagfile.local vs Flagfile.prod into Flagfile
- [ ] ff fmt — formats Flagfile
- [ ] ff edit — opens browser UI with simple editor of Flagfile

distributed functions

- [ ] ff push - pushes flags with namespace into distributed service (ff serve with raft consensus with one writer)
                same ff push would be used in git action automation where each merge into main or release branch
                would trigger:
                    ff validate
                    ff test
                    ff push --server x --namespace y --version git-hash
- [ ] ff rollback - --namespace y --to-version previous-git-hash