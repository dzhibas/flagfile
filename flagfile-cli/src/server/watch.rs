use super::state::ParsedFlags;

pub fn parse_flags(content: &str) -> Option<ParsedFlags> {
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
