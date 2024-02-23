# Roadmap

for the full-blown project Flagfile follow same bottom up technique just like NOM parsing bottom up :)

## sequence of events

1. create parser and evaluator for boolean expressions (DONE)
2. create parser and evaluator for Flagfile (IN PROGRESS)
3. finalize api for parsing and evaluating both
4. publish them as cargo libs
5. export WASM and FFI
6. create demo ffi lib in lets say c# .net core
7. create simple UI to create and update Flagfile on web
8. create sidecar container with storage of Flagfile in git/filesystem/cdn configurable
      expose same through either restful/grpc/redis custom command
9. create full-blown UI multi-tenant and projects, envs and stuff to serve Flagfile through cdn
10. create clientside libs through ffi in other popular languages
