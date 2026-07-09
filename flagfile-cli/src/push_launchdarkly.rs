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

use flagfile_lib::transpile::launchdarkly::{
    transpile, LdFlag, LdVariation, RuleTarget, TranspileConfig,
};

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

#[allow(clippy::too_many_arguments)]
pub async fn run_push(
    flagfile_path: &str,
    project_arg: Option<&str>,
    env_arg: Option<&str>,
    secret_arg: Option<&str>,
    config_path: &str,
    flags_filter: &[String],
    debug: bool,
    dry_run: bool,
) {
    if let Err(()) = push(
        flagfile_path,
        project_arg,
        env_arg,
        secret_arg,
        config_path,
        flags_filter,
        debug,
        dry_run,
    )
    .await
    {
        process::exit(1);
    }
}

#[allow(clippy::too_many_arguments)]
async fn push(
    flagfile_path: &str,
    project_arg: Option<&str>,
    env_arg: Option<&str>,
    secret_arg: Option<&str>,
    config_path: &str,
    flags_filter: &[String],
    debug: bool,
    dry_run: bool,
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

    // Read + parse the Flagfile, resolving any @include directives so the
    // merged flag set is pushed.
    let (_raw, resolved) = crate::read_flagfile_resolved(flagfile_path)?;
    let content = resolved.content;
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
        dry_run,
    };
    if debug {
        eprintln!(
            "[debug] LaunchDarkly push: base_url={} project={} target_env={} dry_run={}\n",
            client.base_url,
            client.project_key,
            only_env.as_deref().unwrap_or("<all>"),
            dry_run,
        );
    }
    if dry_run {
        println!("DRY RUN — reading LaunchDarkly state; no changes will be applied.\n");
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
            // In dry-run the plan is printed by sync_flag; just tally here.
            Ok(Outcome::WouldCreate) => created += 1,
            Ok(Outcome::WouldUpdate) => updated += 1,
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
    if dry_run {
        println!(
            "\nDry run ({}): {} to create, {} to update, {} failed. No changes applied.",
            target, created, updated, failed
        );
    } else {
        println!(
            "\nDone ({}): {} created, {} updated, {} failed",
            target, created, updated, failed
        );
    }

    if failed > 0 {
        Err(())
    } else {
        Ok(())
    }
}

enum Outcome {
    Created,
    Updated,
    /// dry-run: the flag would be created (nothing was written).
    WouldCreate,
    /// dry-run: the flag would be updated (nothing was written).
    WouldUpdate,
}

/// Thin LD REST client. Centralises auth and (when `debug`) logs every request
/// method/URL/payload and response status/body to stderr.
struct LdClient {
    http: reqwest::Client,
    base_url: String,
    project_key: String,
    token: String,
    debug: bool,
    /// When set, only reads (GET) are performed; writes are printed as a plan.
    dry_run: bool,
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

    /// Create the flag if absent, otherwise patch its targeting. In dry-run the
    /// read still happens (to tell create from update and resolve LD's live
    /// variation order), but every write is printed as a plan instead of sent.
    async fn sync_flag(&self, flag: &LdFlag, only_env: Option<&str>) -> Result<Outcome, String> {
        match self.get_flag(&flag.key).await? {
            Some(live) => {
                // Reference LD's *existing* variation ordering by value so we
                // never depend on local indices that may have drifted.
                let live_values = extract_variation_values(&live);
                if self.dry_run {
                    self.print_plan(flag, only_env, &live_values, false)?;
                    return Ok(Outcome::WouldUpdate);
                }
                // Existing flag: never touch `on` — a kill-switch toggled in
                // the LD UI must survive the push (the ownership boundary).
                self.patch_flag(flag, only_env, &live_values, false).await?;
                Ok(Outcome::Updated)
            }
            None => {
                // A flag that doesn't exist yet will hold exactly the variations
                // we send, in our order — so local indices line up.
                let live_values: Vec<Value> =
                    flag.variations.iter().map(|v| v.value.clone()).collect();
                if self.dry_run {
                    self.print_plan(flag, only_env, &live_values, true)?;
                    return Ok(Outcome::WouldCreate);
                }
                self.create_flag(flag).await?;
                // Brand-new flag: LD defaults `on` to false, so enable each env
                // we configure — otherwise the targeting we just wrote is inert.
                self.patch_flag(flag, only_env, &live_values, true).await?;
                Ok(Outcome::Created)
            }
        }
    }

