//! Resolution of `@include <relative-path>` directives.
//!
//! An `@include` line is replaced by the referenced file's content before
//! parsing, producing a single merged Flagfile. Includes resolve relative to
//! the including file's directory and are sandboxed to it: absolute paths and
//! `..` components are rejected, and the resolved file must stay under the
//! root Flagfile's directory (symlinks included).

use std::fs;
use std::path::{Component, Path, PathBuf};

/// A single file pulled in via `@include`, in depth-first inclusion order.
#[derive(Debug, Clone)]
pub struct IncludedFile {
    /// Path as joined from the including file's directory (usable for
    /// display and for locating a sibling `<path>.tests` file).
    pub path: PathBuf,
    /// Raw file content before include expansion.
    pub content: String,
}

/// Result of expanding all `@include` directives.
#[derive(Debug, Clone)]
pub struct ResolvedFlagfile {
    /// Fully merged content, ready for the Flagfile parser.
    pub content: String,
    /// All included files in depth-first inclusion order.
    pub includes: Vec<IncludedFile>,
}

/// Expands `@include` directives in `content`, resolving paths relative to
/// `base_dir` (which is also the sandbox root).
pub fn resolve_includes(content: &str, base_dir: &Path) -> Result<ResolvedFlagfile, String> {
    if !has_include_directive(content) {
        return Ok(ResolvedFlagfile {
            content: content.to_string(),
            includes: Vec::new(),
        });
    }
    let root = fs::canonicalize(base_dir)
        .map_err(|_| format!("could not resolve directory '{}'", base_dir.display()))?;
    let mut includes = Vec::new();
    let mut out = String::new();
    let mut stack: Vec<PathBuf> = Vec::new();
    expand(
        content,
        base_dir,
        &root,
        &base_dir.display().to_string(),
        &mut stack,
        &mut includes,
        &mut out,
    )?;
    Ok(ResolvedFlagfile {
        content: out,
        includes,
    })
}

/// Reads `path` and expands its `@include` directives relative to its
/// parent directory.
pub fn resolve_includes_from_path(path: &Path) -> Result<ResolvedFlagfile, String> {
    let content =
        fs::read_to_string(path).map_err(|_| format!("could not read '{}'", path.display()))?;
    if !has_include_directive(&content) {
        return Ok(ResolvedFlagfile {
            content,
            includes: Vec::new(),
        });
    }
    let dir = parent_dir(path);
    let root = fs::canonicalize(&dir)
        .map_err(|_| format!("could not resolve directory '{}'", dir.display()))?;
    let canonical_self =
        fs::canonicalize(path).map_err(|_| format!("could not resolve '{}'", path.display()))?;
    let mut includes = Vec::new();
    let mut out = String::new();
    let mut stack = vec![canonical_self];
    expand(
        &content,
        &dir,
        &root,
        &path.display().to_string(),
        &mut stack,
        &mut includes,
        &mut out,
    )?;
    Ok(ResolvedFlagfile {
        content: out,
        includes,
    })
}

/// Quick scan so content without includes passes through untouched.
fn has_include_directive(content: &str) -> bool {
    content
        .lines()
        .any(|line| line.trim_start().starts_with("@include"))
}

