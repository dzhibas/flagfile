use std::path::PathBuf;
use std::process;
use std::sync::Arc;

use notify::{EventKind, RecursiveMode, Watcher};

use super::state::AppState;

pub fn parse_flags(content: &str) -> Option<super::state::ParsedFlags> {
    use std::collections::HashMap;

    use flagfile_lib::ast::FlagMetadata;
    use flagfile_lib::parse_flagfile::{parse_flagfile_with_segments, Rule};

    let (remainder, parsed) = match parse_flagfile_with_segments(content) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Warning: reload parse error: {}", e);
            return None;
        }
    };

    if !remainder.trim().is_empty() {
        eprintln!(
            "Warning: reload failed: unexpected content near: {}",
            remainder.trim().lines().next().unwrap_or("")
        );
        return None;
    }

    let mut flags: HashMap<String, Vec<Rule>> = HashMap::new();
    let mut metadata: HashMap<String, FlagMetadata> = HashMap::new();
    for fv in &parsed.flags {
        for (name, def) in fv.iter() {
            flags.insert(name.to_string(), def.rules.clone());
            metadata.insert(name.to_string(), def.metadata.clone());
        }
    }
    Some((flags, metadata, parsed.segments))
}

/// Check whether a notify event touches a `Flagfile*` file.
fn event_affects_flagfile(event: &notify::Event) -> bool {
    event.paths.iter().any(|p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("Flagfile"))
    })
}

pub async fn watch_flagfile(state: Arc<AppState>, path: PathBuf) {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(1);

    let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, _>| {
        if let Ok(event) = res {
            if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_))
                && event_affects_flagfile(&event)
            {
                let _ = tx.try_send(());
            }
        }
    })
    .unwrap_or_else(|e| {
        eprintln!("Failed to create file watcher: {}", e);
        process::exit(1);
    });

    let watch_path = path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    watcher
        .watch(&watch_path, RecursiveMode::NonRecursive)
        .unwrap_or_else(|e| {
            eprintln!("Failed to watch {}: {}", watch_path.display(), e);
            process::exit(1);
        });

    println!("Watching {} for changes", path.display());

    // Keep watcher alive for the lifetime of this task
    let _watcher = watcher;

    loop {
        // Wait for a change notification
        if rx.recv().await.is_none() {
            break;
        }

        // Debounce: wait a bit and drain any extra events
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        while rx.try_recv().is_ok() {}

        // Re-read and re-parse (non-blocking)
        let content = match tokio::fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Warning: failed to read {}: {}", path.display(), e);
                continue;
            }
        };

        match parse_flags(&content) {
            Some((flags, metadata, segments)) => {
                let mut store = state.store.write().await;
                store.flagfile_content = content;
                store.flags = flags;
                store.metadata = metadata;
                store.segments = segments;
                println!("Flagfile reloaded successfully");
            }
            None => {
                // parse_flags already printed the warning
            }
        }
    }
}