    /// Print, without writing anything, what `sync_flag` would send for one flag.
    /// Builds the real request bodies (`build_flag_patch` / `create_body`) so the
    /// preview can't drift from the push path, and surfaces the same errors (e.g.
    /// an `--env` not configured for the flag).
    fn print_plan(
        &self,
        flag: &LdFlag,
        only_env: Option<&str>,
        live_values: &[Value],
        is_create: bool,
    ) -> Result<(), String> {
        // Build first so a bad --env fails before we print a misleading header.
        let patch = build_flag_patch(flag, only_env, live_values, is_create)?;

        let (marker, verb) = if is_create {
            ("+", "CREATE")
        } else {
            ("~", "UPDATE")
        };
        println!("{} {} — would {}", marker, flag.key, verb);
        if is_create {
            let vals: Vec<String> = flag.variations.iter().map(|v| v.value.to_string()).collect();
            println!("    variations: [{}]", vals.join(", "));
        }
        if !patch.var_ops.is_empty() {
            println!(
                "    (+{} variation(s) appended to the live flag)",
                patch.var_ops.len()
            );
        }
        for (env_key, env) in &flag.environments {
            if let Some(only) = only_env {
                if env_key != only {
                    continue;
                }
            }
            let on = if is_create { "on → true" } else { "on untouched" };
            println!(
                "    env '{}': {}, {} rule(s), fallthrough {}, offVariation {}",
                env_key,
                on,
                env.rules.len(),
                describe_target(&env.fallthrough, &flag.variations),
                env.off_variation,
            );
        }
        // Exact request bodies only when --debug, so a plan can be audited.
        if self.debug {
            if is_create {
                eprintln!(
                    "    [payload] POST create:\n{}",
                    serde_json::to_string_pretty(&create_body(flag)).unwrap_or_default()
                );
            }
            if !patch.var_ops.is_empty() {
                eprintln!(
                    "    [payload] PATCH variations:\n{}",
                    serde_json::to_string_pretty(&patch.var_ops).unwrap_or_default()
                );
            }
            eprintln!(
                "    [payload] PATCH targeting:\n{}",
                serde_json::to_string_pretty(&patch.ops).unwrap_or_default()
            );
        }
        println!();
        Ok(())
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
        let body = create_body(flag);
        let (status, text) = self.send(reqwest::Method::POST, &url, Some(&body)).await?;
        if !status.is_success() {
            return Err(format!("create failed ({}): {}", status, text));
        }
        Ok(())
    }

