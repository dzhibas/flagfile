# 12. Environments

[← Back to index](README.md)

Flags often need to behave differently per environment — on everywhere in `dev`
and `stage`, but carefully gated in `prod`. The `@env` rule expresses that
inside a flag block.

## Simple form

`@env name -> value` returns a value when evaluating in that environment:

```flagfile
FF-debug-env-logging {
    @env dev->true
    @env stage->true
    @env prod->false
    false
}
```

(As always, the arrow can have spaces or not.)

## Block form

For richer per-environment logic, give `@env` its own block of rules with its
own default:

```flagfile
FF-sdk-upgrade {
    @env stage {
        appVersion >= 5.3.42 -> false
        appVersion < 4.32.0 -> false
        false
    }
    @env dev -> true

    // rules below apply when no @env matched
    appVersion >= 5.3.42 -> true
    appVersion < 4.32.0 -> false
    false
}
```

## How it fits with regular rules

`@env` rules live alongside ordinary rules in the same top-to-bottom block. An
`@env` rule only applies when the current environment matches its name; if none
match, evaluation continues to the regular rules and the final default — exactly
as shown in `FF-sdk-upgrade` above.

Next: [Annotations →](13-annotations.md)
