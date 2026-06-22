# 2. Return types

[← Back to index](README.md)

A flag doesn't have to return a boolean. The value after `->` can be one of
several types, which lets a flag carry configuration, not just an on/off state.

## Boolean

The classic switch. The keywords are case-insensitive — `true`, `TRUE`,
`false`, and `FALSE` all work.

```flagfile
FF-feature-flat-on-off -> true
FF_Feature23432 -> TRUE
FF-beta-features -> false
```

## Integer

Handy for tunable limits and timeouts.

```flagfile
FF-api-timeout -> 5000
FF-max-retries -> 3
```

## String

Use double or single quotes. Strings are great for picking a named variant.

```flagfile
FF-log-level -> "debug"
FF-button-color -> "blue"
```

## JSON variant

Wrap any JSON value in `json( … )` to return structured configuration. The JSON
can be an object, an array, nested — anything valid. An empty object is just
`json({})`.

```flagfile
FF-feature-json-variant -> json({"success": true})

FF-theme-config -> json({
  "primaryColor": "#007bff",
  "secondaryColor": "#6c757d",
  "darkMode": true,
  "animations": true
})
```

## What it does

The value you return is exactly what your application receives when it evaluates
the flag. A boolean flag reads as a `bool`; a `json(...)` flag hands your code a
parsed JSON value to branch on. A single flag should be consistent in the *kind*
of value it returns, but different flags can return different types.

These return values aren't limited to the short form — they're also what each
rule produces inside a block, which is the next topic.

Next: [Rules and defaults →](03-rules-and-defaults.md)
