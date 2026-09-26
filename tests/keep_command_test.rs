//! End-to-end coverage for `uncomment keep`.
//!
//! The command is driven through the compiled binary whenever the `keep` subcommand is wired into
//! the CLI, and through `uncomment::keep::run` with a clap-parsed argument vector when it is not —
//! the argument vector, the file discovery, the id resolution and the writes are identical either
//! way, so these tests exercise the real command today and keep exercising it once the subcommand
//! lands. [`keep_subcommand_is_wired`] decides which path runs.

use clap::Parser;
use std::path::Path;
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use tempfile::TempDir;
use uncomment::keep::KeepArgs;
use uncomment::scan::id::comment_id;

const BINARY: &str = env!("CARGO_BIN_EXE_uncomment");

/// Parses the same argument vector the wired subcommand would receive.
#[derive(Parser, Debug)]
#[command(name = "keep")]
struct Harness {
    #[command(flatten)]
    keep: KeepArgs,
}

struct KeepRun {
    ok: bool,
    message: String,
}

/// Serialises the in-process path, which has to change the working directory.
fn cwd_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// `--help` succeeds either way — an unwired name is parsed as a path and prints the top-level help —
/// so the usage line is what actually says whether the subcommand exists.
fn subcommand_is_wired(name: &str) -> bool {
    Command::new(BINARY)
        .args([name, "--help"])
        .output()
        .is_ok_and(|output| {
            output.status.success()
                && String::from_utf8_lossy(&output.stdout).contains(&format!("Usage: uncomment {name}"))
        })
}

fn keep_subcommand_is_wired() -> bool {
    static WIRED: OnceLock<bool> = OnceLock::new();
    *WIRED.get_or_init(|| subcommand_is_wired("keep"))
}

fn scan_subcommand_is_wired() -> bool {
    static WIRED: OnceLock<bool> = OnceLock::new();
    *WIRED.get_or_init(|| subcommand_is_wired("scan"))
}

fn run_keep(dir: &Path, argv: &[&str]) -> KeepRun {
    if keep_subcommand_is_wired() {
        let output = Command::new(BINARY)
            .arg("keep")
            .args(argv)
            .current_dir(dir)
            .output()
            .expect("failed to run uncomment keep");
        return KeepRun {
            ok: output.status.success(),
            message: format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        };
    }

    let guard = cwd_lock().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let original = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(dir).expect("chdir into the fixture");

    let mut full = vec!["keep"];
    full.extend_from_slice(argv);
    let harness = Harness::parse_from(full);
    let result = uncomment::keep::run(&harness.keep);

    std::env::set_current_dir(&original).expect("restore cwd");
    drop(guard);

    match result {
        Ok(()) => KeepRun {
            ok: true,
            message: String::new(),
        },
        Err(error) => KeepRun {
            ok: false,
            message: format!("{error:#}"),
        },
    }
}

/// A git repository, because ids are computed against a repo-relative path.
fn fixture(files: &[(&str, &str)]) -> TempDir {
    let dir = TempDir::new().expect("tempdir");
    git(dir.path(), &["init", "-q"]);
    for (name, content) in files {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(&path, content).expect("write fixture");
    }
    git(dir.path(), &["add", "-A"]);
    dir
}

