//! Push transpiled Flagfile flags to the LaunchDarkly REST API.
//!
//! For every flag the local Flagfile transpiles to (`flagfile_lib::transpile::
//! launchdarkly`), we either CREATE it (if it doesn't exist in the LD project)
//! or UPDATE it (semantic targeting via JSON Patch). `--env` narrows the update
//! to a single LD environment; without it every configured environment is
//! synced.
//!
//! NB: managing flags requires a LaunchDarkly *API access token* with write
//! access — an SDK key cannot create or update flags.

use std::collections::BTreeMap;
use std::process;

use serde::Deserialize;
use serde_json::{json, Value};

use flagfile_lib::transpile::launchdarkly::{transpile, LdFlag, RuleTarget, TranspileConfig};

const DEFAULT_BASE_URL: &str = "https://app.launchdarkly.com";

/// `[launchdarkly]` section of ff.toml.
#[derive(Debug, Deserialize, Default)]
pub struct LaunchDarklyConfig {
    pub project_key: Option<String>,
    /// API access token. Prefer the env var over committing this.
    pub api_token: Option<String>,
    /// Override the LD host (e.g. for federal/relay setups).
    pub base_url: Option<String>,
    /// Context kind attached to every generated clause. Defaults to "user".
    pub context_kind: Option<String>,
    /// Flagfile `@env` name -> LD environment key.
    #[serde(default)]
    pub environments: BTreeMap<String, String>,
}

/// Load `[launchdarkly]` from ff.toml (missing file / section -> defaults).
pub fn load_config(config_path: &str) -> LaunchDarklyConfig {
    std::fs::read_to_string(config_path)
        .ok()
        .and_then(|content| {
            #[derive(Deserialize)]
            struct FfToml {
                launchdarkly: Option<LaunchDarklyConfig>,
            }
            toml::from_str::<FfToml>(&content).ok()
        })
        .and_then(|c| c.launchdarkly)
        .unwrap_or_default()
}

/// Resolve the API token from: CLI arg > env var > ff.toml.
fn resolve_token(secret_arg: Option<&str>, config: &LaunchDarklyConfig) -> Option<String> {
    secret_arg
        .map(String::from)
        .or_else(|| std::env::var("LAUNCHDARKLY_API_TOKEN").ok())
        .or_else(|| std::env::var("LD_API_TOKEN").ok())
        .or_else(|| config.api_token.clone())
}

pub async fn run_push(
    flagfile_path: &str,
    project_arg: Option<&str>,
    env_arg: Option<&str>,
    secret_arg: Option<&str>,
    config_path: &str,
    flags_filter: &[String],
    debug: bool,
) {
    if let Err(()) = push(
        flagfile_path,
        project_arg,
        env_arg,
        secret_arg,
        config_path,
        flags_filter,
        debug,
    )
    .await
    {
        process::exit(1);
    }
}

