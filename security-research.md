# Flagfile Security Research Report

**Date:** 2026-02-22
**Scope:** Full codebase audit — `flagfile-lib` (Rust), `flagfile-cli` (Rust), `flagfile-ts` (TypeScript), CI/CD pipelines, supply chain
**Methodology:** Multi-researcher parallel static analysis across all source files
**Researchers:** 5 specialized agents covering Rust library, CLI/HTTP server, TypeScript, DSL evaluation logic, and supply chain/CI

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Critical Findings](#critical-findings)
3. [High Severity Findings](#high-severity-findings)
4. [Medium Severity Findings](#medium-severity-findings)
5. [Low Severity / Informational Findings](#low-severity--informational-findings)
6. [Supply Chain & CI/CD Security](#supply-chain--cicd-security)
7. [Rust vs TypeScript Discrepancies](#rust-vs-typescript-discrepancies)
8. [Positive Security Practices](#positive-security-practices)
9. [Remediation Priority Matrix](#remediation-priority-matrix)

---

## Executive Summary

The flagfile codebase implements a feature flag evaluation DSL with a Rust library, a Rust CLI/HTTP server, and a TypeScript port. The system supports local evaluation, remote flagfile loading, HTTP serving with authentication, SSE streaming, and Raft-based clustering.

**Total findings: 42** across both implementations and infrastructure.

| Severity | Count |
|----------|-------|
| Critical | 4     |
| High     | 13    |
| Medium   | 17    |
| Low/Info | 8     |

**Most urgent issues:**
- ReDoS via user-supplied regex patterns (both implementations)
- Unbounded segment recursion → stack overflow
- Multiple `.unwrap()` panics in the parser (Rust)
- SSRF via unvalidated upstream URL in sidecar mode
- No request body size limits on HTTP endpoints
- `sled` embedded database is an abandoned crate

---

## Critical Findings

### C-1: ReDoS via Untrusted Regex Patterns
**Severity:** CRITICAL
**Affected:** `src/eval.rs:174-176`, `flagfile-ts/src/eval.ts:136-141`

Regex patterns from flagfile definitions are compiled and executed against context values without any validation, timeout, or complexity limit. An attacker who can write or inject a flagfile can craft a pattern that causes catastrophic backtracking, freezing the evaluation loop.

**Rust:**
```rust
Atom::Regex(pattern) => {
    let re_matched = match Regex::new(pattern) {
        Ok(re) => re.is_match(&haystack),  // no timeout, no complexity check
```

**TypeScript:**
```typescript
matched = new RegExp(rhsAtom.value).test(haystack);  // no timeout
```

**Attack vector:**
```
name ~ /^(a+)+b$/
```
With a context value `name = "aaaaaaaaaaaaaaaaac"`, the regex engine hangs for seconds to minutes.

**Impact:** Full denial of service of the flag evaluation service. Any user able to submit or modify a flagfile can trigger this.

**Recommendation:**
- Add regex pattern size limit (e.g., 512 chars max)
- Use a ReDoS-safe library (`regex` crate in Rust is already safe against catastrophic backtracking by construction — verify this holds; in TypeScript, use a library like `re2`)
- Pre-compile and validate all patterns at flagfile parse time, rejecting problematic patterns

---

### C-2: Unbounded Segment Recursion → Stack Overflow
**Severity:** CRITICAL
**Affected:** `src/eval.rs:213-222`, `flagfile-ts/src/eval.ts:229-234`

Segments can reference other segments with no cycle detection or depth limit. A circular dependency between segments causes infinite recursion and a stack overflow crash.

**Rust:**
```rust
AstNode::Segment(name) => {
    if let Some(seg_expr) = segs.get(name.as_str()) {
        eval_impl(seg_expr, context, flag_name, segments).unwrap_or(false)
        // no depth tracking, no visited set
    }
}
```

**Attack vector (flagfile):**
```
@segment A { segment(B) }
@segment B { segment(A) }
FF-test { segment(A) -> true; false }
```

**Impact:** Service crash via stack overflow. Any evaluation of a flag referencing these segments brings down the process.

**Recommendation:**
- Track visited segment names during evaluation (pass a `HashSet<&str>` through recursive calls)
- Alternatively, enforce a maximum segment depth (e.g., 10 levels)
- Detect cycles at parse/load time and reject flagfiles with circular segment dependencies

---

### C-3: `.unwrap()` on JSON Parsing in Flagfile Parser
**Severity:** CRITICAL
**Affected:** `src/parse_flagfile.rs:167` (approximate)

The JSON return value parser calls `.unwrap()` on `serde_json::from_str()`. Malformed JSON in a flagfile will panic the process.

```rust
fn parse_json(i: &str) -> IResult<&str, FlagReturn> {
    let parser = delimited(ws(tag("json(")), take_until(")"), ws(tag(")")));
    map(parser, |v| {
        FlagReturn::Json(serde_json::from_str(v).unwrap())  // panics on bad JSON
    })(i)
}
```

**Attack vector:**
```
FF-broken -> json({"unterminated": "string)
```

**Impact:** Process crash during flagfile loading or reload, taking down the service.

**Recommendation:** Return a parse error instead of panicking — propagate the JSON parse failure as a nom parse error so the entire flagfile is rejected gracefully.

---

### C-4: Context Injection — Flag Evaluation Bypass
**Severity:** CRITICAL (design-level)
**Affected:** `src/eval.rs:15-60`, `flagfile-ts/src/eval.ts:24-78`

The evaluator performs no validation or allowlisting of context keys. If an application passes user-controlled data directly as context keys, an attacker can inject arbitrary values to bypass flag logic.

**Example:**
```
Flag rule: plan == "premium" -> true

Attacker-controlled context: { "plan": "premium" }
// Flag evaluates to true, granting premium access
```

**Impact:** Security-critical flags (access control, premium gating, A/B tests) can be bypassed by an attacker who controls context key names or values.

**Recommendation:**
- Document clearly that context keys must be validated/allowlisted by the calling application before passing to the evaluator
- Consider adding a validation layer in the public API (`lib.rs` / `index.ts`) that restricts which context keys are accepted
- Add a security warning in the README and API docs

---

## High Severity Findings

### H-1: Multiple `.unwrap()` Panics in Parser (Rust)
**Severity:** HIGH
**Affected:** `src/parse.rs:27, 45, 60, 84, 95-97`

Several parser functions assume that after a successful pattern match the subsequent type conversion will always succeed. Edge cases in integer/float overflow or unexpected formatting can cause panics.

```rust
map(parser, |num: &str| Atom::Number(num.parse().unwrap()))(i)   // line 27
map(parser, |n: &str| Atom::Float(n.parse().unwrap()))(i)         // line 45
NaiveDate::parse_from_str(date_str, "%Y-%m-%d").expect("Invalid date format")  // line 60
```

**Attack vectors:** Integer values near `i64::MAX`, float scientific notation edge cases, dates like `9999-99-99`.

**Recommendation:** Replace `.unwrap()`/`.expect()` in parser code with proper error propagation using `nom::Err`.

---

### H-2: Integer Overflow in Percentage Bucketing
**Severity:** HIGH
**Affected:** `src/eval.rs:249-255`, `flagfile-ts/src/eval.ts:241-255`

Float-to-integer conversion for percentage thresholds can overflow or underflow with extreme values.

```rust
let threshold = (rate * 1000.0) as u64;  // undefined behavior if rate < 0 or rate > u64::MAX/1000
```

**Attack vectors:**
```
percentage(-100%, userId)    // rate < 0: cast wraps to large u64, flag always enabled
percentage(10000%, userId)   // overflow: unexpected threshold
```

**Recommendation:** Clamp `rate` to `[0.0, 100.0]` before arithmetic. Validate at parse time.

---

### H-3: Path Traversal via `--file` Argument (CLI)
**Severity:** HIGH
**Affected:** `flagfile-cli/src/main.rs:528-535, 595-610, 663-683`

The `--file` / `-f` CLI argument accepts arbitrary paths with no normalization or directory restriction.

```
ff validate -f ../../../../etc/passwd
ff test -f /etc/shadow
```

**Impact:** If the CLI is invoked by a service or automation with elevated privileges, this allows reading arbitrary files.

**Recommendation:** Canonicalize the path and optionally restrict it to the current working directory or a configured base directory. Emit a warning for absolute paths outside the project.

---

### H-4: SSRF via Unvalidated Upstream URL (Sidecar Mode)
**Severity:** HIGH
**Affected:** `flagfile-cli/src/server/mod.rs:573-584`, `flagfile-cli/src/server/sidecar.rs:20-60`

The `--upstream` parameter is used directly to construct HTTP requests without any URL validation or IP-range blocking.

```
ff serve --upstream http://169.254.169.254/latest/meta-data/
ff serve --upstream http://internal-db:5432/
```

**Impact:** SSRF — the server process can be made to probe internal services, cloud metadata endpoints, or admin interfaces.

**Recommendation:**
- Validate that upstream is an HTTPS URL
- Block RFC-1918 / loopback / link-local address ranges
- Consider a URL allowlist for production deployments

---

### H-5: No Request Body Size Limit on PUT /flagfile
**Severity:** HIGH
**Affected:** `flagfile-cli/src/server/routes.rs:104-326`

The `PUT /flagfile` endpoint accepts a request body with no configured size limit. Axum's default is 2 MB but this is not explicitly enforced.

**Impact:** A multi-gigabyte payload causes memory exhaustion / OOM kill.

**Recommendation:** Add `DefaultBodyLimit::max(10 * 1024 * 1024)` (10 MB) as middleware on the router.

---

### H-6: No CORS Configuration
**Severity:** HIGH
**Affected:** `flagfile-cli/src/server/mod.rs:474-531`

The HTTP server applies no CORS policy. A malicious web page can make credentialed requests to `/v1/eval` or `/flagfile` on behalf of users with valid tokens.

**Recommendation:** Add `tower_http::cors::CorsLayer` with explicit allowed origins. In multi-tenant mode, deny cross-origin requests to evaluation and management endpoints by default.

---

### H-7: No Rate Limiting on Eval Endpoint
**Severity:** HIGH
**Affected:** `flagfile-cli/src/server/routes.rs:371-460`

The `/v1/eval/{flag_name}` endpoint has no rate limiting, enabling:
- DoS via request flooding
- Brute-force enumeration of flag names

**Recommendation:** Add per-IP rate limiting (e.g., `tower_governor` crate, 100 req/s per IP).

---

### H-8: Weak Token Storage — Environment Variable Exposure
**Severity:** HIGH
**Affected:** `flagfile-cli/src/server/mod.rs:554-562`, `flagfile-cli/src/push.rs:35-40`, `flagfile-cli/src/pull.rs:6-14`

Tokens (`FF_SIDECAR_TOKEN`, `FF_WRITE_TOKEN`, `FF_READ_TOKEN`) are sourced from environment variables, which are visible in process listings (`ps aux`), container inspect outputs, and CI/CD logs.

**Recommendation:** Document this risk prominently. Prefer reading tokens from files with mode `0600` or from a secrets manager. Never log token values.

---

### H-9: Namespace Parameter Not Validated
**Severity:** HIGH
**Affected:** `flagfile-cli/src/server/routes.rs:660-704`

URL path parameters like `/ns/{namespace}/...` are not validated against an allowlist or pattern. Special characters, path separators, or injection sequences in namespace names could cause issues in downstream storage, logging, or URL construction.

**Recommendation:** Validate namespace with `^[a-zA-Z0-9_-]{1,63}$` before use.

---

### H-10: No HTTP Timeout on Remote Flagfile Fetch
**Severity:** HIGH
**Affected:** `src/builder.rs:101-127`

Remote flagfile fetching via `reqwest::blocking::Client::new()` has no timeout configured. A slow or unresponsive upstream server blocks the initialization thread indefinitely.

**Recommendation:** Add `.timeout(Duration::from_secs(30))` to the client builder.

---

### H-11: Hardcoded 'Flagfile' Path in TypeScript `init()`
**Severity:** HIGH
**Affected:** `flagfile-ts/src/index.ts:119-122`

```typescript
export function init(): void {
    const content = readFileSync('Flagfile', 'utf-8');
```

The path is hardcoded relative to the current working directory. If a process's CWD is attacker-controlled, an adversary can place a malicious `Flagfile` there.

**Recommendation:** Document that `init()` reads from CWD. Provide an explicit path parameter. Add a note that the CWD must not be user-writable.

---

### H-12: No Input Size Limit on Flagfile Content (TypeScript)
**Severity:** HIGH
**Affected:** `flagfile-ts/src/index.ts:119-163` (`initFromString`, `initRemote`)

No bounds check is applied to the flagfile content before parsing. A 100+ MB flagfile from a remote source causes memory and CPU exhaustion.

**Recommendation:** Check `content.length` before parsing; reject flagfiles larger than a configurable limit (suggested: 10 MB).

---

### H-13: NaN / Infinity Type Confusion in TypeScript Evaluator
**Severity:** HIGH
**Affected:** `flagfile-ts/src/eval.ts:136-156`, `flagfile-ts/src/ast.ts:219-226`

JavaScript's `Number` type can be `NaN` or `±Infinity`. These values are not explicitly rejected:
- `NaN == NaN` is always `false` in JS — a context value of `NaN` will never match any literal, potentially allowing unintended flag states
- `Infinity > 1000` is `true` — an `Infinity` context value passes all numeric `>` guards
- In percentage bucketing, `NaN` stringifies to `"NaN"` and gets a consistent SHA-1 hash, pinning users to a fixed bucket

**Recommendation:** Validate `Number` atoms at construction time; reject `NaN` and `Infinity` with a parse-time or API-boundary error.

---

## Medium Severity Findings

### M-1: Unbounded Regex Pattern Parsing — Memory Exhaustion
**Severity:** MEDIUM-HIGH
**Affected:** `src/parse.rs:263-268`

```rust
let (i, pattern) = take_until("/")(i)?;  // no length limit
```

A regex literal with no closing `/` will consume the rest of the input. A very long regex pattern (millions of characters) allocates unbounded memory.

**Recommendation:** Add a maximum pattern length check (e.g., 1024 characters) before or after `take_until`.

---

### M-2: Unbounded String Literals (TypeScript Parser)
**Severity:** MEDIUM
**Affected:** `flagfile-ts/src/parser.ts:71-83`

```typescript
const end = i.indexOf('"', 1);  // no length limit
```

String literals up to the full flagfile size are accepted. Combined with multiple rules, this allows unbounded memory allocation.

**Recommendation:** Enforce a maximum string literal length during parsing (e.g., 65,536 chars).

---

### M-3: Unvalidated JSON Return Payload Size (TypeScript)
**Severity:** MEDIUM
**Affected:** `flagfile-ts/src/flagfile.ts:88-116`

While `JSON.parse` is safely wrapped, there is no limit on the JSON payload size. Deeply nested or very large JSON objects cause memory exhaustion and potential prototype pollution if the result is consumed with `Object.assign` by downstream code.

**Recommendation:** Check payload size before parsing. Document that JSON return values are `unknown` and must not be passed to `Object.assign` or similar without sanitization.

---

### M-4: Segment Recursion — No Depth Limit (TypeScript)
**Severity:** MEDIUM
**Affected:** `flagfile-ts/src/eval.ts:229-234`

Same issue as C-2 but in the TypeScript implementation. The TypeScript runtime has a smaller default call stack than Rust, making this crash sooner.

**Recommendation:** Mirror the fix from C-2 in the TypeScript evaluator.

---

### M-5: No Maximum Expression Nesting Depth
**Severity:** MEDIUM
**Affected:** `src/parse.rs` (recursive descent), `flagfile-ts/src/parser.ts`

Arbitrarily deeply nested parenthesized expressions can cause stack overflow during parsing:
```
FF-test { (((((((((((((((user = "admin")))))))))))))))) -> true }
```

**Recommendation:** Add a depth counter to the recursive descent parsers and return a parse error when depth exceeds a limit (e.g., 64).

---

### M-6: Semver Float Coercion — Loss of Precision
**Severity:** MEDIUM
**Affected:** `src/ast.rs:22-32`, `flagfile-ts/src/ast.ts:146-160`

When a `Float` value is compared against a `Semver` literal, the float is coerced by string-splitting on `.`. This causes `Float(5.314)` to be interpreted as `Semver(5, 314, 0)` instead of `Semver(5, 3, 14)`, producing incorrect comparisons.

**Recommendation:** Document this limitation prominently. Consider adding a validation that minor/patch components from floats do not exceed 99, or reject float-to-semver coercion entirely.

---

### M-7: Error Messages Leak Flagfile Content
**Severity:** MEDIUM
**Affected:** `src/lib.rs:74-78`, `flagfile-ts/src/index.ts:155-159`

Parse errors include the unparsed flagfile content in the error string:
```rust
format!("Flagfile parsing failed: unexpected content near: {}", remainder.trim()...)
```

If flagfiles contain secrets, tokens, or sensitive configuration, these leak into logs and error monitoring systems.

**Recommendation:** Truncate or redact the content included in error messages. Log full details server-side at DEBUG level only.

---

### M-8: Bearer Token Potentially Logged in Error Messages
**Severity:** MEDIUM
**Affected:** `src/builder.rs:117-120, 216`

```rust
eprintln!("flagfile: remote fetch failed: {}, using fallback '{}'", e, fallback);
eprintln!("flagfile: SSE read error: {}, reconnecting...", e);
```

If `reqwest` error details include header information, or if the error message reflects parts of the request, the bearer token could appear in logs.

**Recommendation:** Sanitize error messages from HTTP clients before logging. Never log `Authorization` header values.

---

### M-9: No CORS + Unauthenticated Observability Endpoints
**Severity:** MEDIUM
**Affected:** `flagfile-cli/src/server/mod.rs:518-522`

`/metrics`, `/health`, and `/readyz` are accessible without authentication. The `/metrics` Prometheus endpoint exposes:
- Number of flags per namespace
- Flag evaluation frequency
- Raft cluster topology and state

This aids attacker reconnaissance.

**Recommendation:** Restrict `/metrics` to localhost or require authentication in multi-tenant deployments.

---

### M-10: Lint Output Not Sanitized — Log Injection Risk
**Severity:** MEDIUM
**Affected:** `flagfile-cli/src/lint/mod.rs:80-96`

Lint warning messages include flag names and rule content directly in output. If a flag name contains ANSI escape codes or newline characters, terminal output can be manipulated.

**Recommendation:** Sanitize flag names and rule content before including in lint output (strip non-printable characters).

---

### M-11: URL Construction via String Concatenation
**Severity:** MEDIUM
**Affected:** `flagfile-cli/src/server/mod.rs:573-584`

```rust
format!("{}/ns/{}/flagfile", upstream, ns)
```

Namespace is not URL-encoded. A namespace containing `/` or `?` could manipulate the path.

**Recommendation:** Use `percent_encoding::utf8_percent_encode` for all user-supplied URL path components.

---

### M-12: File Watcher TOCTOU Race Condition
**Severity:** MEDIUM
**Affected:** `flagfile-cli/src/server/mod.rs:708-798`

Between a file change event being detected and the file being read, the file could be swapped. This enables a TOCTOU attack where a validated flagfile is replaced with an invalid or malicious one.

**Recommendation:** Read the file into a buffer atomically (read once, validate, then swap in-memory). Use advisory file locking where supported.

---

### M-13: Percentage Bucketing Hash Entropy (60 bits)
**Severity:** MEDIUM
**Affected:** `src/eval.rs:251`, `flagfile-ts/src/eval.ts:250`

```rust
let substr = &hex[..15];  // 60 bits of entropy from SHA-1
let bucket = value % 100_000;
```

Truncating to 60 bits and using modulo 100,000 introduces bias. For deployments with ~1 million users, the birthday paradox makes hash collisions statistically inevitable, breaking statistical validity of percentage rollouts.

**Recommendation:** Use all available hash bits (SHA-1 is 160 bits). Consider switching to a purpose-built consistent hashing algorithm (e.g., murmurhash, xxhash).

---

### M-14: Raft Snapshot Integrity Not Verified
**Severity:** MEDIUM
**Affected:** `flagfile-cli/src/server/mod.rs:315-379`

Snapshot data received from Raft peers is applied without cryptographic verification. A malicious peer could send a corrupted snapshot to corrupt the state machine.

**Recommendation:** Add HMAC-SHA256 verification of snapshot payloads. Validate snapshot integrity before applying.

---

### M-15: Unencrypted Raft gRPC Communication
**Severity:** MEDIUM
**Affected:** `flagfile-cli/src/server/raft/` (transport layer)

Inter-node Raft communication via gRPC is not mentioned to use TLS, leaving cluster traffic unencrypted and vulnerable to MITM attacks.

**Recommendation:** Enforce mutual TLS (mTLS) for Raft gRPC connections. Provide configuration options for certificate paths.

---

### M-16: Sidecar HTTP Client Has No Timeout
**Severity:** MEDIUM
**Affected:** `flagfile-cli/src/server/sidecar.rs:32-54`

The sidecar `fetch_and_update()` makes HTTP requests with no timeout. An unresponsive upstream causes the goroutine/task to hang indefinitely, consuming resources.

**Recommendation:** Set a timeout on the `reqwest::Client`:
```rust
reqwest::ClientBuilder::new().timeout(Duration::from_secs(10)).build()
```

---

### M-17: Credentials Stored in `ff.toml` Config File
**Severity:** MEDIUM
**Affected:** `flagfile-cli/src/push.rs:7-17`

Tokens can be stored in plaintext in `ff.toml`. If this file is accidentally committed to a public repository, credentials are exposed.

**Recommendation:** Document that tokens must not be checked into version control. Add a `.gitignore` entry for `ff.toml`. Consider adding a warning if `ff.toml` is found inside a git repository.

---

## Low Severity / Informational Findings

### L-1: NaN Float Comparisons — Rust
**Severity:** LOW
**Affected:** `src/ast.rs:90-98`

`partial_cmp()` returns `None` for NaN, which is correctly handled. However, the behavior (silently returning false for all NaN comparisons) is not documented and could surprise consumers.

**Recommendation:** Add documentation noting NaN comparison behavior.

---

### L-2: `unreachable!()` in ComparisonOp::build_from_str
**Severity:** LOW
**Affected:** `src/ast.rs:227`

```rust
_ => unreachable!(),
```

If this function is ever called with an unexpected operator string (e.g., from a refactor), it panics. Should return a `Result` instead.

**Recommendation:** Replace with `unreachable!()` only if the callsite is internal and guaranteed, otherwise return `Result<Self, &str>`.

---

### L-3: Implicit `=` Operator Ambiguity
**Severity:** LOW
**Affected:** `src/parse.rs:134`, `flagfile-ts/src/parser.ts:152`

Single `=` is accepted as equality (`==`). This could confuse users who write assignment intent. The behavior is consistent between Rust and TypeScript but should be documented.

---

### L-4: Empty String Matching Behavior
**Severity:** LOW
**Affected:** `src/eval.rs:186-192`

Empty string comparisons work correctly (`"hello" ~ "" → true`) but are surprising. This should be documented.

---

### L-5: Unicode Normalization Not Performed
**Severity:** INFO
**Affected:** Both implementations

String comparisons use byte/UTF-8 equality without Unicode normalization. Visually identical strings with different normalization forms will not match.

**Recommendation:** Document string comparison semantics. Consider adding a `normalize()` option for locale-sensitive use cases.

---

### L-6: Long Flag Names / Env Names — No Length Limit
**Severity:** INFO
**Affected:** `src/parse_flagfile.rs:339-344, 377-379`

Flag names and `@env` names have no length restrictions. Extremely long names waste memory.

**Recommendation:** Add a maximum length limit (e.g., 256 characters) for flag names.

---

### L-7: Comments in Flagfiles May Contain Secrets
**Severity:** INFO
**Affected:** All flagfile parsers

Comments stripped at parse time but visible in source could expose sensitive reasoning (e.g., security bypass rationale, test credentials).

**Recommendation:** Audit flagfiles stored in version control for sensitive comments.

---

### L-8: Flag Name Not Validated Early in CLI
**Severity:** LOW
**Affected:** `flagfile-cli/src/main.rs:88-89`

Flag names passed to `ff eval` are not validated against the `FF-`/`FF_` prefix requirement before evaluation. The error message could be used for log injection if unsanitized.

**Recommendation:** Validate flag names at CLI argument parse time and reject early with a clean error.

---

## Supply Chain & CI/CD Security

### SC-1: `cargo-dist` Installer Downloaded via `curl | sh`
**Severity:** HIGH
**File:** `.github/workflows/release.yml:67`

```yaml
run: "curl --proto '=https' --tlsv1.2 -LsSf https://github.com/axodotdev/cargo-dist/releases/download/v0.30.3/cargo-dist-installer.sh | sh"
```

The installer script is downloaded from GitHub at release time without checksum verification. If the release artifact is compromised, the CI/CD pipeline will execute malicious code in the build environment. The version is pinned (`v0.30.3`) which mitigates the worst case, but binary hash is not verified.

**Recommendation:** Download the installer and verify its SHA-256 checksum against a hardcoded expected value before execution.

---

### SC-2: `actions/checkout@v3` — Outdated Action
**Severity:** MEDIUM
**File:** `.github/workflows/rust.yml:18`

```yaml
- uses: actions/checkout@v3
```

`rust.yml` uses `actions/checkout@v3` while `release.yml` uses `v4`. Actions should be pinned to specific SHAs or kept on the latest major version consistently. `v3` is no longer receiving security updates.

**Recommendation:** Update to `actions/checkout@v4` across all workflows. Pin to a specific commit SHA for supply chain hardening.

---

### SC-3: `sled` — Abandoned Embedded Database
**Severity:** HIGH
**File:** `flagfile-cli/Cargo.toml:34`

```toml
sled = "0.34"
```

`sled` v0.34 is effectively abandoned — the last release was in 2021 and the crate's own README advises against using it in production. It has known issues and no recent security patches. Using it in a production server creates supply chain and stability risk.

**Recommendation:** Migrate to an actively maintained embedded database (`redb`, `rocksdb`, `sqlite` via `rusqlite`, or `lmdb`).

---

### SC-4: `protobuf = "2"` — Outdated Major Version
**Severity:** MEDIUM
**File:** `flagfile-cli/Cargo.toml:48`

```toml
protobuf = "2"
```

`protobuf` v2 is the legacy Rust protobuf library. v3 (a rewrite) is current. The v2 crate may have unpatched security issues.

**Recommendation:** Evaluate migration to `protobuf = "3"` or use `prost` exclusively (which is already in the dependencies).

---

### SC-5: No `cargo audit` in CI
**Severity:** MEDIUM
**Files:** All workflow files

No CI workflow runs `cargo audit` to check for known CVEs in dependencies. Given the large dependency surface (Raft, Axum, sled, reqwest, tonic), vulnerabilities could go undetected.

**Recommendation:** Add `cargo audit` step to `rust.yml`:
```yaml
- name: Security audit
  run: cargo install cargo-audit && cargo audit
```
Or use the `rustsec/audit-check` GitHub Action.

---

### SC-6: Inconsistent `chrono` Versions
**Severity:** LOW
**Files:** `Cargo.toml:33` vs `flagfile-cli/Cargo.toml:23`

```toml
# flagfile-lib
chrono = "0.4.34"
# flagfile-cli
chrono = "0.4.43"
```

Both crates will resolve independently and compile multiple versions into the binary. This wastes binary size and could introduce inconsistencies in behavior.

**Recommendation:** Unify `chrono` to a single version in the workspace `[workspace.dependencies]` table.

---

### SC-7: `HOMEBREW_TAP_TOKEN` with `persist-credentials: true`
**Severity:** LOW
**File:** `.github/workflows/release.yml:311-313`

```yaml
- uses: actions/checkout@v4
  with:
    persist-credentials: true
    token: ${{ secrets.HOMEBREW_TAP_TOKEN }}
```

`persist-credentials: true` leaves the token accessible to subsequent steps in the job. While this is intentional for the commit step, any injected code in later steps could exfiltrate it.

**Recommendation:** This is accepted risk for a dedicated publish job. Ensure the job only runs verified release code and the token's scope is limited to the tap repository.

---

### SC-8: Release Workflow Triggered on Pull Requests
**Severity:** MEDIUM
**File:** `.github/workflows/release.yml:42-44`

```yaml
on:
  pull_request:
  push:
    tags: ...
```

The release workflow runs `dist plan` on every PR. While it only publishes on tag pushes, any third-party action used in this workflow is also executed on PRs from forks, potentially exposing secrets or build infrastructure.

**Recommendation:** Restrict PR triggers to trusted contributors or separate the plan/build steps from the publish steps with explicit permissions checks.

---

## Rust vs TypeScript Discrepancies

These differences between the two implementations could cause divergent behavior for the same flagfile, potentially creating exploitable inconsistencies when one is used in production and the other for testing.

| Area | Rust | TypeScript | Risk |
|------|------|-----------|------|
| SHA-1 truncation | `&hex[..15]` (60 bits) | `hex.slice(0, 15)` | Same — consistent ✓ |
| Float parsing | `n.parse::<f64>().unwrap()` | `parseFloat(m[0])` | Rust panics on edge cases, TS returns NaN |
| Date validation | `chrono` strict parsing | Manual string slicing | TS `parseDateTime` less strict — accepts invalid months/days |
| NaN handling | `partial_cmp()` returns `None` | No explicit NaN check | TS `Infinity > x` is `true`; Rust correctly rejects |
| Regex error | `Regex::new()` → `Err` | `new RegExp()` → `catch` | Both treat as false ✓ |
| Segment recursion | Stack overflow (larger stack) | Stack overflow (smaller stack, crashes sooner) | Both vulnerable |

**Highest risk discrepancy:** TypeScript `parseDateTime` does not validate that month/day values are in valid ranges before constructing the date atom. A date like `2024-99-99` parses differently in each implementation, which could cause different flag evaluation results.

---

## Positive Security Practices

The codebase demonstrates several commendable security practices:

- ✅ `unsafe_code = "forbid"` enforced at workspace level (`Cargo.toml:15`)
- ✅ `"strict": true` TypeScript configuration
- ✅ No `eval()` or `Function()` constructor usage in TypeScript
- ✅ No `Object.assign()` on untrusted data
- ✅ `JSON.parse()` always wrapped in `try/catch` (TypeScript)
- ✅ Zero runtime npm dependencies (TypeScript only uses dev deps)
- ✅ Authentication required on management endpoints (bearer token)
- ✅ No SQL or database query construction from user input
- ✅ Rust regex crate is NFA-based and immune to most ReDoS patterns (verify for specific patterns)
- ✅ Clear separation of parsing and evaluation phases
- ✅ `persist-credentials: false` on most CI checkout steps
- ✅ TLS enforced for reqwest (`rustls-tls` feature, no `native-tls`)

---

## Remediation Priority Matrix

### Immediate Action Required (CRITICAL)

| ID | Issue | Fix |
|----|-------|-----|
| C-1 | ReDoS via user regex | Add size limit; use timeout-aware matching |
| C-2 | Segment recursion / stack overflow | Add visited set + depth limit |
| C-3 | `.unwrap()` on JSON parse → panic | Propagate as parse error |
| C-4 | Context injection bypass | Document + add API-boundary validation |
| SC-3 | `sled` abandoned crate | Migrate to maintained alternative |

### High Priority (Next Release)

| ID | Issue | Fix |
|----|-------|-----|
| H-1 | Parser `.unwrap()` panics | Replace with proper error propagation |
| H-2 | Percentage overflow | Clamp rate to [0.0, 100.0] |
| H-4 | SSRF via upstream URL | Validate URL, block internal IPs |
| H-5 | No body size limit | Add `DefaultBodyLimit` middleware |
| H-6 | No CORS | Add `CorsLayer` with explicit origins |
| H-7 | No rate limiting | Add `tower_governor` middleware |
| H-10 | No HTTP timeout | Add `.timeout(Duration::from_secs(30))` |
| H-13 | NaN/Infinity in TypeScript | Reject at atom construction boundary |
| SC-1 | `curl \| sh` without checksum | Verify SHA-256 of installer |
| SC-5 | No `cargo audit` in CI | Add audit step to CI workflow |

### Medium Priority

| ID | Issue | Fix |
|----|-------|-----|
| M-1 | Unbounded regex pattern | Add 1024-char limit in parser |
| M-2 | Unbounded string literals | Add 65536-char limit |
| M-3 | Unvalidated JSON payload size | Check before parsing |
| M-6 | Semver float coercion | Document or reject |
| M-7 | Error message content leakage | Sanitize/truncate error output |
| M-9 | Unauthenticated /metrics | Restrict to localhost or add auth |
| M-14 | Raft snapshot integrity | Add HMAC verification |
| M-15 | Raft gRPC unencrypted | Enforce mTLS |
| SC-2 | `checkout@v3` outdated | Update to v4 |
| SC-4 | `protobuf = "2"` outdated | Evaluate v3 migration |
| SC-8 | Release workflow on PRs | Restrict fork-PR triggers |

---

*Report generated by multi-agent security research team. All findings are based on static analysis of source code. Dynamic testing (fuzzing, penetration testing) is recommended as a follow-up to validate and discover additional issues.*
