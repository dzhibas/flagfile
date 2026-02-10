# Features TODO

## DSL features to add

- [ ] percentage(rate, field?, salt?) - field and salt optional. use SHA-1
      if salt not specified than flag name is used as salt

      FF-new-checkout {
          percentage(5%, userId) -> true
          false
      }

      FF-gradual-migration {
          percentage(50, "migration") and plan == "premium" -> true
          percentage(10, "migration") -> true
          false
      }

ALGORITHM: flagfile_bucket(flag_name, bucket_key, salt?)

1. Build input string:
   - If salt provided: "{flag_name}.{salt}.{bucket_key}"
   - If no salt:       "{flag_name}.{bucket_key}"
   
   All strings are UTF-8. Concatenation uses literal "." separator.

2. Compute SHA-1:
   sha1_hex = lowercase hex string of SHA-1(input_bytes)
   
   Example: SHA-1("FF-checkout.alice") = "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0"

3. Take first 15 hex characters:
   substr = sha1_hex[0..15]
   
   Example: "a1b2c3d4e5f6a7b"

4. Parse as integer (base 16):
   value = parseInt(substr, 16)
   
   Example: 0xa1b2c3d4e5f6a7b = 11667345614019896955 (but capped to 15 chars)

5. Compute bucket (0 - 99999):
   bucket = value % 100000
   
   Using 100,000 partitions (not 100) gives 0.001% granularity

6. Compare:
   bucket < (percentage * 1000) → IN rollout
   
   Example: 25% → bucket < 25000
            0.5% → bucket < 500

- [ ] date support exists, but DateTime / ISO Timestamps needed

      FF-launch-event {
          now() > 2025-06-15T09:00:00Z and now() < 2025-06-15T18:00:00Z -> true
          false
      }

 - [ ] mod() for bucketing into buckets

      Syntax: mod(variable, divisor) returns a number usable in comparisons

      FF-ab-test {
          mod(userId, 2) == 0 -> json({"variant": "A"})
          mod(userId, 2) == 1 -> json({"variant": "B"})
      }

 - [ ] contains any we have, we need somehow contains all function

      FF-beta-only {
          tags in (beta_tester, internal) -> true
          false
      }
      FF-required-roles {
          roles all (admin, editor) -> true
          false
      }

## Segments - not to repiet same conditions

        // Define reusable segments at the top of Flagfile
        @segment beta-testers {
            email ~ /@company\.com$/
            or userId in ("user-123", "user-456", "user-789")
        }

        @segment premium-eu {
            tier == "premium" and countryCode in ("NL", "DE", "FR", "BE")
        }

        @segment mobile-users {
            platform in ("ios", "android") and appVersion >= 2.0.0
        }

        // Use them in flags — clean, DRY, readable
        FF-new-dashboard {
            segment(beta-testers) -> true
            segment(premium-eu) and percentage(50%, userId) -> true
            false
        }

        FF-new-checkout {
            segment(beta-testers) -> true
            segment(mobile-users) -> json({"layout": "mobile-optimized"})
            false
        }

## Imports (Multi-file Flagfiles)

        // Flagfile
        @import "./segments.flagfile"        // local segments
        @import "./flags/checkout.flagfile"  // feature area grouping
        @import "./flags/payments.flagfile"

        // Still can have flags directly here
        FF-maintenance-mode -> false

--- 

        // segments.flagfile — shared via git submodule or package
        @segment internal-users {
            email ~ /@company\.com$/
        }

        @segment enterprise {
            tier == "enterprise" and seats >= 50
        }

## Flag Metadata / Annotations

Flags need lifecycle management. Without metadata, flags accumulate as tech debt forever.

        // Metadata as annotations above the flag
        @owner "payments-team"
        @expires 2026-06-01
        @ticket "JIRA-1234"
        @description "New 3DS2 authentication flow for EU payments"
        @type release                    // release | experiment | ops | permission
        FF-3ds2-auth {
            segment(premium-eu) -> true
            false
        }

        // Permanent operational flag — no expiry
        @owner "platform-team"
        @type ops
        @description "Kill switch for notification service"
        FF-kill-notifications -> true

        // The CLI can then enforce governance:
        // $ ff lint
        // ⚠ FF-old-feature: expired 2025-12-01 (45 days ago)
        // ⚠ FF-unnamed-flag: missing @owner
        // ⚠ FF-experiment-x: type=experiment but no @expires set

## Environments

The same flag often needs different behavior in dev/staging/prod. we can have diff content deployed, but this way it can be done easier

        FF-debug-logging {
            @env dev -> true
            @env stage -> true
            @env prod -> false
        }

        // Or more usefully: different rollout speeds per environment
        FF-new-search {
            @env dev -> true
            @env stage -> true
            @env prod {
                segment(beta-testers) -> true
                percentage(25%, userId) -> true
                false
            }
        }

