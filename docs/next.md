DSL next features to implement

## Flag Dependencies / Prerequisites

        FF-new-checkout -> true

        // This flag only evaluates if FF-new-checkout is true
        @requires FF-new-checkout
        FF-checkout-upsell {
            percentage(50%, userId) -> true
            false
        }

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