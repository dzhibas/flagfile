use std::process::{self, Command};

const DOCS_BASE_URL: &str = "https://github.com/dzhibas/flagfile/blob/main/docs/syntax";

/// Syntax guide sections: (file name, extra aliases).
/// A topic also matches by prefix of the file slug or any of its words,
/// so `env` → `12-environments.md` and `segment` → `11-segments.md`.
const TOPICS: &[(&str, &[&str])] = &[
    ("01-getting-started.md", &["start", "intro", "naming", "comments"]),
    ("02-return-types.md", &["types", "json", "variants"]),
    ("03-rules-and-defaults.md", &["default", "fallthrough"]),
    ("04-comparisons.md", &["compare", "operators", "semver", "dates"]),
    ("05-logic-and-grouping.md", &["and", "or", "not", "grouping"]),
    ("06-string-matching.md", &["regex", "contains", "match"]),
    ("07-arrays-membership.md", &["in", "array", "membership"]),
    ("08-null-checks.md", &["null"]),
    ("09-functions.md", &["lower", "upper", "now", "coalesce"]),
    ("10-percentage-rollouts.md", &["percent", "rollout"]),
    ("11-segments.md", &[]),
    ("12-environments.md", &["env"]),
    ("13-annotations.md", &["owner", "expiry", "metadata"]),
    ("14-tests.md", &["test"]),
    ("15-includes.md", &["include"]),
];

/// Strip the `NN-` prefix and `.md` suffix: `12-environments.md` → `environments`.
fn slug(file: &str) -> &str {
    let name = file.trim_end_matches(".md");
    name.split_once('-').map_or(name, |(_, rest)| rest)
}

fn find_topic(query: &str) -> Result<&'static str, Vec<&'static str>> {
    let q = query.trim().to_lowercase();

    // Exact section number: `ff docs 12`
    if let Ok(n) = q.parse::<usize>() {
        return match TOPICS.get(n.wrapping_sub(1)) {
            Some((file, _)) => Ok(file),
            None => Err(vec![]),
        };
    }

    // Exact slug or alias wins outright
    if let Some((file, _)) = TOPICS
        .iter()
        .find(|(file, aliases)| slug(file) == q || aliases.contains(&q.as_str()))
    {
        return Ok(file);
    }

    // Otherwise prefix match on the slug or any of its words
    let matches: Vec<&'static str> = TOPICS
        .iter()
        .filter(|(file, _)| {
            let s = slug(file);
            s.starts_with(&q) || s.split('-').any(|w| w.starts_with(&q))
        })
        .map(|(file, _)| *file)
        .collect();

    match matches.as_slice() {
        [file] => Ok(file),
        _ => Err(matches),
    }
}

fn open_url(url: &str) -> std::io::Result<()> {
    let mut cmd = if cfg!(target_os = "macos") {
        Command::new("open")
    } else if cfg!(target_os = "windows") {
        let mut c = Command::new("cmd");
        c.args(["/C", "start", ""]);
        c
    } else {
        Command::new("xdg-open")
    };
    cmd.arg(url).status().map(|_| ())
}

fn print_topics() {
    eprintln!("Available topics:");
    for (file, _) in TOPICS {
        eprintln!("  {}", slug(file));
    }
}

pub fn run_docs(topic: Option<&str>) {
    let url = match topic {
        None => format!("{DOCS_BASE_URL}/README.md"),
        Some(t) => match find_topic(t) {
            Ok(file) => format!("{DOCS_BASE_URL}/{file}"),
            Err(candidates) if candidates.len() > 1 => {
                eprintln!(
                    "Topic '{t}' is ambiguous, did you mean: {}",
                    candidates.iter().map(|f| slug(f)).collect::<Vec<_>>().join(", ")
                );
                process::exit(1);
            }
            Err(_) => {
                eprintln!("Unknown docs topic '{t}'");
                print_topics();
                process::exit(1);
            }
        },
    };

    println!("Opening {url}");
    if let Err(e) = open_url(&url) {
        eprintln!("Could not open browser: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_topic_by_prefix() {
        assert_eq!(find_topic("env"), Ok("12-environments.md"));
        assert_eq!(find_topic("segment"), Ok("11-segments.md"));
        assert_eq!(find_topic("segments"), Ok("11-segments.md"));
        assert_eq!(find_topic("Tests"), Ok("14-tests.md"));
        assert_eq!(find_topic("string"), Ok("06-string-matching.md"));
    }

    #[test]
    fn test_find_topic_by_alias_and_number() {
        assert_eq!(find_topic("regex"), Ok("06-string-matching.md"));
        assert_eq!(find_topic("in"), Ok("07-arrays-membership.md"));
        assert_eq!(find_topic("12"), Ok("12-environments.md"));
    }

    #[test]
    fn test_find_topic_unknown_or_ambiguous() {
        assert_eq!(find_topic("nope"), Err(vec![]));
        assert_eq!(find_topic("99"), Err(vec![]));
        assert!(find_topic("r").unwrap_err().len() > 1);
    }
}