and in application env injected during init

        // App startup
        flagfile_lib::init_with_env("prod");

## Value type declaration for stricter linting

        // Type declaration prevents runtime surprises
        @type bool
        FF-feature-x -> true

        @type string  
        FF-button-color -> "blue"

        @type int
        FF-retry-count -> 3

        @type json
        FF-theme {
            tier == "premium" -> json({"dark": true})
            json({"dark": false})
        }

        $ ff validate
        ✗ FF-retry-count: rule returns string "three" but flag is declared @type int

## Flag Dependencies / Prerequisites

        FF-new-checkout -> true

        // This flag only evaluates if FF-new-checkout is true
        @requires FF-new-checkout
        FF-checkout-upsell {
            percentage(50%, userId) -> true
            false
        }

If FF-new-checkout is false, FF-checkout-upsell automatically returns false without evaluating rules

## Variants

Current approach uses JSON for variants, but a dedicated syntax is cleaner for A/B testing:

        ```
        @type variant
        FF-checkout-experiment {
            @variants {
                control  -> json({"steps": 4, "layout": "classic"})
                streamlined -> json({"steps": 2, "layout": "single-page"})
                express  -> json({"steps": 1, "layout": "one-click"})
            }

            // Assign by percentage bands
            segment(beta-testers) -> streamlined
            percentage(33%, userId) -> streamlined
            percentage(66%, userId) -> express
            control
        }
        ```

This makes it explicit that the flag is an experiment with named variants rather than arbitrary JSON values.

## `coalesce()` / Null Handling

When context values might be missing:

        ```
        FF-geo-features {
            // Use countryCode if present, fall back to region, then "unknown"
            coalesce(countryCode, region, "unknown") == "NL" -> true
            false
        }
        ```
### List / Array Values in Context

Sometimes you need to check if a user has a particular role or feature entitlement:

        ```
        FF-admin-panel {
            // roles is an array: ["viewer", "editor", "admin"]
            "admin" in roles -> true
            false
        }

        FF-advanced-export {
            // Check if any of user's entitlements match
            "export-csv" in entitlements or "export-all" in entitlements -> true
            false
        }
        ```

The key is that `in` works both ways — value in list (you already have this) AND value in context-array.

## Named rules (Structured)

Beyond freeform `//` comments, structured rule descriptions help with audit logs or see which rule was triggered:

        ```
        FF-payment-routing {
            // These descriptions appear in the admin UI and audit logs
            @rule "High-value transaction routing"
            amount > 10000 and currency == "EUR" -> json({"provider": "stripe"})

            @rule "Default payment provider"
            json({"provider": "adyen"})
        }
        ```

### `@deprecated` annotation

        ```
        @deprecated "Use FF-new-checkout instead"
        @expires 2026-04-01
        FF-old-checkout -> true

        // $ ff lint
        // ⚠ FF-old-checkout is deprecated: "Use FF-new-checkout instead"
        // 
        // $ ff find FF-old-checkout
        // src/checkout.rs:42  let old = ff("FF-old-checkout", &ctx);
        // src/checkout.rs:89  if ff("FF-old-checkout", &ctx) { ... }
        // Found 2 references. Flag expires 2026-04-01.
        ```

- [ ] syntax version annotation at the top aka @version=1
- [ ] global setting to treat everything case-insensitive for example @case_ci true. its like all strings would go through lower() modifier

- [x] date > timestamp comparison
- [x] parse NOW function
- [x] evaluator for Flagfile
- [x] restructure and rename project into Flagfile.rs / into workspaces
- [x] introduce new comparison ops - regex match (and does not match), contains (and does not contain)

Flagfile-Relay

relay is actually `ff serve` which exposes rest api, open-feature api

- [ ] Server options: gRPC (tonic), rest (axum), redis (pretend to be redis), flagfile cdn
  - [ ] axum rest api
  - [ ] server as cdn can serve this as flagfile or converted through converter into other framework (like launchdarkly or .net flagging)
- [ ] Pullers: pulling from cdn, from github, SSE - listening for update as server side events
- [ ] Create lightweight side-car container for this
- [ ] Look into if we can transform AST of feature flags into json structure launchdarkly uses and force ld-relay to pull state from our apis instead so client libraries doesnt need to change. similar to what dorklyorg/dorkly does with yaml files

Flagfile other crazy ideas

- [ ] Compile into other framework languages something like you have Flagfile and then convert to .net feature flagging

Done

- [x] startsWith endsWith - Syntax: ^~ (startsWith), ~$ (endsWith), with negated forms !^~ and !~$
- [x] Atom parsing for Date
- [x] Date converted to chrono datetime for comparison later
- [x] Scopes and negated scopes. ex.: a=b and !(c=d or g=z)
- [x] either lower() or upper() function calls or case-insensitive string comparison operators
- [x] support for single quote strings
- [x] evaluation with provided context
- [x] date comparisons
- [x] parse comments in function body under rules
