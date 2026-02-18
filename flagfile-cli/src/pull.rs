use std::process;

use crate::push::{load_remote_config, resolve_remote_url};

/// Resolve the read token from: CLI arg > env var > ff.toml config
fn resolve_read_token(
    secret_arg: Option<&str>,
    config: &crate::push::RemoteConfig,
) -> Option<String> {
    secret_arg
        .map(String::from)
        .or_else(|| std::env::var("FF_READ_TOKEN").ok())
        .or_else(|| config.tokens.as_ref().and_then(|t| t.read.clone()))
}

pub async fn run_pull(
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

    let token = match resolve_read_token(secret_arg, &config) {
        Some(t) => t,
        None => {
            eprintln!("No read token specified. Use --secret, set FF_READ_TOKEN, or configure [remote.tokens] in ff.toml");
            process::exit(1);
        }
    };

    let namespace = namespace_arg
        .map(String::from)
        .or_else(|| config.namespace.clone());

    // 1. Build URL
    let url = match &namespace {
        Some(ns) => format!("{}/ns/{}/flagfile", remote.trim_end_matches('/'), ns),
        None => format!("{}/flagfile", remote.trim_end_matches('/')),
    };

    // 2. Send GET request
    let client = reqwest::Client::new();
    let response = match client
        .get(&url)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to pull: {}", e);
            process::exit(1);
        }
    };

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        eprintln!("Pull failed ({}): {}", status, body);
        process::exit(1);
    }

    // 3. Write response body to output file
    let body = match response.text().await {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Failed to read response body: {}", e);
            process::exit(1);
        }
    };

    if let Err(e) = std::fs::write(flagfile_path, &body) {
        eprintln!("Failed to write {}: {}", flagfile_path, e);
        process::exit(1);
    }

    let ns_display = namespace.as_deref().unwrap_or("root");
    println!("✓ Pulled from {} to {}", ns_display, flagfile_path);
}
