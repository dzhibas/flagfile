# TODO

items tracking to be done

Still todo

- [ ] date > timestamp comparison
- [ ] parse NOW function
- [ ] evaluator for Flagfile
- [ ] restructure and rename project into Flagfile.rs / into workspaces
- [ ] introduce new comparison ops - regex match (and does not match), contains (and does not contain)

CLI

- [ ] flagfile validate
- [ ] flagfile init
- [ ] flagfile list
- [ ] built-in ui to edit and save flagfile in current dir
- [ ] flagfile find // find all flagnames within current directory source code recursively
        like you would do with grep or ripgrep with regex /(FF[-_].*)\s?->/
- [ ] make it so you can install flagfile-cli with brew or shell script
- [ ] flagfile fmt
- [ ] flagfile test (will look for flagfile.tests file and run those)

Done

- [x] Atom parsing for Date
- [x] Date converted to chrono datetime for comparison later
- [x] Scopes and negated scopes. ex.: a=b and !(c=d or g=z)
- [x] either lower() or upper() function calls or case-insensitive string comparison operators
- [x] support for single quote strings
- [x] evaluation with provided context
- [x] date comparisons
- [x] parse comments in function body under rules