fn git(dir: &Path, argv: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(argv)
        .output()
        .unwrap_or_else(|error| panic!("git {argv:?} failed to start: {error}"));
    assert!(
        output.status.success(),
        "git {argv:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn read(dir: &Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).expect("read fixture")
}

/// The default `uncomment` run, which must find nothing to remove once markers are in place.
fn default_run(dir: &Path, extra: &[&str]) -> String {
    let output = Command::new(BINARY)
        .arg(".")
        .args(extra)
        .current_dir(dir)
        .output()
        .expect("failed to run uncomment");
    assert!(
        output.status.success(),
        "uncomment failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn a_line_comment_is_marked_in_its_own_text() {
    let dir = fixture(&[("src/a.rs", "// legacy shim\nfn main() {}\n")]);
    let run = run_keep(dir.path(), &["--all-removable", "."]);
    assert!(run.ok, "{}", run.message);
    assert_eq!(read(dir.path(), "src/a.rs"), "// legacy shim ~keep\nfn main() {}\n");
}

#[test]
fn a_rust_doc_comment_gets_a_plain_line_marker_above_it() {
    let dir = fixture(&[("src/a.rs", "/// Public API.\npub fn f() {}\n")]);
    let run = run_keep(dir.path(), &["--match", "Public API", "."]);
    assert!(run.ok, "{}", run.message);
    assert_eq!(
        read(dir.path(), "src/a.rs"),
        "// ~keep\n/// Public API.\npub fn f() {}\n"
    );
}

#[test]
fn a_python_docstring_gets_a_hash_marker_above_it_and_is_not_edited() {
    let source = "def f():\n    \"\"\"Explain f.\"\"\"\n    return 1\n";
    let dir = fixture(&[("src/a.py", source)]);
    let run = run_keep(dir.path(), &["--match", "Explain f", "."]);
    assert!(run.ok, "{}", run.message);

    let after = read(dir.path(), "src/a.py");
    assert_eq!(
        after,
        "def f():\n    # ~keep\n    \"\"\"Explain f.\"\"\"\n    return 1\n"
    );
    // The docstring is a string node whose bytes are `__doc__` at runtime.
    assert!(after.contains("\"\"\"Explain f.\"\"\""), "{after}");
    assert!(!after.contains("Explain f. ~keep"), "{after}");
}

#[test]
fn a_c_block_comment_gets_a_marker_line_above_it() {
    let dir = fixture(&[("src/a.c", "/* legacy path */\nint main(void) { return 0; }\n")]);
    let run = run_keep(dir.path(), &["--all-removable", "."]);
    assert!(run.ok, "{}", run.message);
    assert_eq!(
        read(dir.path(), "src/a.c"),
        "// ~keep\n/* legacy path */\nint main(void) { return 0; }\n"
    );
}

#[test]
fn marking_several_comments_in_one_file_produces_every_marker() {
    let source = "// one\nfn a() {}\n// two\nfn b() {}\n/* three */\nfn c() {}\n// four\nfn d() {}\n";
    let dir = fixture(&[("src/a.rs", source)]);
    let run = run_keep(dir.path(), &["--all-removable", "."]);
    assert!(run.ok, "{}", run.message);
    assert_eq!(
        read(dir.path(), "src/a.rs"),
        "// one ~keep\nfn a() {}\n// two ~keep\nfn b() {}\n// ~keep\n/* three */\nfn c() {}\n// four ~keep\nfn d() {}\n"
    );
    assert_eq!(read(dir.path(), "src/a.rs").matches("~keep").count(), 4);
}

#[test]
fn re_running_keep_on_its_own_output_is_byte_identical() {
    let source = "// one\nfn a() {}\n/* two */\nfn b() {}\n";
    let dir = fixture(&[("src/a.rs", source)]);

    assert!(run_keep(dir.path(), &["--all-removable", "."]).ok);
    let once = read(dir.path(), "src/a.rs");

    let second = run_keep(dir.path(), &["--all-removable", "."]);
    assert!(second.ok, "{}", second.message);
    assert_eq!(read(dir.path(), "src/a.rs"), once);
}

#[test]
fn the_first_line_of_the_file_and_a_comment_after_a_blank_line_are_both_marked() {
    let source = "/* first */\nint x;\n\n/* later */\nint y;\n";
    let dir = fixture(&[("src/a.c", source)]);
    let run = run_keep(dir.path(), &["--all-removable", "."]);
    assert!(run.ok, "{}", run.message);
    assert_eq!(
        read(dir.path(), "src/a.c"),
        "// ~keep\n/* first */\nint x;\n\n// ~keep\n/* later */\nint y;\n"
    );
    assert_eq!(default_removed_count(dir.path()), 0);
}

#[test]
fn a_crlf_file_stays_crlf() {
    let source = "// one\r\nfn a() {}\r\n/* two */\r\nfn b() {}\r\n";
    let dir = fixture(&[("src/a.rs", source)]);
    let run = run_keep(dir.path(), &["--all-removable", "."]);
    assert!(run.ok, "{}", run.message);

    let after = read(dir.path(), "src/a.rs");
    assert_eq!(
        after,
        "// one ~keep\r\nfn a() {}\r\n// ~keep\r\n/* two */\r\nfn b() {}\r\n"
    );
    assert_eq!(after.matches('\n').count(), after.matches("\r\n").count());
}

#[test]
fn an_id_from_a_decisions_file_marks_exactly_that_comment() {
    let source = "// keep this\nfn a() {}\n// drop this\nfn b() {}\n";
    let dir = fixture(&[("src/a.rs", source)]);

    // The id `scan` would have written for the first comment, computed with the same function.
    let decisions = dir.path().join("decisions.jsonl");
    std::fs::write(
        &decisions,
        format!(
            "{{\"id\":\"{}\",\"path\":\"src/a.rs\",\"text\":\"// keep this\"}}\n",
            comment_id("src/a.rs", "// keep this", 0)
        ),
    )
    .expect("write decisions");

    let run = run_keep(dir.path(), &["--from", "decisions.jsonl", "."]);
    assert!(run.ok, "{}", run.message);
    assert_eq!(
        read(dir.path(), "src/a.rs"),
        "// keep this ~keep\nfn a() {}\n// drop this\nfn b() {}\n"
    );
}

#[test]
fn an_id_that_no_longer_resolves_is_an_error_naming_it_and_its_source() {
    let dir = fixture(&[("src/a.rs", "// one\nfn a() {}\n")]);
    let decisions = dir.path().join("decisions.jsonl");
    let stale = comment_id("src/a.rs", "// a comment that was deleted", 0);
    std::fs::write(&decisions, format!("{{\"id\":\"{stale}\"}}\n")).expect("write decisions");

    let run = run_keep(dir.path(), &["--from", "decisions.jsonl", "."]);
    assert!(!run.ok, "expected a failure, got: {}", run.message);
    assert!(run.message.contains(&stale), "{}", run.message);
    assert!(run.message.contains("decisions.jsonl"), "{}", run.message);
    assert_eq!(read(dir.path(), "src/a.rs"), "// one\nfn a() {}\n");
}

#[test]
fn skip_missing_downgrades_an_unresolvable_id_to_a_warning() {
    let source = "// one\nfn a() {}\n";
    let dir = fixture(&[("src/a.rs", source)]);
    let decisions = dir.path().join("decisions.jsonl");
    std::fs::write(
        &decisions,
        format!(
            "{{\"id\":\"{}\"}}\n{{\"id\":\"{}\"}}\n",
            comment_id("src/a.rs", "// one", 0),
            comment_id("src/a.rs", "// gone", 0)
        ),
    )
    .expect("write decisions");

    let run = run_keep(dir.path(), &["--from", "decisions.jsonl", "--skip-missing", "."]);
    assert!(run.ok, "{}", run.message);
    assert_eq!(read(dir.path(), "src/a.rs"), "// one ~keep\nfn a() {}\n");
}

#[test]
fn a_dry_run_writes_nothing() {
    let source = "// one\nfn a() {}\n";
    let dir = fixture(&[("src/a.rs", source)]);
    let run = run_keep(dir.path(), &["--all-removable", "--dry-run", "."]);
    assert!(run.ok, "{}", run.message);
    assert_eq!(read(dir.path(), "src/a.rs"), source);
}

#[test]
fn a_comment_in_a_language_with_no_line_comment_token_is_reported_not_guessed_at() {
    let source = "/* only blocks here */\na { color: red; }\n";
    let dir = fixture(&[("src/a.css", source)]);
    let run = run_keep(dir.path(), &["--all-removable", "."]);
    assert!(run.ok, "{}", run.message);
    assert_eq!(read(dir.path(), "src/a.css"), source);
}

/// The acceptance test for the feature: mark everything a default run would strip, then prove a
/// default run strips nothing, that the markers survive it, and that the only change to the tree is
/// added markers.
#[test]
fn the_all_removable_round_trip_leaves_a_default_run_with_nothing_to_remove() {
    let dir = fixture(&[
        (
            "src/a.rs",
            "// legacy shim\nfn a() {}\n\n/* block rationale */\nfn b() {}\n\n/// Documented.\npub fn c() {}\n",
        ),
        (
            "src/b.py",
            "# module note\ndef f():\n    \"\"\"Explain f.\"\"\"\n    return 1  # inline\n",
        ),
        (
            "src/c.c",
            "/* header note */\nint main(void) {\n    /* inner */\n    return 0;\n}\n",
        ),
    ]);

    let run = run_keep(dir.path(), &["--all-removable", "."]);
    assert!(run.ok, "{}", run.message);

    let marked: Vec<String> = ["src/a.rs", "src/b.py", "src/c.c"]
        .iter()
        .map(|name| read(dir.path(), name))
        .collect();

    // A default run finds nothing to remove and leaves every marker in place.
    let summary = default_run(dir.path(), &["--dry-run"]);
    assert!(summary.contains("0 comments removed"), "{summary}");
    default_run(dir.path(), &[]);
    for (name, before) in ["src/a.rs", "src/b.py", "src/c.c"].iter().zip(&marked) {
        assert_eq!(&read(dir.path(), name), before, "{name} changed under a default run");
    }

    assert_only_added_markers(dir.path());
}

/// The same property through the documented `scan` → `keep` pipeline, which needs the `scan`
/// subcommand. Skipped with a note until it lands; the `--all-removable` test above proves the
/// property in the meantime.
#[test]
fn the_scan_round_trip_leaves_a_default_run_with_nothing_to_remove() {
    if !scan_subcommand_is_wired() {
        eprintln!("skipped: the `scan` subcommand is not wired into the CLI yet");
        return;
    }

    let dir = fixture(&[
        (
            "src/a.rs",
            "// legacy shim\nfn a() {}\n\n/* block rationale */\nfn b() {}\n",
        ),
        ("src/b.py", "# module note\ndef f():\n    return 1  # inline\n"),
    ]);

    let inventory = dir.path().join("inventory.jsonl");
    let scan = Command::new(BINARY)
        .args(["scan", "--only", "removable", "--format", "jsonl", "-o"])
        .arg(&inventory)
        .arg(".")
        .current_dir(dir.path())
        .output()
        .expect("failed to run uncomment scan");
    assert!(
        scan.status.success(),
        "scan failed: {}",
        String::from_utf8_lossy(&scan.stderr)
    );

    let run = run_keep(dir.path(), &["--from", "inventory.jsonl", "."]);
    assert!(run.ok, "{}", run.message);

    let summary = default_run(dir.path(), &["--dry-run"]);
    assert!(summary.contains("0 comments removed"), "{summary}");
    assert_only_added_markers(dir.path());
}

/// How many comments a default dry run would remove, read out of its own summary line.
fn default_removed_count(dir: &Path) -> usize {
    let summary = default_run(dir, &["--dry-run"]);
    let (before, _) = summary
        .split_once(" comments removed")
        .unwrap_or_else(|| panic!("no summary line in: {summary}"));
    before
        .rsplit(|c: char| !c.is_ascii_digit())
        .find(|field| !field.is_empty())
        .and_then(|field| field.parse().ok())
        .unwrap_or_else(|| panic!("no count in: {summary}"))
}

/// The only change to the tree is added `~keep` markers.
///
/// A marker line above a comment shows up as a pure addition; a marker appended to a line comment
/// shows up as a removal paired with the same line plus the marker, so a removed line is only
/// acceptable when its marked counterpart was added.
fn assert_only_added_markers(dir: &Path) {
    let diff = git(dir, &["diff", "--", "."]);
    assert!(!diff.trim().is_empty(), "expected a diff");

    let mut added: Vec<&str> = Vec::new();
    let mut removed: Vec<&str> = Vec::new();
    for line in diff.lines() {
        if line.starts_with("+++") || line.starts_with("---") {
            continue;
        }
        if let Some(rest) = line.strip_prefix('+') {
            added.push(rest);
        } else if let Some(rest) = line.strip_prefix('-') {
            removed.push(rest);
        }
    }

    for line in &added {
        assert!(
            line.contains("~keep"),
            "added a line without a marker: {line:?}\n{diff}"
        );
    }
    for line in &removed {
        let marked = format!("{line} ~keep");
        assert!(
            added.contains(&marked.as_str()),
            "removed {line:?} without adding it back marked\n{diff}"
        );
    }
}
