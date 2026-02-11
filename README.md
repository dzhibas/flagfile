# <img src="https://github.com/dzhibas/flagfile/blob/main/public/flagfile-white-bg.svg?raw=true" width=250px/>

![Tests passing](https://github.com/dzhibas/flagfile/actions/workflows/tests.yml/badge.svg)

Flagfile

it's developer and devops friendly feature flagging solution with cli command to manage thise where you define all your flags with rules and tests for those in Flagfile in this format: [Flagfile.example](Flagfile.example)

it has a cli available through `brew install dzhibas/tap/flagfile-cli` which allows you to init, run tests, validate syntax, serve flags through openfeature-api, find flags in codebase and evaluate flags through command line

it also have libraries to work with flagfile in each language, check our [examples/](examples/) folder

## Usage in Rust

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
flagfile-lib = "0.4"
```
or until it becomes more stable and will be published into crates.io

```toml
[depedencies]
flagfile-lib = { git = "https://github.com/dzhibas/flagfile.git" }
```

Then use `init()` to load a `Flagfile` from the current directory and `ff()` to evaluate flags:

```rust
use flagfile_lib::{Context, ff};
use std::collections::HashMap;

fn main() {
    flagfile_lib::init();

    let ctx: Context = HashMap::from([("tier", "premium".into()), ("country", "nl".into())]);
    let flag: bool = ff("FF-feature-y", &ctx).expect("Flag not found").into();

    if flag {
        println!("Flag is on");
    } else {
        println!("Flag is off");
    }
}
```

## Usage in Javascript/Typescript

add dependency in package.json of `flagfile-js`. then `ff init` and then

```js
import { init, ff } from "flagfile-ts";

init();

const ctx = {
  tier: "premium",
  countryCode: "nl",
};

if (ff("FF-feature-y", ctx)) {
  console.log("Flag is on");
} else {
  console.log("Flag is off");
}
```

## Features language summary:

*   Feature flag name definition: `FF-<name>` or `FF_<name>`

it's choosen specifically so that it prefixed with FF, cause cli provides a way to find it in codebase repo if you will need to clean it up with `ff find` or `ff find -s premium`

*   Short notation: `FF-name -> value`
*   Block notation: `FF-name { rules... defaultValue }`
*   Values: `true`, `TRUE`, `false`, `FALSE`, `json({"key": "value"})`, int or string
*   Rules: `condition1 && condition2 -> value`
*   Default value in block: A final `value` without `->`
*   Conditions:
    *   Comparisons: `==`, `!=` (implicit from `not`), `>=`, `<=`, `>`, `<`
    *   Logical op (case-insensitive): `and`, `or`
    *   Grouping: `(...)`
    *   Membership (case-insensitive): `in`, `not in`
    *   Contains / regex match: `~` (contains or regex match), `!~` (does not contain or does not match regex)
    *   Operands: Identifiers, string literals, number literals, date literals (`YYYY-MM-DD`), regex literals (`/pattern/`), `NOW()`
    *   Tuple/List for `in`/`not in`: `(1,2,3)`
    *   String contains with `name ~ nik` and negating does not contains `name !~ nik`
    *   Regex match with `name ~ /.*nik.*/` and negating with ` !~ `
    *   Function calls to `upper(), lower(), now()` so that `lower(name) ~ nik`
    *   SemVer check so that `appVersion >= 5.3.2`
*   Comments: singleline `// ...` and multiline `/* ... */`
*   In Block notation can have multiple rules to evaluate
*   Multi-line rules
*   Inline tests with leaving in comment block annotation `@test flag(context) == true` and it will be tested with `ff test`

## Flagfile

it's a flagfile in your application root folder to control behaviour and feature flagging in your app

VSCode extension for syntax highlighting, linting, validating and running tests: `code --install-extension flagfile-vscode-plugin/flagfile-0.1.0.vsix` needs flagfile-cli installed through `brew install dzhibas/tap/flagfile-cli`

Example DSL syntax

```cpp
// Basic on/off switches
FF-new-ui -> true
FF-beta-features -> false
FF-maintenance-mode -> false

// Configuration values
FF-api-timeout -> 5000
FF-max-retries -> 3
FF-log-level -> "debug"

// Feature variants
FF-button-color -> "blue"
FF-theme-config -> json({
  "primaryColor": "#007bff",
  "secondaryColor": "#6c757d",
  "darkMode": true,
  "animations": true
})

// ==== Complext features with multiple rules block === 

// Geographic-based features
FF-regional-features {
  countryCode == "US" -> json({
    "paymentMethods": ["card", "paypal", "apple_pay"],
    "currency": "USD",
    "taxCalculation": "state_based"
  })
  
  countryCode == "EU" -> json({
    "paymentMethods": ["card", "paypal", "sepa"],
    "currency": "EUR", 
    "taxCalculation": "vat_based"
  })
  
  countryCode in ("NL", "BE", "DE") -> json({
    "paymentMethods": ["card", "ideal", "sepa"],
    "currency": "EUR",
    "taxCalculation": "vat_based",
    "languageOptions": ["en", "nl", "de"]
  })
  
  // Default for other countries
  json({
    "paymentMethods": ["card"],
    "currency": "USD",
    "taxCalculation": "basic"
  })
}


// User tier based features
FF-premium-features {
  tier == "enterprise" and seats >= 100 -> json({
    "features": ["advanced_analytics", "custom_integrations", "priority_support", "sso"],
    "apiRateLimit": 10000,
    "storageLimit": "unlimited"
  })
  
  tier == "premium" -> json({
    "features": ["analytics", "integrations", "priority_support"],
    "apiRateLimit": 5000,
    "storageLimit": "100GB"
  })
  
  tier == "basic" -> json({
    "features": ["basic_analytics"],
    "apiRateLimit": 1000,
    "storageLimit": "10GB"
  })
  
  // Free tier
  json({
    "features": [],
    "apiRateLimit": 100,
    "storageLimit": "1GB"
  })
}

// Time-based feature rollout
FF-new-dashboard {
  // Enable for internal users immediately
  userType == "internal" -> true
  
  // Gradual rollout for external users
  userType == "external" and rolloutPercentage <= 25 and NOW() > "2024-01-15" -> true
  userType == "external" and rolloutPercentage <= 50 and NOW() > "2024-02-01" -> true
  userType == "external" and rolloutPercentage <= 75 and NOW() > "2024-02-15" -> true
  userType == "external" and NOW() > "2024-03-01" -> true
  
  false
}

// A/B testing configuration
FF-checkout-flow {
  // Variant A: Traditional checkout
  experimentGroup == "A" -> json({
    "variant": "traditional",
    "steps": ["cart", "shipping", "payment", "confirmation"],
    "guestCheckout": false
  })
  
  // Variant B: One-page checkout
  experimentGroup == "B" -> json({
    "variant": "onepage", 
    "steps": ["onepage_checkout", "confirmation"],
    "guestCheckout": true
  })
  
  // Control group gets traditional
  json({
    "variant": "traditional",
    "steps": ["cart", "shipping", "payment", "confirmation"],
    "guestCheckout": false
  })
}

// A/B testing configuration
FF-checkout-flow {
  // Variant A: Traditional checkout
  experimentGroup == "A" -> json({
    "variant": "traditional",
    "steps": ["cart", "shipping", "payment", "confirmation"],
    "guestCheckout": false
  })
  
  // Variant B: One-page checkout
  experimentGroup == "B" -> json({
    "variant": "onepage", 
    "steps": ["onepage_checkout", "confirmation"],
    "guestCheckout": true
  })
  
  // Control group gets traditional
  json({
    "variant": "traditional",
    "steps": ["cart", "shipping", "payment", "confirmation"],
    "guestCheckout": false
  })
}

// Device and platform specific features
FF-mobile-features {
  platform == "ios" and appVersion >= "2.1.0" -> json({
    "features": ["push_notifications", "biometric_auth", "offline_mode"],
    "ui": "ios_native"
  })
  
  platform == "android" and appVersion >= "2.1.0" -> json({
    "features": ["push_notifications", "fingerprint_auth", "offline_mode"],
    "ui": "material_design"
  })
  
  platform == "web" and browserName in ("chrome", "firefox", "safari") -> json({
    "features": ["push_notifications", "pwa_install"],
    "ui": "responsive"
  })
  
  // Fallback for older versions or unsupported platforms
  json({
    "features": ["basic_auth"],
    "ui": "basic"
  })
}

// Performance and load based features  
FF-performance-mode {
  // Reduce features under high load
  serverLoad > 80 -> json({
    "enableAnimations": false,
    "enableAnalytics": false,
    "cacheStrategy": "aggressive",
    "imageQuality": "low"
  })
  
  serverLoad > 60 -> json({
    "enableAnimations": true,
    "enableAnalytics": false, 
    "cacheStrategy": "moderate",
    "imageQuality": "medium"
  })
  
  // Normal performance mode
  json({
    "enableAnimations": true,
    "enableAnalytics": true,
    "cacheStrategy": "normal", 
    "imageQuality": "high"
  })
}

// Multi-condition complex feature
FF-advanced-search {
  // Enterprise customers with specific requirements
  tier == "enterprise" and 
  searchVolume > 10000 and 
  dataSize in ("large", "xlarge") and
  region in ("us-east", "eu-west") -> json({
    "searchEngine": "elasticsearch",
    "features": ["fuzzy_search", "autocomplete", "faceted_search", "ml_ranking"],
    "indexStrategy": "distributed",
    "cacheSize": "256MB"
  })
  
  // Premium customers 
  tier == "premium" and searchVolume > 1000 -> json({
    "searchEngine": "solr",
    "features": ["fuzzy_search", "autocomplete", "faceted_search"],
    "indexStrategy": "single_node",
    "cacheSize": "64MB"
  })
  
  // Basic search for everyone else
  json({
    "searchEngine": "basic",
    "features": ["exact_match"],
    "indexStrategy": "in_memory",
    "cacheSize": "16MB"
  })
}
```

```cpp
// once you dont have rules you can use short notation to return boolean
FF-feature-flat-on-off -> true

// can be snake_case as well as kebab-case
FF_feature_can_be_snake_case_213213 -> FALSE

// can be camelCase
FF_featureOneOrTwo -> FALSE

// can be PascalCase
FF_Feature23432 -> TRUE

// you can return non-boolean in this example json. or empty json object json({})
FF-feature-json-variant -> json({"success": true})

// features are forced to start with FF- case-sensitive as
// it allows you later to find all flags through the codebase
FF-feature-name-specifics -> false

// you can have feature with multiple rules in it with default flag value returned in the end
// you can have comments or comment blocks with // or /* comment */
FF-feature-y {
    // if country is NL return True
    countryCode == "NL" -> true
    // else default to false
    false
}

// you can also return different variations (non-boolean) as example json
FF-testing {
    // default variant
    json({"success": true})
}

// and have more complex feature with multiple rules in it and some rules multiline rule, which at the end defaults to false
// aswel capitalize for visibility boolean TRUE/FALSE
FF-feature-complex-ticket-234234 {
    // complex bool expression
    a = b and c=d and (dd not in (1,2,3) or z == "demo car") -> TRUE

    // another one
    z == "demo car" -> FALSE

    // with checking more
    g in (4,5,6) and z == "demo car" -> TRUE

    // and multi-line rule works
    model in (ms,mx,m3,my) and created >= 2024-01-01
        and demo == false -> TRUE

    FALSE
}

// different kind of comments inside
FF-feature1 {
    /* comment like this */
    true
    a == "something" -> false
    false
    json({})
}

/* this is multi-line commented feature
FF-timer-feature {
    // turn on only on evaluation time after 22nd feb
    NOW() > 2024-02-22 -> true
    false
}
*/
```
