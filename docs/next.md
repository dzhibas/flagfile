DSL next features to implement

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