fn parent_dir(path: &Path) -> PathBuf {
    match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

/// Recursively expands `content` into `out`, tracking comment state so
/// `@include` inside `//` and `/* */` comments is left verbatim.
fn expand(
    content: &str,
    dir: &Path,
    root: &Path,
    includer: &str,
    stack: &mut Vec<PathBuf>,
    includes: &mut Vec<IncludedFile>,
    out: &mut String,
) -> Result<(), String> {
    let mut in_block_comment = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if in_block_comment {
            out.push_str(line);
            out.push('\n');
            if trimmed.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }
        if trimmed.starts_with("/*") && !trimmed.contains("*/") {
            in_block_comment = true;
        }
        let include_path = if trimmed.starts_with("//") || trimmed.starts_with("/*") {
            None
        } else {
            parse_include_line(line)
        };
        let Some(raw_path) = include_path else {
            out.push_str(line);
            out.push('\n');
            continue;
        };

        validate_include_path(raw_path)?;
        let joined = dir.join(raw_path);
        if !joined.is_file() {
            return Err(format!(
                "included file '{}' not found (included from '{}')",
                raw_path, includer
            ));
        }
        let canonical = fs::canonicalize(&joined)
            .map_err(|_| format!("could not resolve include '{}'", joined.display()))?;
        if !canonical.starts_with(root) {
            return Err(format!(
                "include path '{}' escapes the flagfile directory '{}'",
                raw_path,
                root.display()
            ));
        }
        if stack.contains(&canonical) {
            return Err(format!(
                "include cycle detected: '{}' is already being included",
                joined.display()
            ));
        }
        let file_content = fs::read_to_string(&joined)
            .map_err(|_| format!("could not read included file '{}'", joined.display()))?;
        includes.push(IncludedFile {
            path: joined.clone(),
            content: file_content.clone(),
        });
        stack.push(canonical);
        expand(
            &file_content,
            &parent_dir(&joined),
            root,
            &joined.display().to_string(),
            stack,
            includes,
            out,
        )?;
        stack.pop();
    }
    Ok(())
}

/// Returns the include path if the line is an `@include` directive.
fn parse_include_line(line: &str) -> Option<&str> {
    let rest = line.trim().strip_prefix("@include")?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let path = rest.trim();
    if path.is_empty() {
        return None;
    }
    let unquoted = path
        .strip_prefix('"')
        .and_then(|p| p.strip_suffix('"'))
        .unwrap_or(path);
    Some(unquoted)
}

/// Rejects empty, absolute, and `..`-containing include paths.
fn validate_include_path(raw: &str) -> Result<(), String> {
    if raw.is_empty() {
        return Err("include path is empty".to_string());
    }
    let path = Path::new(raw);
    if path.is_absolute() {
        return Err(format!(
            "include path '{}' is not allowed: absolute paths are forbidden",
            raw
        ));
    }
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(format!(
            "include path '{}' is not allowed: '..' components are forbidden",
            raw
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_include_line ─────────────────────────────────

    #[test]
    fn test_parse_include_line_basic() {
        assert_eq!(
            parse_include_line("@include Flagfile.demo"),
            Some("Flagfile.demo")
        );
    }

    #[test]
    fn test_parse_include_line_subdir() {
        assert_eq!(
            parse_include_line("@include cua/Flagfile"),
            Some("cua/Flagfile")
        );
    }

    #[test]
    fn test_parse_include_line_indented_and_quoted() {
        assert_eq!(
            parse_include_line("  @include \"my flags.ff\"  "),
            Some("my flags.ff")
        );
    }

    #[test]
    fn test_parse_include_line_rejects_non_directives() {
        assert_eq!(parse_include_line("// @include nope.ff"), None);
        assert_eq!(parse_include_line("FF-flag -> true"), None);
        assert_eq!(parse_include_line("@includes nope.ff"), None);
        assert_eq!(parse_include_line("@include"), None);
    }

    // ── validate_include_path ──────────────────────────────

    #[test]
    fn test_validate_include_path_accepts_relative() {
        assert!(validate_include_path("Flagfile.demo").is_ok());
        assert!(validate_include_path("cua/Flagfile").is_ok());
    }

    #[test]
    fn test_validate_include_path_rejects_parent_dir() {
        let err = validate_include_path("../secret.ff").unwrap_err();
        assert!(err.contains(".."));
        let err = validate_include_path("cua/../../secret.ff").unwrap_err();
        assert!(err.contains(".."));
    }

    #[test]
    fn test_validate_include_path_rejects_absolute() {
        let err = validate_include_path("/etc/hosts").unwrap_err();
        assert!(err.contains("absolute"));
    }

    #[test]
    fn test_validate_include_path_rejects_empty() {
        assert!(validate_include_path("").is_err());
    }
}
