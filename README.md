# boolean expresion parser in Nom

same as pest parser written with pest.rs here https://github.com/dzhibas/bool_expr_parser but parsed with NOM

## TODO

- comparison expressions > < >= <= = ==
- logic expressions and or
- array expressions in [] not in []
- value expressions: ="string inside", =STRING_without_spaces (treated as string if no spaces around), =1212 (digits), =1213.21 (floats)
