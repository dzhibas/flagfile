# TODO

items tracking to be done

Flagfile-Parser

missing features:

- [x] startsWith endsWith - Syntax: ^~ (startsWith), ~$ (endsWith), with negated forms !^~ and !~$
- [ ] set not set

      FF-new-onboarding {
          userId is set and signupDate > 2025-01-01 -> true
          userId is not set -> false
          false
      }

- [ ] percentage()

 Syntax: percentage(variable, threshold) or percentage(variable, threshold, "salt")

      FF-new-checkout {
          percentage(userId, 5, "FF-new-checkout") -> true
          false
      }

      FF-gradual-migration {
          percentage(orgId, 50, "migration") and plan == "premium" -> true
          percentage(orgId, 10, "migration") -> true
          false
      }

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

- [ ] syntax version annotation at the top aka // @version=1
- [ ] global setting to treat everything case-insensitive for example // @case-insensitive=true. its like all strings would go through lower() modifier

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

- [x] Atom parsing for Date
- [x] Date converted to chrono datetime for comparison later
- [x] Scopes and negated scopes. ex.: a=b and !(c=d or g=z)
- [x] either lower() or upper() function calls or case-insensitive string comparison operators
- [x] support for single quote strings
- [x] evaluation with provided context
- [x] date comparisons
- [x] parse comments in function body under rules
