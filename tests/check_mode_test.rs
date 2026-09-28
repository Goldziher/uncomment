//! End-to-end tests for `uncomment --check`, driven through the compiled binary.
//!
//! `--check` is a gate: its whole contract is the exit code, the lines it prints and the files it
//! leaves alone, so every test runs the real binary in a fixture repository and asserts on those.
//! Git runs with the machine's global and system config switched off, so a signing or hook setting
//! on the machine running the tests cannot fail a fixture commit.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

const BINARY: &str = env!("CARGO_BIN_EXE_uncomment");

const EXIT_CLEAN: i32 = 0;
const EXIT_REMOVABLE: i32 = 1;
const EXIT_ERROR: i32 = 2;

#[cfg(windows)]
const NULL_DEVICE: &str = "NUL";
#[cfg(not(windows))]
const NULL_DEVICE: &str = "/dev/null";

const REMOVABLE_RS: &str = "fn main() {\n    // strip me\n    let x = 1; // trailing note\n    println!(\"{x}\");\n}\n";

const PRESERVED_RS: &str = "\
/// Documented, so preserved.
fn main() {
    // TODO: preserved by default
    // this explains something ~keep
    #[allow(dead_code)] // clippy::needless_return is intended
    let x = 1; // eslint-disable-line
}
";

struct Fixture {
    dir: TempDir,
}

impl Fixture {
    fn new() -> Self {
        let fixture = Self {
            dir: TempDir::new().expect("temp dir"),
        };
        fixture.write(".git/HEAD", "ref: refs/heads/main\n");
        fixture
    }

