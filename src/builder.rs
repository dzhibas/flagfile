pub struct FlagfileBuilder {
    file: String,
    remote: Option<String>,
    token: Option<String>,
    namespace: Option<String>,
    env: Option<String>,
    fallback: String,
    consumed: bool,
    #[cfg(feature = "remote")]
    on_update: Option<Box<dyn Fn() + Send + 'static>>,
}

pub fn create_builder() -> FlagfileBuilder {
    FlagfileBuilder {
        file: "Flagfile".into(),
        remote: None,
        token: None,
        namespace: None,
        env: None,
        fallback: "Flagfile".into(),
        consumed: false,
        #[cfg(feature = "remote")]
        on_update: None,
    }
}

impl FlagfileBuilder {
    pub fn remote(mut self, url: &str) -> Self {
        self.remote = Some(url.to_string());
        self
    }

    pub fn token(mut self, token: &str) -> Self {
        self.token = Some(token.to_string());
        self
    }

    pub fn namespace(mut self, ns: &str) -> Self {
        self.namespace = Some(ns.to_string());
        self
    }

    pub fn env(mut self, env: &str) -> Self {
        self.env = Some(env.to_string());
        self
    }

    pub fn file(mut self, path: &str) -> Self {
        self.file = path.to_string();
        self
    }

    pub fn fallback(mut self, path: &str) -> Self {
        self.fallback = path.to_string();
        self
    }

    /// Register a callback that fires after each successful remote reload.
    /// The callback runs on the background SSE thread.
    #[cfg(feature = "remote")]
    pub fn on_update(mut self, cb: impl Fn() + Send + 'static) -> Self {
        self.on_update = Some(Box::new(cb));
        self
    }
}

impl Drop for FlagfileBuilder {
    fn drop(&mut self) {
        if self.consumed {
            return;
        }
        self.consumed = true;

        match &self.remote {
            None => {
                // Local mode — read file, parse, store in global state
                let content = std::fs::read_to_string(&self.file)
                    .unwrap_or_else(|_| panic!("Could not read '{}'", self.file));
                super::init_from_str_inner(&content, self.env.clone());
            }
            Some(url) => {
                // Remote mode — requires "remote" feature
                #[cfg(feature = "remote")]
                {
                    let url = url.clone();
                    let token = self.token.clone();
                    let namespace = self.namespace.clone();
                    let env = self.env.clone();
                    let fallback = self.fallback.clone();

                    let flagfile_url = match &namespace {
                        Some(ns) => format!("{}/ns/{}/flagfile", url, ns),
                        None => format!("{}/flagfile", url),
                    };
                    let events_url = match &namespace {
                        Some(ns) => format!("{}/ns/{}/events", url, ns),
                        None => format!("{}/events", url),
                    };

                    // Try fetching from remote, fall back to local file on failure.
                    let client = reqwest::blocking::Client::new();
                    let mut request = client.get(&flagfile_url);
                    if let Some(ref t) = token {
                        request = request.bearer_auth(t);
                    }

                    let remote_ok = match request
                        .send()
                        .and_then(|r| r.error_for_status())
                        .and_then(|r| r.text())
                    {
                        Ok(content) => {
                            super::init_from_str_inner(&content, env.clone());
                            true
                        }
                        Err(e) => {
                            eprintln!(
                                "flagfile: remote fetch failed: {}, using fallback '{}'",
                                e, fallback
                            );
                            let content = std::fs::read_to_string(&fallback).unwrap_or_else(|_| {
                                panic!("Could not read fallback '{}'", fallback)
                            });
                            super::init_from_str_inner(&content, env.clone());
                            false
                        }
                    };

                    // Only subscribe to SSE if the initial fetch succeeded.
                    // If the server was unreachable / 404 there is no point
                    // in trying to open an event stream.
                    if remote_ok {
                        let on_update = self.on_update.take();
                        std::thread::spawn(move || {
                            sse_listener(
                                &events_url,
                                &flagfile_url,
                                token.as_deref(),
                                env,
                                on_update.as_deref(),
                            );
                        });
                    }
                }
                #[cfg(not(feature = "remote"))]
                {
                    let _ = url;
                    panic!("Remote mode requires the 'remote' feature. Enable it in Cargo.toml: flagfile-lib = {{ features = [\"remote\"] }}");
                }
            }
        }
    }
}

