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