    /// A fixture backed by a real repository, for the flags that shell out to `git diff`.
    fn with_repository() -> Self {
        let fixture = Self {
            dir: TempDir::new().expect("temp dir"),
        };
        fixture.git(&["init", "-q", "-b", "main"]);
        fixture
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn write(&self, relative: &str, content: &str) -> PathBuf {
        let path = self.root().join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(&path, content).expect("write fixture file");
        path
    }

    fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.root().join(relative)).expect("read fixture file")
    }

    fn command(&self, program: &str) -> Command {
        let mut command = Command::new(program);
        command
            .current_dir(self.root())
            .env("NO_COLOR", "1")
            .env("GIT_CONFIG_GLOBAL", NULL_DEVICE)
            .env("GIT_CONFIG_NOSYSTEM", "1");
        command
    }

    fn run(&self, argv: &[&str]) -> Output {
        self.command(BINARY).args(argv).output().expect("run uncomment")
    }

    fn git(&self, argv: &[&str]) {
        let output = self
            .command("git")
            .args([
                "-c",
                "user.email=check@example.com",
                "-c",
                "user.name=Check Fixture",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(argv)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {argv:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn commit_all(&self, message: &str) {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-q", "-m", message]);
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The `path:line:col: text` lines, without the summary.
fn finding_lines(output: &Output) -> Vec<String> {
    stdout(output)
        .lines()
        .filter(|line| !line.starts_with('✓') && !line.starts_with('✗') && !line.is_empty())
        .map(str::to_string)
        .collect()
}

fn code(output: &Output) -> i32 {
    output.status.code().expect("exited normally, not by signal")
}

// --- exit codes and output --------------------------------------------------------------------

#[test]
fn removable_comments_fail_the_check_and_are_listed_as_path_line_col() {
    let fixture = Fixture::new();
    fixture.write("src/main.rs", REMOVABLE_RS);

    let output = fixture.run(&["--check", "src"]);

    assert_eq!(code(&output), EXIT_REMOVABLE, "stderr: {}", stderr(&output));
    assert_eq!(
        finding_lines(&output),
        vec![
            "src/main.rs:2:5: // strip me".to_string(),
            "src/main.rs:3:16: // trailing note".to_string(),
        ]
    );
    let summary = stdout(&output);
    assert!(
        summary.contains("✗ 2 removable comment(s) in 1 file(s) (1 file(s) checked)"),
        "summary line missing: {summary}"
    );
}

#[test]
fn the_check_never_modifies_a_file() {
    let fixture = Fixture::new();
    fixture.write("src/main.rs", REMOVABLE_RS);

    let output = fixture.run(&["--check", "src/main.rs"]);

    assert_eq!(code(&output), EXIT_REMOVABLE);
    assert_eq!(fixture.read("src/main.rs"), REMOVABLE_RS);
}

#[test]
fn only_preserved_comments_pass_the_check() {
    let fixture = Fixture::new();
    fixture.write("src/main.rs", PRESERVED_RS);
    fixture.write("tool.py", "import os  # noqa: F401\n# ~keep why this import exists\n");

    let output = fixture.run(&["--check", "src", "tool.py"]);

    assert_eq!(
        code(&output),
        EXIT_CLEAN,
        "preserved comments were reported: {}",
        stdout(&output)
    );
    assert_eq!(finding_lines(&output), Vec::<String>::new());
    assert!(
        stdout(&output).contains("✓ no removable comments (2 file(s) checked)"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn a_configured_preserve_pattern_is_honoured() {
    let fixture = Fixture::new();
    fixture.write(".uncomment.toml", "[global]\npreserve_patterns = [\"SAFETY\"]\n");
    fixture.write("src/lib.rs", "fn f() {\n    // SAFETY: the pointer is valid\n}\n");

    let output = fixture.run(&["--check", "src"]);

    assert_eq!(code(&output), EXIT_CLEAN, "{}", stdout(&output));
}

#[test]
fn findings_are_sorted_by_path_then_line() {
    let fixture = Fixture::new();
    fixture.write("b.js", "// b one\nconst b = 1;\n// b two\n");
    fixture.write("a.js", "const a = 1;\n// a two\n");
    fixture.write("sub/c.js", "// c one\n");

    let output = fixture.run(&["--check", "-j", "4", "b.js", "sub", "a.js"]);

    assert_eq!(
        finding_lines(&output),
        vec![
            "a.js:2:1: // a two".to_string(),
            "b.js:1:1: // b one".to_string(),
            "b.js:3:1: // b two".to_string(),
            "sub/c.js:1:1: // c one".to_string(),
        ]
    );
}

#[test]
fn a_long_comment_is_truncated_to_its_first_line() {
    let fixture = Fixture::new();
    let long = "y".repeat(200);
    fixture.write("a.js", &format!("const a = 1;\n/* {long}\n   second line */\n"));

    let output = fixture.run(&["--check", "a.js"]);

    let lines = finding_lines(&output);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].starts_with("a.js:2:1: /* yyy"), "{}", lines[0]);
    assert!(!lines[0].contains("second line"), "{}", lines[0]);
    assert!(lines[0].len() < long.len(), "the excerpt must be capped: {}", lines[0]);
}

#[test]
fn quiet_prints_only_the_summary() {
    let fixture = Fixture::new();
    fixture.write("a.js", "// gone\n");

    let output = fixture.run(&["--check", "--quiet", "a.js"]);

    assert_eq!(code(&output), EXIT_REMOVABLE);
    assert_eq!(finding_lines(&output), Vec::<String>::new());
    assert!(
        stdout(&output).contains("✗ 1 removable comment(s)"),
        "{}",
        stdout(&output)
    );
}

// --- pre-commit usage: explicit file lists ----------------------------------------------------

#[test]
fn explicit_files_skip_excluded_and_unsupported_files_silently() {
    let fixture = Fixture::new();
    fixture.write(".uncomment.toml", "[global]\nexclude = [\"vendor/**\"]\n");
    fixture.write("vendor/dep.js", "// vendored, never checked\n");
    fixture.write("notes.txt", "# not source\n");
    fixture.write("image.bin", "\u{0}\u{1}");
    fixture.write("src/app.js", "const a = 1;\n");

    let output = fixture.run(&["--check", "vendor/dep.js", "notes.txt", "image.bin", "src/app.js"]);

    assert_eq!(code(&output), EXIT_CLEAN, "{}", stdout(&output));
    assert!(
        stdout(&output).contains("(1 file(s) checked)"),
        "only src/app.js is checked: {}",
        stdout(&output)
    );
    assert_eq!(stderr(&output), "", "nothing to say about skipped files");
}

#[test]
fn a_file_list_with_no_supported_file_passes() {
    let fixture = Fixture::new();
    fixture.write("README.txt", "hello\n");

    let output = fixture.run(&["--check", "README.txt"]);

    assert_eq!(code(&output), EXIT_CLEAN);
    assert!(
        stdout(&output).contains("✓ no removable comments (0 file(s) checked)"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn a_file_that_is_not_utf8_is_skipped_with_a_note() {
    let fixture = Fixture::new();
    fs::write(fixture.root().join("latin1.js"), b"// \xff\xfe not text\n").expect("write latin-1 file");
    fixture.write("ok.js", "const a = 1;\n");

    let output = fixture.run(&["--check", "latin1.js", "ok.js"]);

    assert_eq!(code(&output), EXIT_CLEAN, "{}", stderr(&output));
    assert!(stderr(&output).contains("skipped latin1.js"), "{}", stderr(&output));
}

// --- errors ------------------------------------------------------------------------------------

#[test]
fn no_paths_is_a_usage_error() {
    let fixture = Fixture::new();

    let output = fixture.run(&["--check"]);

    assert_eq!(code(&output), EXIT_ERROR, "{}", stderr(&output));
    assert!(stderr(&output).contains("No input paths"), "{}", stderr(&output));
}

#[test]
fn an_unreadable_config_is_an_error_not_a_pass() {
    let fixture = Fixture::new();
    fixture.write("a.js", "const a = 1;\n");

    let output = fixture.run(&["--check", "--config", "missing.toml", "a.js"]);

    assert_eq!(code(&output), EXIT_ERROR, "{}", stderr(&output));
}

#[test]
fn a_rejected_config_is_an_error_not_a_pass() {
    let fixture = Fixture::new();
    fixture.write(".uncomment.toml", "[global]\nno_such_key = true\n");
    fixture.write("a.js", "const a = 1;\n");

    let output = fixture.run(&["--check", "a.js"]);

    assert_eq!(code(&output), EXIT_ERROR, "{}", stderr(&output));
}

#[cfg(unix)]
#[test]
fn a_file_that_cannot_be_read_is_an_error_not_a_pass() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = Fixture::new();
    let locked = fixture.write("locked.js", "// would be removed\n");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("chmod 000");
    if fs::read(&locked).is_ok() {
        eprintln!("skipped: running as a user permissions do not apply to");
        return;
    }

    let output = fixture.run(&["--check", "locked.js"]);

    fs::set_permissions(&locked, fs::Permissions::from_mode(0o644)).expect("restore permissions");
    assert_eq!(code(&output), EXIT_ERROR, "{}", stdout(&output));
    assert!(
        stdout(&output).contains("1 file(s) could not be inspected"),
        "{}",
        stdout(&output)
    );
}

// --- flag interactions --------------------------------------------------------------------------

#[test]
fn check_with_diff_is_rejected() {
    let fixture = Fixture::new();
    fixture.write("a.js", "// gone\n");

    let output = fixture.run(&["--check", "--diff", "a.js"]);

    assert_eq!(code(&output), EXIT_ERROR);
    assert!(
        stderr(&output).contains("'--check' cannot be used with '--diff'"),
        "{}",
        stderr(&output)
    );
    assert_eq!(fixture.read("a.js"), "// gone\n");
}

#[test]
fn check_with_dry_run_is_accepted_and_changes_nothing() {
    let fixture = Fixture::new();
    fixture.write("a.js", "// gone\n");

    let output = fixture.run(&["--check", "--dry-run", "a.js"]);

    assert_eq!(code(&output), EXIT_REMOVABLE);
    assert_eq!(finding_lines(&output), vec!["a.js:1:1: // gone".to_string()]);
    assert_eq!(fixture.read("a.js"), "// gone\n");
}

#[test]
fn format_requires_check() {
    let fixture = Fixture::new();
    fixture.write("a.js", "// gone\n");

    let output = fixture.run(&["--format", "json", "a.js"]);

    assert_eq!(code(&output), EXIT_ERROR);
    assert!(stderr(&output).contains("--check"), "{}", stderr(&output));
    assert_eq!(fixture.read("a.js"), "// gone\n", "a usage error must write nothing");
}

#[test]
fn changed_only_requires_check() {
    let fixture = Fixture::new();
    fixture.write("a.js", "// gone\n");

    let output = fixture.run(&["--changed-only", "a.js"]);

    assert_eq!(code(&output), EXIT_ERROR);
    assert!(stderr(&output).contains("requires --check"), "{}", stderr(&output));
    assert_eq!(fixture.read("a.js"), "// gone\n", "a usage error must write nothing");
}

#[test]
fn without_check_a_dry_run_still_exits_zero() {
    let fixture = Fixture::new();
    fixture.write("a.js", "// gone\n");

    let output = fixture.run(&["--dry-run", "a.js"]);

    assert_eq!(code(&output), EXIT_CLEAN, "--dry-run is a preview, never a gate");
}

// --- JSON ----------------------------------------------------------------------------------------

#[test]
fn json_output_carries_every_finding_and_the_summary() {
    let fixture = Fixture::new();
    fixture.write("src/main.rs", REMOVABLE_RS);
    fixture.write("src/clean.rs", "fn f() {}\n");

    let output = fixture.run(&["--check", "--format", "json", "src"]);

    assert_eq!(code(&output), EXIT_REMOVABLE);
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout is one JSON document");
    let violations = document["violations"].as_array().expect("violations array");
    assert_eq!(violations.len(), 2);
    assert_eq!(violations[0]["path"], "src/main.rs");
    assert_eq!(violations[0]["line"], 2);
    assert_eq!(violations[0]["column"], 5);
    assert_eq!(violations[0]["end_line"], 2);
    assert_eq!(violations[0]["excerpt"], "// strip me");
    assert_eq!(violations[1]["column"], 16);
    assert_eq!(document["summary"]["files_checked"], 2);
    assert_eq!(document["summary"]["files_with_violations"], 1);
    assert_eq!(document["summary"]["violations"], 2);
    assert_eq!(document["summary"]["uninspectable_files"], 0);
    assert!(document["notes"].is_array());
}

#[test]
fn json_output_on_a_clean_tree_is_empty_and_exits_zero() {
    let fixture = Fixture::new();
    fixture.write("src/clean.rs", "fn f() {}\n");

    let output = fixture.run(&["--check", "--format", "json", "src"]);

    assert_eq!(code(&output), EXIT_CLEAN);
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout is one JSON document");
    assert_eq!(document["violations"].as_array().map(Vec::len), Some(0));
    assert_eq!(document["summary"]["violations"], 0);
}

// --- --changed-only -----------------------------------------------------------------------------

/// `main` holds a file with a pre-existing removable comment; `feat` adds a second one.
fn branched_repository() -> Fixture {
    let fixture = Fixture::with_repository();
    fixture.write("src/old.js", "// pre-existing\nconst old = 1;\n");
    fixture.commit_all("base");
    fixture.git(&["checkout", "-q", "-b", "feat"]);
    fixture.write("src/new.js", "// brand new\nconst fresh = 1;\n");
    fixture.commit_all("change");
    fixture
}

#[test]
fn changed_only_checks_only_files_changed_against_the_base() {
    let fixture = branched_repository();

    let everything = fixture.run(&["--check", "src"]);
    assert_eq!(finding_lines(&everything).len(), 2);

    let changed = fixture.run(&["--check", "--changed-only", "--base", "main", "src"]);

    assert_eq!(code(&changed), EXIT_REMOVABLE);
    assert_eq!(
        finding_lines(&changed),
        vec!["src/new.js:1:1: // brand new".to_string()]
    );
    assert!(
        stderr(&changed).contains("--changed-only against main: 1 of 2 file(s) changed"),
        "{}",
        stderr(&changed)
    );
}

#[test]
fn changed_only_passes_when_the_branch_touched_only_clean_files() {
    let fixture = Fixture::with_repository();
    fixture.write("src/old.js", "// pre-existing\nconst old = 1;\n");
    fixture.commit_all("base");
    fixture.git(&["checkout", "-q", "-b", "feat"]);
    fixture.write("src/new.js", "const fresh = 1;\n");
    fixture.commit_all("change");

    let output = fixture.run(&["--check", "--changed-only", "--base", "main", "src"]);

    assert_eq!(code(&output), EXIT_CLEAN, "{}", stdout(&output));
}

#[test]
fn an_unknown_base_ref_is_an_error_not_a_pass() {
    let fixture = branched_repository();

    let output = fixture.run(&["--check", "--changed-only", "--base", "no-such-ref", "src"]);

    assert_eq!(code(&output), EXIT_ERROR, "{}", stderr(&output));
    assert!(stderr(&output).contains("no-such-ref"), "{}", stderr(&output));
}