/// Background SSE listener that reconnects with exponential backoff.
/// On each `flag_update` event, re-fetches the flagfile content and reloads
/// the global state. On `server_shutdown`, breaks out and reconnects.
#[cfg(feature = "remote")]
fn sse_listener(
    events_url: &str,
    flagfile_url: &str,
    token: Option<&str>,
    env: Option<String>,
    on_update: Option<&(dyn Fn() + Send)>,
) {
    use std::io::{BufRead, BufReader};
    use std::time::Duration;

    const BASE_DELAY_MS: u64 = 1_000;
    const MAX_DELAY_MS: u64 = 30_000;

    let client = reqwest::blocking::Client::new();
    let mut attempt: u32 = 0;

    loop {
        let mut request = client.get(events_url).header("Accept", "text/event-stream");
        if let Some(t) = token {
            request = request.bearer_auth(t);
        }

        match request.send().and_then(|r| r.error_for_status()) {
            Ok(resp) => {
                // Connected successfully — reset backoff
                attempt = 0;

                let reader = BufReader::new(resp);
                let mut event_type = String::new();
                let mut shutdown = false;

                for line in reader.lines() {
                    match line {
                        Ok(line) => {
                            if line.starts_with(':') {
                                // SSE comment (keep-alive), ignore
                            } else if let Some(ev) = line.strip_prefix("event: ") {
                                event_type = ev.trim().to_string();
                            } else if line.starts_with("data: ") {
                                if event_type == "flag_update" {
                                    reload_from_remote(
                                        &client,
                                        flagfile_url,
                                        token,
                                        &env,
                                        on_update,
                                    );
                                } else if event_type == "server_shutdown" {
                                    shutdown = true;
                                    break;
                                }
                                event_type.clear();
                            } else if line.is_empty() {
                                event_type.clear();
                            }
                        }
                        Err(e) => {
                            eprintln!("flagfile: SSE read error: {}, reconnecting...", e);
                            break;
                        }
                    }
                }

                if shutdown {
                    // Server is restarting — try to refresh flags once before
                    // entering the backoff loop.
                    reload_from_remote(&client, flagfile_url, token, &env, on_update);
                }
            }
            Err(e) => {
                eprintln!("flagfile: SSE connection failed: {}", e);
            }
        }

        // Exponential backoff: 1s, 2s, 4s, 8s, … capped at 30s
        let delay_ms = BASE_DELAY_MS.saturating_mul(1u64.checked_shl(attempt).unwrap_or(u64::MAX));
        let delay = Duration::from_millis(delay_ms.min(MAX_DELAY_MS));
        attempt = attempt.saturating_add(1);
        std::thread::sleep(delay);
    }
}

/// Fetch the flagfile from the remote server and reload global state.
#[cfg(feature = "remote")]
fn reload_from_remote(
    client: &reqwest::blocking::Client,
    flagfile_url: &str,
    token: Option<&str>,
    env: &Option<String>,
    on_update: Option<&(dyn Fn() + Send)>,
) {
    let mut request = client.get(flagfile_url);
    if let Some(t) = token {
        request = request.bearer_auth(t);
    }
    match request
        .send()
        .and_then(|r| r.error_for_status())
        .and_then(|r| r.text())
    {
        Ok(content) => match super::parse_and_store(&content, env.clone()) {
            Ok(()) => {
                eprintln!("flagfile: reloaded from remote");
                if let Some(cb) = on_update {
                    cb();
                }
            }
            Err(e) => {
                eprintln!("flagfile: reload parse error: {}", e);
            }
        },
        Err(e) => {
            eprintln!("flagfile: reload fetch failed: {}", e);
        }
    }
}