    /// Patch the flag's metadata and per-env targeting. The request bodies are
    /// built by `build_flag_patch` (pure — see it for variation remapping and the
    /// `enable_on` rule). LD forbids changing variations and config in the same
    /// patch, so any new variations go out as their own request first.
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
        let patch = build_flag_patch(flag, only_env, live_values, enable_on)?;
        // 1. Add new variations (separate request — LD rejects mixing the two).
        if !patch.var_ops.is_empty() {
            self.patch(&url, &patch.var_ops).await?;
        }
        // 2. Metadata + targeting, now that any new variations exist.
        self.patch(&url, &patch.ops).await
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

/// The create request body for a flag: flag-level fields only (per-env targeting
/// is applied separately via PATCH). Pure, so `--dry-run` can preview it.
fn create_body(flag: &LdFlag) -> Value {
    let mut body = json!({
        "key": flag.key,
        "name": flag.name,
        "variations": flag.variations,
        "tags": flag.tags,
    });
    if let Some(desc) = &flag.description {
        body["description"] = Value::String(desc.clone());
    }
    body
}

/// The two JSON-Patch request bodies a targeting update sends, in order:
/// `var_ops` (append missing variations — LD forbids mixing this with config
/// changes) then `ops` (metadata + per-env targeting).
struct FlagPatch {
    var_ops: Vec<Value>,
    ops: Vec<Value>,
}

/// Build the patch bodies for one flag against LD's live variation ordering.
/// Pure (no I/O), so it drives both the real push (`patch_flag`) and the
/// `--dry-run` preview (`print_plan`) from one source of truth.
///
/// `live_values` is the flag's current variation values **in LD's order**; every
/// variation reference we emit is resolved against it by value (appending any
/// value LD is missing), so it stays correct even if the Flagfile reordered or
/// changed its returns since the flag was created.
///
/// `enable_on` is set only when the flag was just created: a new LD flag starts
/// with `on: false`, so each configured env must be turned on for the targeting
/// to serve. On updates it is false, so the env's live on/off state (a UI
/// kill-switch) is left untouched.
fn build_flag_patch(
    flag: &LdFlag,
    only_env: Option<&str>,
    live_values: &[Value],
    enable_on: bool,
) -> Result<FlagPatch, String> {
    // Append (never remove/reorder) any variation value LD doesn't have yet, so
    // other environments' index-based targeting keeps working. `mapped` tracks
    // the resulting LD ordering used to resolve indices below.
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

    // Per-environment targeting. Ad-hoc `targets` and the env's `on`/off state
    // are owned by LD at runtime — see the transpile module. We only set `on`
    // for a flag we just created (see `enable_on`); on updates it is left as-is
    // so a kill-switch toggled in the UI survives the push.
    let mut touched_env = false;
    for (env_key, env) in &flag.environments {
        if let Some(only) = only_env {
            if env_key != only {
                continue;
            }
        }
        touched_env = true;
        let base = format!("/environments/{}", env_key);
        if enable_on {
            ops.push(json!({ "op": "replace", "path": format!("{}/on", base), "value": true }));
        }
        let rules: Vec<Value> = env.rules.iter().map(|r| remap_rule(r, &live_index)).collect();
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

    Ok(FlagPatch { var_ops, ops })
}

/// One-line, human-readable description of a rule/fallthrough target for the
/// `--dry-run` plan (shows the served value, not just its index).
fn describe_target(target: &RuleTarget, variations: &[LdVariation]) -> String {
    let val = |idx: usize| {
        variations
            .get(idx)
            .map(|v| v.value.to_string())
            .unwrap_or_else(|| "?".to_string())
    };
    match target {
        RuleTarget::Variation { variation } => {
            format!("→ variation {} ({})", variation, val(*variation))
        }
        RuleTarget::Rollout { rollout } => {
            let parts: Vec<String> = rollout
                .variations
                .iter()
                .map(|wv| format!("{:.1}%→v{}", wv.weight as f64 / 1000.0, wv.variation))
                .collect();
            format!("→ rollout [{}]", parts.join(" "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use flagfile_lib::parse_flagfile::parse_flagfile_with_segments;
    use flagfile_lib::transpile::launchdarkly::{transpile, TranspileConfig};

    /// Transpile a one-flag Flagfile (single env `_` -> "production") to an LdFlag.
    fn one_flag(src: &str) -> LdFlag {
        let (_, parsed) = parse_flagfile_with_segments(src).expect("parse");
        let cfg = TranspileConfig {
            project_key: "p".into(),
            env_keys: BTreeMap::from([("_".into(), "production".into())]),
            default_context_kind: "user".into(),
        };
        transpile(&parsed, &cfg)
            .expect("transpile")
            .into_iter()
            .next()
            .expect("one flag")
    }

    fn has_on_op(ops: &[Value]) -> bool {
        ops.iter().any(|op| {
            op.get("path")
                .and_then(|p| p.as_str())
                .map(|p| p.ends_with("/on"))
                .unwrap_or(false)
        })
    }

    // Regression guard for the ownership boundary (fix #1): a create enables the
    // env; an update must never emit an `on` op, so a UI kill-switch survives.
    #[test]
    fn create_enables_on_update_leaves_it() {
        let flag = one_flag("FF-x {\n    country == \"NL\" -> true\n    false\n}\n");
        let live = [Value::Bool(true), Value::Bool(false)];

        let created = build_flag_patch(&flag, None, &live, true).expect("create patch");
        assert!(has_on_op(&created.ops), "create must enable `on`");

        let updated = build_flag_patch(&flag, None, &live, false).expect("update patch");
        assert!(!has_on_op(&updated.ops), "update must not touch `on`");
    }

    // A variation LD doesn't have yet is appended (never reordered) as its own op.
    #[test]
    fn missing_variation_is_appended() {
        let flag = one_flag("FF-x {\n    country == \"NL\" -> true\n    false\n}\n");
        // LD only knows `true` so far; `false` must be appended.
        let patch = build_flag_patch(&flag, None, &[Value::Bool(true)], false).expect("patch");
        assert_eq!(patch.var_ops.len(), 1);
        assert!(patch.var_ops[0]["path"]
            .as_str()
            .unwrap()
            .starts_with("/variations"));
    }

    // Targeting an env the flag doesn't configure is an error (same as a push).
    #[test]
    fn unconfigured_env_errors() {
        let flag = one_flag("FF-x {\n    true\n}\n");
        let err = build_flag_patch(
            &flag,
            Some("staging"),
            &[Value::Bool(true), Value::Bool(false)],
            false,
        );
        assert!(err.is_err());
    }

    // create_body carries flag-level fields and omits per-env targeting.
    #[test]
    fn create_body_has_flag_level_fields() {
        let flag = one_flag("FF-x {\n    true\n}\n");
        let body = create_body(&flag);
        assert_eq!(body["key"], "FF-x");
        assert!(body.get("variations").is_some());
        assert!(body.get("environments").is_none());
    }
}