async fn push(
    flagfile_path: &str,
    project_arg: Option<&str>,
    env_arg: Option<&str>,
    secret_arg: Option<&str>,
    config_path: &str,
    flags_filter: &[String],
    debug: bool,
) -> Result<(), ()> {
    let config = load_config(config_path);

    let project_key = project_arg
        .map(String::from)
        .or_else(|| config.project_key.clone())
        .ok_or_else(|| {
            eprintln!("No LaunchDarkly project. Use --project or set [launchdarkly] project_key in ff.toml");
        })?;

    let token = resolve_token(secret_arg, &config).ok_or_else(|| {
        eprintln!("No LaunchDarkly API token. Use --secret, set LAUNCHDARKLY_API_TOKEN, or set [launchdarkly] api_token in ff.toml (needs an API access token, not an SDK key)");
    })?;

    if config.environments.is_empty() {
        eprintln!("No environment mapping. Configure [launchdarkly.environments] in ff.toml, e.g.\n\n  [launchdarkly.environments]\n  prod = \"production\"\n  dev = \"test\"");
        return Err(());
    }

    let base_url = config
        .base_url
        .clone()
        .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
    let base_url = base_url.trim_end_matches('/').to_string();

    // Resolve --env (a Flagfile env name, or a literal LD env key) to the LD
    // environment key we'll patch.
    let only_env: Option<String> = match env_arg {
        Some(name) => Some(
            config
                .environments
                .get(name)
                .cloned()
                .unwrap_or_else(|| name.to_string()),
        ),
        None => None,
    };

    // Read + parse the Flagfile.
    let content = std::fs::read_to_string(flagfile_path).map_err(|_| {
        eprintln!("{} does not exist", flagfile_path);
    })?;
    let (rest, mut parsed) = flagfile_lib::parse_flagfile::parse_flagfile_with_segments(&content)
        .map_err(|e| {
        eprintln!("Validation failed: {}", e);
    })?;
    if !rest.trim().is_empty() {
        eprintln!("Validation failed: unparsed trailing content");
        return Err(());
    }

    // Optionally narrow to an explicit set of flags (--flags a,b,c). Filtering
    // here (before transpile) means unrelated, untranspilable flags don't block
    // a targeted push.
    let wanted: std::collections::HashSet<&str> = flags_filter
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if !wanted.is_empty() {
        let present: std::collections::HashSet<&str> = parsed
            .flags
            .iter()
            .flat_map(|m| m.keys().copied())
            .collect();
        for name in &wanted {
            if !present.contains(name) {
                eprintln!(
                    "Warning: requested flag '{}' not found in {}",
                    name, flagfile_path
                );
            }
        }
        for map in parsed.flags.iter_mut() {
            map.retain(|k, _| wanted.contains(k));
        }
        parsed.flags.retain(|m| !m.is_empty());

        if parsed.flags.is_empty() {
            eprintln!("None of the requested flags were found.");
            return Err(());
        }
    }

    // Transpile to the LD target model.
    let cfg = TranspileConfig {
        project_key: project_key.clone(),
        env_keys: config
            .environments
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        default_context_kind: config
            .context_kind
            .clone()
            .unwrap_or_else(|| "user".to_string()),
    };
    let flags = match transpile(&parsed, &cfg) {
        Ok(flags) => flags,
        Err(errors) => {
            eprintln!(
                "{} flag(s) cannot be represented in LaunchDarkly:",
                errors.len()
            );
            for e in &errors {
                eprintln!("  - {:?}", e);
            }
            return Err(());
        }
    };

    if flags.is_empty() {
        println!("No flags to push.");
        return Ok(());
    }

    let client = LdClient {
        http: reqwest::Client::new(),
        base_url,
        project_key: project_key.clone(),
        token,
        debug,
    };
    if debug {
        eprintln!(
            "[debug] LaunchDarkly push: base_url={} project={} target_env={}\n",
            client.base_url,
            client.project_key,
            only_env.as_deref().unwrap_or("<all>")
        );
    }

    let mut created = 0usize;
    let mut updated = 0usize;
    let mut failed = 0usize;

    for flag in &flags {
        match client.sync_flag(flag, only_env.as_deref()).await {
            Ok(Outcome::Created) => {
                created += 1;
                println!("✓ created {}", flag.key);
            }
            Ok(Outcome::Updated) => {
                updated += 1;
                println!("✓ updated {}", flag.key);
            }
            Err(e) => {
                failed += 1;
                eprintln!("✗ {}: {}", flag.key, e);
            }
        }
    }

    let target = match &only_env {
        Some(env) => format!("project '{}' env '{}'", project_key, env),
        None => format!("project '{}'", project_key),
    };
    println!(
        "\nDone ({}): {} created, {} updated, {} failed",
        target, created, updated, failed
    );

    if failed > 0 {
        Err(())
    } else {
        Ok(())
    }
}

enum Outcome {
    Created,
    Updated,
}

/// Thin LD REST client. Centralises auth and (when `debug`) logs every request
/// method/URL/payload and response status/body to stderr.
struct LdClient {
    http: reqwest::Client,
    base_url: String,
    project_key: String,
    token: String,
    debug: bool,
}

impl LdClient {
    /// Send a request, optionally with a JSON body, returning (status, body).
    /// Logs the call when `debug` is on. The auth token is never printed.
    async fn send(
        &self,
        method: reqwest::Method,
        url: &str,
        body: Option<&Value>,
    ) -> Result<(reqwest::StatusCode, String), String> {
        if self.debug {
            eprintln!("→ {} {}", method, url);
            if let Some(b) = body {
                eprintln!(
                    "  payload: {}",
                    serde_json::to_string_pretty(b).unwrap_or_else(|_| b.to_string())
                );
            }
        }

        let mut req = self
            .http
            .request(method.clone(), url)
            .header("Authorization", &self.token);
        if let Some(b) = body {
            req = req.json(b);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| format!("{} {}: {}", method, url, e))?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();

        if self.debug {
            eprintln!(
                "← {} {}\n",
                status.as_u16(),
                if text.is_empty() {
                    "(empty body)"
                } else {
                    text.as_str()
                }
            );
        }

        Ok((status, text))
    }

