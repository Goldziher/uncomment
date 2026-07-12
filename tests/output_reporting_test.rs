use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use tempfile::TempDir;

fn uncomment_binary() -> PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("uncomment")
}

fn run(file: &std::path::Path, args: &[&str]) -> Output {
    let mut all_args: Vec<&str> = vec![file.to_str().unwrap()];
    all_args.extend_from_slice(args);
    Command::new(uncomment_binary()).args(&all_args).output().unwrap()
}

#[test]
fn reports_removed_line_ranges_in_stdout() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("sample.js");
    fs::write(&file, "// first\nconst x = 1; // second\n").unwrap();

    let output = run(&file, &["--dry-run"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "run failed: {stdout}");
    assert!(stdout.contains("sample.js"), "expected file path, got: {stdout}");
    assert!(stdout.contains("would remove 2"), "expected count, got: {stdout}");
    assert!(stdout.contains("L1"), "expected L1, got: {stdout}");
    assert!(stdout.contains("L2"), "expected L2, got: {stdout}");
}

#[test]
fn shows_preservation_hint_on_stderr_when_comments_removed() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("sample.js");
    fs::write(&file, "// strip me\nconst x = 1;\n").unwrap();

    let output = run(&file, &["--dry-run"]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.contains("~keep"), "expected preservation hint, got: {stderr}");
    assert!(stderr.contains("--ignore"), "expected --ignore guidance, got: {stderr}");
}

#[test]
fn no_hint_when_nothing_is_removed() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("sample.js");
    // A lone TODO is preserved by default, so nothing is removed.
    fs::write(&file, "// TODO: keep me\nconst x = 1;\n").unwrap();

    let output = run(&file, &["--dry-run"]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!stderr.contains("~keep"), "hint should be absent, got: {stderr}");
}

#[test]
fn quiet_suppresses_per_file_output_but_still_writes_and_summarizes() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("sample.js");
    fs::write(&file, "// strip me\nconst x = 1;\n").unwrap();

    let output = run(&file, &["--quiet"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "run failed: {stderr}");
    assert!(
        !stdout.contains("Modified:"),
        "quiet should hide per-file lines, got: {stdout}"
    );
    assert!(
        stdout.contains("files processed"),
        "summary should remain, got: {stdout}"
    );
    assert!(
        !stderr.contains("~keep"),
        "quiet should suppress the hint, got: {stderr}"
    );

    let contents = fs::read_to_string(&file).unwrap();
    assert!(
        !contents.contains("strip me"),
        "file should be modified on disk, got: {contents}"
    );
}

#[test]
fn diff_does_not_misalign_unchanged_lines() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("sample.rs");
    fs::write(
        &file,
        "// strip this leading comment\nfn main() {\n    let a = 1;\n    let b = 2;\n    let c = 3;\n    let keep = 42;\n    let d = 4;\n}\n",
    )
    .unwrap();

    let output = run(&file, &["--dry-run", "--diff"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        stdout.contains("-// strip this leading comment"),
        "the comment should be shown removed, got: {stdout}"
    );
    assert!(
        !stdout.contains("-    let a = 1;"),
        "unchanged code must not be marked removed (regression: old index-based diff), got: {stdout}"
    );
    assert!(
        !stdout.contains("let keep"),
        "a line outside the context window should be hidden, got: {stdout}"
    );
}

#[test]
fn quiet_suppresses_important_removal_warning() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("sample.js");
    fs::write(&file, "// eslint-disable-next-line\nconst x = 1;\n").unwrap();

    let loud = run(&file, &["--dry-run", "--no-default-ignores"]);
    assert!(
        String::from_utf8_lossy(&loud.stderr).contains("potentially important"),
        "expected the warning without --quiet"
    );

    fs::write(&file, "// eslint-disable-next-line\nconst x = 1;\n").unwrap();
    let quiet = run(&file, &["--dry-run", "--no-default-ignores", "--quiet"]);
    assert!(
        !String::from_utf8_lossy(&quiet.stderr).contains("potentially important"),
        "quiet should suppress the important-removal warning, got: {}",
        String::from_utf8_lossy(&quiet.stderr)
    );
}

#[test]
fn diff_handles_file_without_trailing_newline() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("sample.rs");
    fs::write(&file, "fn main() {} // strip").unwrap();

    let output = run(&file, &["--dry-run", "--diff"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "run failed: {stdout}");
    assert!(
        stdout.contains("-fn main() {} // strip"),
        "expected the original trailing-comment line, got: {stdout}"
    );
    assert!(
        stdout.contains("+fn main() {}"),
        "expected the processed line, got: {stdout}"
    );
}

#[test]
fn diff_works_without_dry_run() {
    let dir = TempDir::new().unwrap();
    let file = dir.path().join("sample.rs");
    fs::write(&file, "// strip me\nfn main() {}\n").unwrap();

    let output = run(&file, &["--diff"]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "run failed");
    assert!(stdout.contains("(processed)"), "expected a diff header, got: {stdout}");
    assert!(stdout.contains("-// strip me"), "expected removed line, got: {stdout}");

    let contents = fs::read_to_string(&file).unwrap();
    assert!(
        !contents.contains("strip me"),
        "file should be modified on disk, got: {contents}"
    );
}
