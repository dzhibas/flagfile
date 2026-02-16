use std::process;

use serde::Deserialize;

/// Configuration for remote operations (from ff.toml [remote] section)
#[derive(Debug, Deserialize, Default)]
pub struct RemoteConfig {
    pub url: Option<String>,
    pub namespace: Option<String>,
    pub tokens: Option<RemoteTokens>,
}

#[derive(Debug, Deserialize, Default)]
pub struct RemoteTokens {
    pub read: Option<String>,
    pub write: Option<String>,
}

/// Load remote config from ff.toml
pub fn load_remote_config(config_path: &str) -> RemoteConfig {
    std::fs::read_to_string(config_path)
        .ok()
        .and_then(|content| {
            #[derive(Deserialize)]
            struct FfToml {
                remote: Option<RemoteConfig>,
            }
            toml::from_str::<FfToml>(&content).ok()
        })
        .and_then(|c| c.remote)
        .unwrap_or_default()
}

/// Resolve the write token from: CLI arg > env var > ff.toml config
fn resolve_write_token(secret_arg: Option<&str>, config: &RemoteConfig) -> Option<String> {
    secret_arg
        .map(String::from)
        .or_else(|| std::env::var("FF_WRITE_TOKEN").ok())
        .or_else(|| config.tokens.as_ref().and_then(|t| t.write.clone()))
}

/// Resolve the remote URL from: CLI arg > ff.toml config
pub fn resolve_remote_url(remote_arg: Option<&str>, config: &RemoteConfig) -> Option<String> {
    remote_arg.map(String::from).or_else(|| config.url.clone())
}

pub fn run_push(
    flagfile_path: &str,
    remote_arg: Option<&str>,
    namespace_arg: Option<&str>,
    secret_arg: Option<&str>,
    config_path: &str,
) {
    let config = load_remote_config(config_path);

    let remote = match resolve_remote_url(remote_arg, &config) {
        Some(url) => url,
        None => {
            eprintln!("No remote URL specified. Use --remote or configure [remote] in ff.toml");
            process::exit(1);
        }
    };

    let token = match resolve_write_token(secret_arg, &config) {
        Some(t) => t,
        None => {
            eprintln!("No write token specified. Use --secret, set FF_WRITE_TOKEN, or configure [remote.tokens] in ff.toml");
            process::exit(1);
        }
    };

    let namespace = namespace_arg
        .map(String::from)
        .or_else(|| config.namespace.clone());

    // 1. Read local Flagfile
    let content = match std::fs::read_to_string(flagfile_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("{} does not exist", flagfile_path);
            process::exit(1);
        }
    };

    // 2. Validate syntax locally (fail fast)
    if let Err(e) = flagfile_lib::parse_flagfile::parse_flagfile_with_segments(&content) {
        eprintln!("Validation failed: {}", e);
        process::exit(1);
    }

    // 3. Build URL
    let url = match &namespace {
        Some(ns) => format!("{}/ns/{}/flagfile", remote.trim_end_matches('/'), ns),
        None => format!("{}/flagfile", remote.trim_end_matches('/')),
    };

    // 4. Send PUT request
    let client = reqwest::blocking::Client::new();
    let response = match client
        .put(&url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "text/plain")
        .body(content)
        .send()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to push: {}", e);
            process::exit(1);
        }
    };

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        eprintln!("Push failed ({}): {}", status, body);
        process::exit(1);
    }

    // 5. Parse response
    #[derive(Deserialize)]
    struct PushResponse {
        flags_count: Option<u64>,
        hash: Option<String>,
    }

    let ns_display = namespace.as_deref().unwrap_or("root");
    match response.json::<PushResponse>() {
        Ok(resp) => {
            let count = resp.flags_count.unwrap_or(0);
            let hash = resp.hash.unwrap_or_else(|| "unknown".to_string());
            println!("✓ Pushed {} flags to {} (hash: {})", count, ns_display, hash);
        }
        Err(_) => {
            println!("✓ Pushed to {}", ns_display);
        }
    }
}
