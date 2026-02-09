# TODO

items tracking to be done

Flagfile-Parser

- [x] date > timestamp comparison
- [x] parse NOW function
- [x] evaluator for Flagfile
- [x] restructure and rename project into Flagfile.rs / into workspaces
- [ ] introduce new comparison ops - regex match (and does not match), contains (and does not contain)

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
