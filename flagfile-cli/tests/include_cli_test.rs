use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/include")
        .join(rel)
}

fn ff(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ff"))
        .args(args)
        .output()
        .expect("failed to run ff binary")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}

// ── validate ───────────────────────────────────────────────

#[test]
fn test_validate_passes_with_includes() {
    let flagfile = fixture("withtests/Flagfile");
    let out = ff(&["validate", "-f", &flagfile.display().to_string()]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let output = stdout(&out);
    assert!(output.contains("FF-root-feature"));
    assert!(output.contains("FF-included-feature"));
}

#[test]
fn test_validate_fails_on_missing_include() {
    let flagfile = fixture("missing/Flagfile");
    let out = ff(&["validate", "-f", &flagfile.display().to_string()]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("nope.ff"), "stderr: {}", stderr(&out));
}

// ── lint ───────────────────────────────────────────────────

#[test]
fn test_lint_passes_with_includes() {
    let flagfile = fixture("withtests/Flagfile");
    let out = ff(&["lint", "-f", &flagfile.display().to_string()]);
    assert!(
        out.status.success(),
        "stdout: {} stderr: {}",
        stdout(&out),
        stderr(&out)
    );
}

#[test]
fn test_lint_fails_on_missing_include() {
    let flagfile = fixture("missing/Flagfile");
    let out = ff(&["lint", "-f", &flagfile.display().to_string()]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("nope.ff"), "stderr: {}", stderr(&out));
}

// ── test ───────────────────────────────────────────────────

#[test]
fn test_test_runs_included_tests_files() {
    let flagfile = fixture("withtests/Flagfile");
    let testfile = fixture("withtests/Flagfile.tests");
    let out = ff(&[
        "test",
        "-f",
        &flagfile.display().to_string(),
        "-t",
        &testfile.display().to_string(),
    ]);
    let output = stdout(&out);
    assert!(
        out.status.success(),
        "stdout: {} stderr: {}",
        output,
        stderr(&out)
    );
    // root test file ran
    assert!(output.contains("FF-root-feature == true"));
    // included file's sibling tests file was discovered and ran
    assert!(output.contains("sub/Flagfile.tests"), "stdout: {output}");
    assert!(output.contains("FF-included-feature(countryCode=nl) == true"));
    assert!(output.contains("FF-included-feature(countryCode=DE) == false"));
    // included file's inline @test annotation ran, attributed to its own file
    assert!(
        output.contains("inline @test") && output.contains("sub/Flagfile)"),
        "stdout: {output}"
    );
}

#[test]
fn test_test_fails_on_failing_included_assertion() {
    let flagfile = fixture("withtests_fail/Flagfile");
    let testfile = fixture("withtests_fail/Flagfile.tests");
    let out = ff(&[
        "test",
        "-f",
        &flagfile.display().to_string(),
        "-t",
        &testfile.display().to_string(),
    ]);
    assert!(!out.status.success(), "stdout: {}", stdout(&out));
    assert!(stdout(&out).contains("FAIL"), "stdout: {}", stdout(&out));
}

#[test]
fn test_test_fails_on_missing_include() {
    let flagfile = fixture("missing/Flagfile");
    let out = ff(&["test", "-f", &flagfile.display().to_string()]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("nope.ff"), "stderr: {}", stderr(&out));
}

#[test]
fn test_test_fails_on_escaping_include() {
    let flagfile = fixture("escape/dir/Flagfile");
    let out = ff(&["test", "-f", &flagfile.display().to_string()]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains(".."), "stderr: {}", stderr(&out));
}

// ── check ──────────────────────────────────────────────────

#[test]
fn test_check_passes_with_includes() {
    let flagfile = fixture("withtests/Flagfile");
    let testfile = fixture("withtests/Flagfile.tests");
    let out = ff(&[
        "check",
        "-f",
        &flagfile.display().to_string(),
        "-t",
        &testfile.display().to_string(),
    ]);
    assert!(
        out.status.success(),
        "stdout: {} stderr: {}",
        stdout(&out),
        stderr(&out)
    );
}

#[test]
fn test_check_fails_on_missing_include() {
    let flagfile = fixture("missing/Flagfile");
    let out = ff(&["check", "-f", &flagfile.display().to_string()]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("nope.ff"), "stderr: {}", stderr(&out));
}

// ── eval / list ────────────────────────────────────────────

#[test]
fn test_eval_works_with_includes() {
    let flagfile = fixture("withtests/Flagfile");
    let out = ff(&[
        "eval",
        "-f",
        &flagfile.display().to_string(),
        "FF-included-feature",
        "countryCode=nl",
    ]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(
        stdout(&out).to_lowercase().contains("true"),
        "stdout: {}",
        stdout(&out)
    );
}

#[test]
fn test_list_shows_included_flags() {
    let flagfile = fixture("withtests/Flagfile");
    let out = ff(&["list", "-f", &flagfile.display().to_string()]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    let output = stdout(&out);
    assert!(output.contains("FF-root-feature"));
    assert!(output.contains("FF-included-feature"));
}

// ── fmt ────────────────────────────────────────────────────

#[test]
fn test_fmt_check_passes_with_formatted_includes() {
    let flagfile = fixture("fmt_clean/Flagfile");
    let out = ff(&["fmt", "--check", "-f", &flagfile.display().to_string()]);
    assert!(
        out.status.success(),
        "stdout: {} stderr: {}",
        stdout(&out),
        stderr(&out)
    );
}

#[test]
fn test_fmt_check_reports_unformatted_included_file() {
    let flagfile = fixture("fmt_unformatted/Flagfile");
    let out = ff(&["fmt", "--check", "-f", &flagfile.display().to_string()]);
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(err.contains("would reformat"), "stderr: {err}");
    assert!(err.contains("sub/Flagfile"), "stderr: {err}");
}

#[test]
fn test_fmt_fails_on_missing_include() {
    let flagfile = fixture("missing/Flagfile");
    let out = ff(&["fmt", "--check", "-f", &flagfile.display().to_string()]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("nope.ff"), "stderr: {}", stderr(&out));
}

#[test]
fn test_fmt_formats_included_files_in_place() {
    // build a scratch tree so the committed fixtures stay untouched
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("fmt_include_write");
    let sub = dir.join("cua");
    std::fs::create_dir_all(&sub).unwrap();
    let root = dir.join("Flagfile");
    std::fs::write(&root, "FF-root -> true\n\n@include cua/Flagfile\n").unwrap();
    let inc = sub.join("Flagfile");
    std::fs::write(
        &inc,
        "FF-included {\n  plan == premium ->   true\n  false\n}\n",
    )
    .unwrap();

    let out = ff(&["fmt", "-f", &root.display().to_string()]);
    assert!(out.status.success(), "stderr: {}", stderr(&out));
    assert!(
        stdout(&out).contains("formatted"),
        "stdout: {}",
        stdout(&out)
    );

    let root_after = std::fs::read_to_string(&root).unwrap();
    assert!(
        root_after.contains("@include cua/Flagfile"),
        "include line must be preserved, not inlined: {root_after}"
    );
    let inc_after = std::fs::read_to_string(&inc).unwrap();
    assert!(
        inc_after.contains("    plan == premium -> TRUE"),
        "included file should be reformatted in place: {inc_after}"
    );
}
