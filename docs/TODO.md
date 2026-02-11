# Features TODO

## DSL features to add

### Imports (Multi-file Flagfiles)

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

- [ ] syntax version annotation at the top aka @version=1
- [ ] global setting to treat everything case-insensitive for example @case_ci true. its like all strings would go through lower() modifier

- [x] date > timestamp comparison
- [x] parse NOW function
- [x] evaluator for Flagfile
- [x] restructure and rename project into Flagfile.rs / into workspaces
- [x] introduce new comparison ops - regex match (and does not match), contains (and does not contain)


CLI

- [ ] migration tool, to migrate from one product to Flagfile syntax. Lets say LD flag json dump to Flagfile syntax

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

- [x] percentage(rate, contextField?, salt?) - field and salt optional. use SHA-1
- [x] startsWith endsWith - Syntax: ^~ (startsWith), ~$ (endsWith), with negated forms !^~ and !~$
- [x] Atom parsing for Date
- [x] Date converted to chrono datetime for comparison later
- [x] Scopes and negated scopes. ex.: a=b and !(c=d or g=z)
- [x] either lower() or upper() function calls or case-insensitive string comparison operators
- [x] support for single quote strings
- [x] evaluation with provided context
- [x] date comparisons
- [x] parse comments in function body under rules
