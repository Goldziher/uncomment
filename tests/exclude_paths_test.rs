//! Path exclusion — `[global] exclude` and `--exclude` — driven through the compiled binary, so each
//! of the four subcommands' own file collector is exercised rather than a shared helper standing in
//! for all of them.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_uncomment"))
}

/// One removable comment and one keyless TODO, so a default run, `scan`, `keep` and `lint` all have
/// something to say about every file in the fixture.
const SOURCE: &str = "# a removable comment\ndef f():\n    return 1  # TODO: no key\n";

/// Both live under `playground/`, the second one a level deeper, so a `playground/**` rule has to
/// cover a whole subtree and not just its immediate children.
const EXCLUDED: [&str; 2] = ["playground/skip.py", "playground/deep/inner.py"];

const KEPT: &str = "src/keep.py";

const CONFIG: &str = "[global]\nexclude = [\"playground/**\"]\n\n[lint]\nenabled = true\n";

fn write(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(&path, content).expect("write fixture file");
}

fn read(root: &Path, relative: &str) -> String {
    fs::read_to_string(root.join(relative)).expect("read fixture file")
}

fn fixture(config: Option<&str>) -> TempDir {
    let temp = TempDir::new().expect("temp dir");
    let root = temp.path();
    // A real `HEAD`, so lint's "is this the current branch's issue?" lookup has a branch to read.
    write(root, ".git/HEAD", "ref: refs/heads/work\n");
    if let Some(config) = config {
        write(root, ".uncomment.toml", config);
    }
    write(root, KEPT, SOURCE);
    for relative in EXCLUDED {
        write(root, relative, SOURCE);
    }
    temp
}

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(binary())
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run uncomment")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn assert_untouched(root: &Path) {
    for relative in EXCLUDED {
        assert_eq!(read(root, relative), SOURCE, "{relative} must not be rewritten");
    }
}

#[test]
fn the_default_run_never_rewrites_an_excluded_path() {
    let temp = fixture(Some(CONFIG));
    let root = temp.path();

    let output = run(root, &["."]);
    assert!(output.status.success(), "run failed: {}", stderr(&output));

    assert_ne!(read(root, KEPT), SOURCE, "{KEPT} should have been rewritten");
    assert_untouched(root);
}

#[test]
fn scan_leaves_an_excluded_path_out_of_the_inventory() {
    let temp = fixture(Some(CONFIG));
    let root = temp.path();

    let output = run(root, &["scan", "."]);
    assert!(output.status.success(), "scan failed: {}", stderr(&output));

    let report = stdout(&output);
    assert!(report.contains("keep.py"), "scan should report {KEPT}: {report}");
    assert!(
        !report.contains("playground"),
        "scan reported an excluded path: {report}"
    );
}

#[test]
fn lint_does_not_inspect_an_excluded_path() {
    let temp = fixture(Some(CONFIG));
    let root = temp.path();

    let output = run(root, &["lint", "."]);
    let report = stdout(&output);
    assert!(report.contains("keep.py"), "lint should report {KEPT}: {report}");
    assert!(
        !report.contains("playground"),
        "lint reported an excluded path: {report}"
    );
}

#[test]
fn keep_does_not_mark_an_excluded_path() {
    let temp = fixture(Some(CONFIG));
    let root = temp.path();

    let output = run(root, &["keep", ".", "--all-removable"]);
    assert!(output.status.success(), "keep failed: {}", stderr(&output));

    assert!(read(root, KEPT).contains("~keep"), "{KEPT} should have been marked");
    assert_untouched(root);
}

#[test]
fn the_exclude_flag_works_without_any_config_file() {
    let temp = fixture(None);
    let root = temp.path();

    let output = run(root, &[".", "--exclude", "playground/**"]);
    assert!(output.status.success(), "run failed: {}", stderr(&output));

    assert_ne!(read(root, KEPT), SOURCE, "{KEPT} should have been rewritten");
    assert_untouched(root);
}

#[test]
fn the_exclude_flag_adds_to_the_configured_list() {
    let temp = fixture(Some(CONFIG));
    let root = temp.path();

    let output = run(root, &[".", "--exclude", "src/**"]);
    assert!(output.status.success(), "run failed: {}", stderr(&output));

    assert_eq!(
        read(root, KEPT),
        SOURCE,
        "--exclude must apply on top of the configured globs, not replace them"
    );
    assert_untouched(root);
}

/// A caller naming the file outright must get the same answer whichever way the path is spelled,
/// which is only true if the candidate is normalized before it is matched.
#[test]
fn an_excluded_file_named_outright_is_skipped_however_its_path_is_spelled() {
    for spelling in ["playground/skip.py", "./playground/skip.py"] {
        let temp = fixture(Some(CONFIG));
        let root = temp.path();

        let output = run(root, &[spelling]);
        assert!(output.status.success(), "run failed: {}", stderr(&output));
        assert_eq!(
            read(root, "playground/skip.py"),
            SOURCE,
            "naming the file as {spelling} bypassed the exclusion"
        );
    }
}

/// `--no-gitignore` takes every collector down its `glob::glob` branch instead of the walker, and
/// the exclusion has to hold there too.
#[test]
fn exclusion_holds_when_collection_goes_through_a_glob() {
    let temp = fixture(Some(CONFIG));
    let root = temp.path();

    let output = run(root, &["*/*.py", "--no-gitignore"]);
    assert!(output.status.success(), "run failed: {}", stderr(&output));

    assert_ne!(read(root, KEPT), SOURCE, "{KEPT} should have been rewritten");
    assert_eq!(
        read(root, "playground/skip.py"),
        SOURCE,
        "the glob branch bypassed the exclusion"
    );
}

#[test]
fn an_unparseable_exclude_glob_in_the_config_fails_the_run_and_names_the_key() {
    let temp = fixture(Some("[global]\nexclude = [\"playground/[\"]\n"));
    let root = temp.path();

    let output = run(root, &["."]);
    assert!(!output.status.success(), "a broken exclude glob must fail the run");
    let message = stderr(&output);
    assert!(
        message.contains("[global] exclude") && message.contains("playground/["),
        "the error should name the setting and the glob: {message}"
    );
    assert_eq!(read(root, KEPT), SOURCE, "nothing should be rewritten after the error");
}

#[test]
fn an_unparseable_exclude_glob_on_the_command_line_fails_the_run() {
    let temp = fixture(None);
    let root = temp.path();

    let output = run(root, &[".", "--exclude", "playground/["]);
    assert!(!output.status.success(), "a broken --exclude glob must fail the run");
    let message = stderr(&output);
    assert!(
        message.contains("--exclude") && message.contains("playground/["),
        "the error should name the flag and the glob: {message}"
    );
}
