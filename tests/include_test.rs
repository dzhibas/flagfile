use std::collections::HashMap;
use std::path::{Path, PathBuf};

use flagfile_lib::ast::Atom;
use flagfile_lib::eval::Context;
use flagfile_lib::include::resolve_includes_from_path;
use flagfile_lib::parse_flagfile::parse_flagfile_with_segments;
use flagfile_lib::{ff, init_from_str, FlagReturn};

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/include")
        .join(rel)
}

fn flag_names<'a>(parsed: &'a flagfile_lib::ParsedFlagfile) -> Vec<&'a str> {
    parsed
        .flags
        .iter()
        .flat_map(|fv| fv.keys().copied())
        .collect()
}

// ── Merging ────────────────────────────────────────────────

#[test]
fn test_basic_include_merges_flags() {
    let resolved = resolve_includes_from_path(&fixture("basic/Flagfile")).unwrap();

    assert_eq!(resolved.includes.len(), 1);
    assert!(resolved.includes[0].path.ends_with("Flagfile.demo"));

    let (rest, parsed) = parse_flagfile_with_segments(&resolved.content).unwrap();
    assert_eq!(rest.trim(), "");
    let names = flag_names(&parsed);
    assert!(names.contains(&"FF-basic-local"));
    assert!(names.contains(&"FF-basic-included"));
}

#[test]
fn test_nested_includes_resolve_relative_to_own_dir() {
    let resolved = resolve_includes_from_path(&fixture("nested/Flagfile")).unwrap();

    // depth-first inclusion order
    assert_eq!(resolved.includes.len(), 2);
    assert!(resolved.includes[0].path.ends_with("sub/Flagfile"));
    assert!(resolved.includes[1].path.ends_with("sub/deep/more.ff"));

    let (rest, parsed) = parse_flagfile_with_segments(&resolved.content).unwrap();
    assert_eq!(rest.trim(), "");
    let names = flag_names(&parsed);
    assert!(names.contains(&"FF-nested-root"));
    assert!(names.contains(&"FF-nested-sub"));
    assert!(names.contains(&"FF-nested-deep"));
}

#[test]
fn test_include_directives_in_comments_are_ignored() {
    let resolved = resolve_includes_from_path(&fixture("comments/Flagfile")).unwrap();

    assert!(resolved.includes.is_empty());
    let (rest, parsed) = parse_flagfile_with_segments(&resolved.content).unwrap();
    assert_eq!(rest.trim(), "");
    assert!(flag_names(&parsed).contains(&"FF-comments-only"));
}

// ── Errors ─────────────────────────────────────────────────

#[test]
fn test_missing_include_errors() {
    let err = resolve_includes_from_path(&fixture("missing/Flagfile")).unwrap_err();
    assert!(
        err.contains("nope.ff"),
        "error should name missing file: {err}"
    );
    assert!(
        err.contains("Flagfile"),
        "error should name including file: {err}"
    );
}

#[test]
fn test_parent_dir_include_rejected() {
    // ../secret.ff exists on disk but must still be rejected
    let err = resolve_includes_from_path(&fixture("escape/dir/Flagfile")).unwrap_err();
    assert!(err.contains(".."), "error should mention '..': {err}");
}

#[test]
fn test_absolute_include_rejected() {
    let err = resolve_includes_from_path(&fixture("abs/Flagfile")).unwrap_err();
    assert!(
        err.contains("absolute"),
        "error should mention absolute paths: {err}"
    );
}

#[test]
fn test_include_cycle_detected() {
    let err = resolve_includes_from_path(&fixture("cycle/Flagfile")).unwrap_err();
    assert!(err.contains("cycle"), "error should mention cycle: {err}");
}

#[test]
fn test_self_include_detected() {
    let err = resolve_includes_from_path(&fixture("selfcycle/Flagfile")).unwrap_err();
    assert!(err.contains("cycle"), "error should mention cycle: {err}");
}

// ── Evaluation & tests discovery ───────────────────────────

// The only test in this binary that touches the global FLAGS state.
#[test]
fn test_included_flags_evaluate() {
    let resolved = resolve_includes_from_path(&fixture("withtests/Flagfile")).unwrap();
    init_from_str(&resolved.content);

    let ctx: Context = HashMap::from([("countryCode", Atom::String("nl".to_string()))]);
    assert!(matches!(
        ff("FF-included-feature", &ctx),
        Some(FlagReturn::OnOff(true))
    ));
    assert!(matches!(
        ff("FF-root-feature", &Context::new()),
        Some(FlagReturn::OnOff(true))
    ));
}

#[test]
fn test_includes_expose_paths_for_tests_discovery() {
    let resolved = resolve_includes_from_path(&fixture("withtests/Flagfile")).unwrap();

    assert_eq!(resolved.includes.len(), 1);
    let tests_path = PathBuf::from(format!("{}.tests", resolved.includes[0].path.display()));
    assert!(
        tests_path.exists(),
        "sibling tests file should be discoverable at {}",
        tests_path.display()
    );
    // raw content is preserved per included file (for inline @test extraction)
    assert!(resolved.includes[0].content.contains("@test"));
}