    /// Create the flag if absent, otherwise patch its targeting.
    async fn sync_flag(&self, flag: &LdFlag, only_env: Option<&str>) -> Result<Outcome, String> {
        match self.get_flag(&flag.key).await? {
            Some(live) => {
                // Reference LD's *existing* variation ordering by value so we
                // never depend on local indices that may have drifted.
                let live_values = extract_variation_values(&live);
                // Existing flag: never touch `on` — a kill-switch toggled in
                // the LD UI must survive the push (the ownership boundary).
                self.patch_flag(flag, only_env, &live_values, false).await?;
                Ok(Outcome::Updated)
            }
            None => {
                self.create_flag(flag).await?;
                // A freshly created flag holds exactly the variations we sent,
                // in our order — so local indices line up.
                let live_values: Vec<Value> =
                    flag.variations.iter().map(|v| v.value.clone()).collect();
                // Brand-new flag: LD defaults `on` to false, so enable each env
                // we configure — otherwise the targeting we just wrote is inert.
                self.patch_flag(flag, only_env, &live_values, true).await?;
                Ok(Outcome::Created)
            }
        }
    }

    /// GET the flag: `Some(json)` if it exists, `None` on 404.
    async fn get_flag(&self, flag_key: &str) -> Result<Option<Value>, String> {
        let url = format!(
            "{}/api/v2/flags/{}/{}",
            self.base_url, self.project_key, flag_key
        );
        let (status, body) = self.send(reqwest::Method::GET, &url, None).await?;
        match status.as_u16() {
            200 => serde_json::from_str(&body)
                .map(Some)
                .map_err(|e| format!("parse flag: {}", e)),
            404 => Ok(None),
            401 | 403 => Err(format!(
                "{} unauthorized: {}\n  Hint: the LD REST API needs an API *access token* with write \
access (Account settings → Authorization → Access tokens), not an SDK key/client-side ID.",
                status.as_u16(),
                body
            )),
            code => Err(format!("GET flag returned {}: {}", code, body)),
        }
    }

    async fn create_flag(&self, flag: &LdFlag) -> Result<(), String> {
        let url = format!("{}/api/v2/flags/{}", self.base_url, self.project_key);

        // The create endpoint sets flag-level fields only; per-env targeting is
        // applied afterwards via PATCH.
        let mut body = json!({
            "key": flag.key,
            "name": flag.name,
            "variations": flag.variations,
            "tags": flag.tags,
        });
        if let Some(desc) = &flag.description {
            body["description"] = Value::String(desc.clone());
        }

        let (status, text) = self.send(reqwest::Method::POST, &url, Some(&body)).await?;
        if !status.is_success() {
            return Err(format!("create failed ({}): {}", status, text));
        }
        Ok(())
    }

