# <img src="https://github.com/dzhibas/flagfile/blob/main/public/flagfile.svg?raw=true" width=100px/> Flagfile

![Tests passing](https://github.com/dzhibas/flagfile/actions/workflows/tests.yml/badge.svg)

it's developer friendly feature flagging solution where you define all your flags in Flagfile in this format: [Flagfile.example](Flagfile.example)

its boolean expression parser library which was initially written in pest.rs (https://github.com/dzhibas/bool_expr_parser) and later rewrote everything in Nom rust lib

Feature rules can be describe in a expresions similar to all developers and DevOps and does not need any intermediate json format to express these
```
country == NL and created > 2024-02-15 and userId not in (122133, 122132323, 2323423)
```

```rust
let rule = "country == NL and created > 2024-02-15 and userId not in (122133, 122132323, 2323423)";
let (i, expr) = parse(&rule).expect("parse error");
let flag_value = eval(&expr, &HashMap::from([("country", "NL"), ("userId", "2132321"), ("created", "2024-02-02")]);
dbg!(flag_value);
```

## Usage as a Rust library

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
flagfile-lib = "0.3"
```

Then use `init()` to load a `Flagfile` from the current directory and `ff()` to evaluate flags:

```rust
use std::collections::HashMap;
use flagfile_lib::{init, ff, FlagReturn, Context};
use flagfile_lib::ast::Atom;

fn main() {
    // Reads and parses "Flagfile" from the current directory
    flagfile_lib::init();

    // Build a context with runtime values
    let ctx: Context = HashMap::from([
        ("tier", "premium".into()),
        ("country", "NL".into()),
    ]);

    match flagfile_lib::ff("FF-premium-features", &ctx) {
        Some(FlagReturn::OnOff(true)) => println!("Flag is on"),
        Some(FlagReturn::OnOff(false)) => println!("Flag is off"),
        Some(FlagReturn::Json(v)) => println!("Config: {}", v),
        Some(FlagReturn::Integer(n)) => println!("Value: {}", n),
        Some(FlagReturn::Str(s)) => println!("String: {}", s),
        None => println!("Flag not found or no rule matched"),
    }
}
```

## Features language summary:

*   Feature flag name definition: `FF-<name>` or `FF_<name>`
*   Short notation: `FF-name -> value`
*   Block notation: `FF-name { rules... defaultValue }`
*   Values: `true`, `TRUE`, `false`, `FALSE`, `json({"key": "value"})`
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
*   Comments: singleline `// ...` and multiline `/* ... */`
*   In Block notation can have multiple rules to evaluate
*   Multi-line rules

## Flagfile

it's a flagfile in your application root folder to control behaviour and feature flagging in your app

Flagfile.example (with comments):

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