    /// Patch the flag's metadata and per-env targeting. `live_values` is the
    /// flag's current variation values **in LD's order**; all variation
    /// references we emit are resolved against it by value (appending any value
    /// LD is missing), so they stay correct even if the Flagfile reordered or
    /// changed its returns since the flag was created.
    ///
    /// `enable_on` is set only when the flag was just created: a new LD flag
    /// starts with `on: false`, so we must turn each configured env on for the
    /// targeting to serve. On updates it is false, so the env's live on/off
    /// state (a UI kill-switch) is left untouched.
    async fn patch_flag(
        &self,
        flag: &LdFlag,
        only_env: Option<&str>,
        live_values: &[Value],
        enable_on: bool,
    ) -> Result<(), String> {
        let url = format!(
            "{}/api/v2/flags/{}/{}",
            self.base_url, self.project_key, flag.key
        );

        // Append (never remove/reorder) any variation value LD doesn't have yet,
        // so other environments' index-based targeting keeps working. `mapped`
        // tracks the resulting LD ordering used to resolve indices below. LD
        // forbids changing variations and config in the same patch, so these go
        // out as their own request first.
        let mut var_ops: Vec<Value> = Vec::new();
        let mut mapped: Vec<Value> = live_values.to_vec();
        for v in &flag.variations {
            if !mapped.iter().any(|x| x == &v.value) {
                var_ops.push(
                    json!({ "op": "add", "path": "/variations/-", "value": { "value": v.value } }),
                );
                mapped.push(v.value.clone());
            }
        }
        // local variation index -> LD variation index, resolved by value.
        let live_index = |local: usize| -> usize {
            match flag.variations.get(local) {
                Some(var) => mapped.iter().position(|x| x == &var.value).unwrap_or(0),
                None => 0,
            }
        };

        let mut ops: Vec<Value> = Vec::new();

        // Flag-level metadata (kept in sync from the Flagfile).
        ops.push(json!({ "op": "replace", "path": "/name", "value": flag.name }));
        ops.push(json!({ "op": "replace", "path": "/tags", "value": flag.tags }));
        if let Some(desc) = &flag.description {
            ops.push(json!({ "op": "replace", "path": "/description", "value": desc }));
        }

        // Per-environment targeting. Ad-hoc `targets` and the env's `on`/off
        // state are owned by LD at runtime — see the transpile module. We only
        // set `on` for a flag we just created (see `enable_on`); on updates it
        // is left as-is so a kill-switch toggled in the UI survives the push.
        let mut touched_env = false;
        for (env_key, env) in &flag.environments {
            if let Some(only) = only_env {
                if env_key != only {
                    continue;
                }
            }
            touched_env = true;
            let base = format!("/environments/{}", env_key);
            // Newly created flag only: turn the env on so the targeting we just
            // wrote actually serves (LD defaults a new flag's `on` to false).
            if enable_on {
                ops.push(json!({ "op": "replace", "path": format!("{}/on", base), "value": true }));
            }
            let rules: Vec<Value> = env
                .rules
                .iter()
                .map(|r| remap_rule(r, &live_index))
                .collect();
            ops.push(json!({ "op": "replace", "path": format!("{}/rules", base), "value": rules }));
            ops.push(json!({ "op": "replace", "path": format!("{}/fallthrough", base), "value": remap_target(&env.fallthrough, &live_index) }));
            ops.push(json!({ "op": "replace", "path": format!("{}/offVariation", base), "value": live_index(env.off_variation) }));
            ops.push(json!({ "op": "replace", "path": format!("{}/prerequisites", base), "value": env.prerequisites }));
        }

        if let Some(only) = only_env {
            if !touched_env {
                return Err(format!(
                    "environment '{}' is not configured for this flag",
                    only
                ));
            }
        }

        // 1. Add new variations (separate request — LD rejects mixing the two).
        if !var_ops.is_empty() {
            self.patch(&url, &var_ops).await?;
        }
        // 2. Metadata + targeting, now that any new variations exist.
        self.patch(&url, &ops).await
    }

    /// Send a JSON-Patch array to the flag and check the result.
    async fn patch(&self, url: &str, ops: &[Value]) -> Result<(), String> {
        let body = Value::Array(ops.to_vec());
        let (status, text) = self.send(reqwest::Method::PATCH, url, Some(&body)).await?;
        if !status.is_success() {
            return Err(format!("update failed ({}): {}", status, text));
        }
        Ok(())
    }
}

/// Variation values from a live LD flag JSON, in LD's order.
fn extract_variation_values(flag: &Value) -> Vec<Value> {
    flag.get("variations")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|v| v.get("value").cloned().unwrap_or(Value::Null))
                .collect()
        })
        .unwrap_or_default()
}

/// Serialize a rule target (fixed variation or rollout) with indices remapped
/// to LD's live variation ordering.
fn remap_target(target: &RuleTarget, live_index: &impl Fn(usize) -> usize) -> Value {
    match target {
        RuleTarget::Variation { variation } => json!({ "variation": live_index(*variation) }),
        RuleTarget::Rollout { rollout } => {
            let variations: Vec<Value> = rollout
                .variations
                .iter()
                .map(|wv| json!({ "variation": live_index(wv.variation), "weight": wv.weight }))
                .collect();
            let mut obj = serde_json::Map::new();
            let mut roll = serde_json::Map::new();
            roll.insert("variations".into(), Value::Array(variations));
            if let Some(bucket_by) = &rollout.bucket_by {
                roll.insert("bucketBy".into(), Value::String(bucket_by.clone()));
            }
            obj.insert("rollout".into(), Value::Object(roll));
            Value::Object(obj)
        }
    }
}

/// Serialize an LD rule (clauses + remapped target) as a single JSON object.
fn remap_rule(
    rule: &flagfile_lib::transpile::launchdarkly::LdRule,
    live_index: &impl Fn(usize) -> usize,
) -> Value {
    let mut obj = serde_json::Map::new();
    if let Some(desc) = &rule.description {
        obj.insert("description".into(), Value::String(desc.clone()));
    }
    obj.insert(
        "clauses".into(),
        serde_json::to_value(&rule.clauses).unwrap_or(Value::Array(vec![])),
    );
    // RuleTarget is #[serde(flatten)]ed onto the rule, so merge its keys in.
    if let Value::Object(target) = remap_target(&rule.target, live_index) {
        obj.extend(target);
    }
    Value::Object(obj)
}
