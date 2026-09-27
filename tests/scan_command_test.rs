//! End-to-end tests for `uncomment scan`, driven through the compiled binary so the argument
//! surface, the report bytes, the stream each one lands on and the exit status are all covered.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_uncomment"))
}

/// A tree with one comment of every interesting kind, and a comment repeated across two files so
/// grouping has something to collapse.
fn fixture() -> TempDir {
    let temp = TempDir::new().expect("temp dir");
    let root = temp.path();
    fs::create_dir_all(root.join("src")).expect("create src");

    fs::write(
        root.join("src/main.rs"),
        "// leading comment\nfn main() {\n    // legacy shim\n    println!(\"x\"); // trailing note\n    \
         // TODO: keep me\n}\n\n/// Doc comment\nfn helper() {}\n\n// FOO handled here\nfn other() {}\n",
    )
    .expect("write main.rs");

    fs::write(
        root.join("src/other.rs"),
        "fn second() {\n    // legacy shim\n    /* block\n       comment */\n}\n",
    )
    .expect("write other.rs");

    fs::write(
        root.join("src/util.py"),
        "# a plain comment\ndef f():\n    \"\"\"Docstring.\"\"\"\n    return 1  # trailing note\n",
    )
    .expect("write util.py");

    temp
}

fn run_in(dir: &Path, args: &[&str]) -> Output {
    let output = Command::new(binary())
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run uncomment");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unrecognized subcommand"),
        "`uncomment scan` is not wired into the CLI yet: add the `Commands::Scan(ScanArgs)` variant \
         in src/cli.rs and its dispatch in src/main.rs. clap said: {stderr}"
    );
    output
}

fn scan(dir: &Path, args: &[&str]) -> String {
    let mut full = vec!["scan", "."];
    full.extend_from_slice(args);
    let output = run_in(dir, &full);
    assert!(
        output.status.success(),
        "scan {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("report is utf-8")
}

fn records(dir: &Path, args: &[&str]) -> Vec<Value> {
    scan(dir, args)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap_or_else(|error| panic!("line is not JSON ({error}): {line}")))
        .collect()
}

fn text_of(record: &Value) -> String {
    record["text"].as_str().unwrap_or_default().to_string()
}

fn verdict_of(record: &Value) -> String {
    record["verdict"].as_str().unwrap_or_default().to_string()
}

/// The verdict `scan` reports for the one record whose text contains `needle`.
fn verdict_for(dir: &Path, args: &[&str], needle: &str) -> String {
    let matching: Vec<Value> = records(dir, args)
        .into_iter()
        .filter(|record| text_of(record).contains(needle))
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one record for {needle}: {matching:?}"
    );
    verdict_of(&matching[0])
}

/// The `N comments removed` figure from a real run's summary line.
fn comments_removed(dir: &Path) -> usize {
    let output = run_in(dir, &[".", "--dry-run"]);
    assert!(output.status.success(), "the default run failed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let summary = stdout
        .lines()
        .find(|line| line.contains("comments removed"))
        .unwrap_or_else(|| panic!("no summary line in: {stdout}"));
    summary
        .split(" comments removed")
        .next()
        .and_then(|head| head.split_whitespace().next_back())
        .and_then(|count| count.parse().ok())
        .unwrap_or_else(|| panic!("no count in summary: {summary}"))
}

#[test]
fn every_jsonl_line_parses_on_its_own() {
    let temp = fixture();
    let report = scan(temp.path(), &[]);

    assert!(report.lines().count() >= 8, "expected a full inventory: {report}");
    for line in report.lines() {
        let record: Value = serde_json::from_str(line).unwrap_or_else(|error| panic!("{error}: {line}"));
        for key in [
            "id",
            "path",
            "line",
            "end_line",
            "start_byte",
            "end_byte",
            "kind",
            "verdict",
            "text",
        ] {
            assert!(record.get(key).is_some(), "missing {key} in {line}");
        }
        assert_eq!(record["id"].as_str().map(str::len), Some(10), "short id in {line}");
        assert!(record["path"].as_str().is_some_and(|path| !path.contains('\\')));
        assert!(matches!(verdict_of(&record).as_str(), "remove" | "preserve"), "{line}");
    }
}

/// The load-bearing agreement: the inventory's removable half is exactly what a real run strips.
#[test]
fn only_removable_matches_the_real_runs_removed_count() {
    let temp = fixture();
    let scanned = records(temp.path(), &["--only", "removable"]);
    assert!(scanned.iter().all(|record| verdict_of(record) == "remove"));
    assert_eq!(scanned.len(), comments_removed(temp.path()));
}

#[test]
fn removable_and_preserved_partition_the_inventory() {
    let temp = fixture();
    let all = records(temp.path(), &[]).len();
    let removable = records(temp.path(), &["--only", "removable"]).len();
    let preserved = records(temp.path(), &["--only", "preserved"]).len();

    assert!(removable > 0 && preserved > 0);
    assert_eq!(removable + preserved, all);
}

#[test]
fn preserved_records_carry_a_reason() {
    let temp = fixture();
    let preserved = records(temp.path(), &["--only", "preserved"]);

    assert!(!preserved.is_empty());
    for record in &preserved {
        let reason = record.get("reason").unwrap_or_else(|| panic!("no reason in {record}"));
        assert!(
            reason.is_string() || reason.get("pattern").and_then(Value::as_str).is_some(),
            "unexpected reason encoding: {reason}"
        );
    }

    let todo = preserved
        .iter()
        .find(|record| text_of(record).contains("TODO: keep me"))
        .unwrap_or_else(|| panic!("the TODO comment was not preserved: {preserved:?}"));
    assert_eq!(todo["reason"]["pattern"], "TODO");
}

#[test]
fn removed_records_carry_no_reason() {
    let temp = fixture();
    for record in records(temp.path(), &["--only", "removable"]) {
        assert!(record.get("reason").is_none(), "unexpected reason: {record}");
    }
}

#[test]
fn group_counts_sum_to_the_ungrouped_total() {
    let temp = fixture();
    let ungrouped = records(temp.path(), &[]).len();
    let groups = records(temp.path(), &["--group-identical"]);

    assert!(groups.len() < ungrouped, "grouping collapsed nothing: {groups:?}");
    let total: u64 = groups
        .iter()
        .map(|group| group["count"].as_u64().unwrap_or_default())
        .sum();
    assert_eq!(total, ungrouped as u64);

    let shim = groups
        .iter()
        .find(|group| text_of(group).contains("legacy shim"))
        .unwrap_or_else(|| panic!("no group for the repeated comment: {groups:?}"));
    assert_eq!(shim["count"], 2);
    assert_eq!(shim["sites"].as_array().map(Vec::len), Some(2));
    assert_eq!(shim["id"].as_str().map(str::len), Some(10));
    assert_eq!(verdict_of(shim), "remove");

    // Counts are the ordering key, so the largest group leads.
    let counts: Vec<u64> = groups
        .iter()
        .map(|group| group["count"].as_u64().unwrap_or_default())
        .collect();
    let mut sorted = counts.clone();
    sorted.sort_by(|left, right| right.cmp(left));
    assert_eq!(counts, sorted);
}

#[test]
fn a_group_whose_sites_disagree_reports_mixed() {
    let temp = TempDir::new().expect("temp dir");
    // The marker sits on its own line, so the comment below it is preserved without its own text
    // changing — the only way one wording can carry two verdicts.
    fs::write(temp.path().join("keep.rs"), "// ~keep\n// shared note\nfn a() {}\n").expect("write");
    fs::write(temp.path().join("drop.rs"), "// shared note\nfn b() {}\n").expect("write");

    let groups = records(temp.path(), &["--group-identical"]);
    let mixed = groups
        .iter()
        .find(|group| text_of(group) == "// shared note")
        .unwrap_or_else(|| panic!("no group for the shared wording: {groups:?}"));

    assert_eq!(mixed["count"], 2, "{mixed}");
    assert_eq!(verdict_of(mixed), "mixed");
    assert_eq!(mixed["removable"], 1);
    assert_eq!(mixed["preserved"], 1);
}

#[test]
fn a_comment_on_the_first_line_reports_line_one() {
    let temp = TempDir::new().expect("temp dir");
    fs::write(temp.path().join("first.rs"), "// on the very first line\nfn a() {}\n").expect("write");

    let first = &records(temp.path(), &[])[0];
    assert_eq!(first["line"], 1, "0-based row leaked into the report");
    assert_eq!(first["end_line"], 1);
    assert_eq!(first["start_byte"], 0);
}

#[test]
fn a_multi_line_comment_reports_both_of_its_lines() {
    let temp = TempDir::new().expect("temp dir");
    fs::write(
        temp.path().join("block.rs"),
        "fn a() {}\n/* one\n   two */\nfn b() {}\n",
    )
    .expect("write");

    let block = records(temp.path(), &[])
        .into_iter()
        .find(|record| record["kind"] == "block")
        .unwrap_or_else(|| panic!("no block comment reported"));
    assert_eq!(block["line"], 2);
    assert_eq!(block["end_line"], 3);
}

#[test]
fn remove_doc_moves_a_doc_comment_from_preserve_to_remove() {
    let temp = fixture();
    assert_eq!(verdict_for(temp.path(), &[], "Doc comment"), "preserve");
    assert_eq!(verdict_for(temp.path(), &["--remove-doc"], "Doc comment"), "remove");
}

#[test]
fn ignore_moves_a_matching_comment_from_remove_to_preserve() {
    let temp = fixture();
    assert_eq!(verdict_for(temp.path(), &[], "FOO handled here"), "remove");
    assert_eq!(
        verdict_for(temp.path(), &["--ignore", "FOO"], "FOO handled here"),
        "preserve"
    );

    let preserved = records(temp.path(), &["--ignore", "FOO", "--only", "preserved"]);
    let foo = preserved
        .iter()
        .find(|record| text_of(record).contains("FOO handled here"))
        .unwrap_or_else(|| panic!("FOO comment missing: {preserved:?}"));
    assert_eq!(foo["reason"]["pattern"], "FOO");
}

#[test]
fn two_runs_are_byte_identical() {
    let temp = fixture();
    let first = scan(temp.path(), &[]);
    let second = scan(temp.path(), &[]);
    assert_eq!(first, second);

    let grouped_first = scan(temp.path(), &["--group-identical"]);
    let grouped_second = scan(temp.path(), &["--group-identical"]);
    assert_eq!(grouped_first, grouped_second);
}

#[test]
fn the_thread_count_does_not_change_the_output() {
    let temp = fixture();
    assert_eq!(scan(temp.path(), &["-j", "1"]), scan(temp.path(), &["-j", "8"]));
    assert_eq!(
        scan(temp.path(), &["-j", "1", "--group-identical"]),
        scan(temp.path(), &["-j", "8", "--group-identical"])
    );
    assert_eq!(scan(temp.path(), &["-j", "8"]), scan(temp.path(), &["-j", "0"]));
}

#[test]
fn an_output_file_receives_the_report_and_stdout_stays_empty() {
    let temp = fixture();
    let expected = scan(temp.path(), &[]);

    let output = run_in(temp.path(), &["scan", ".", "-o", "report.jsonl"]);
    assert!(output.status.success());
    assert!(output.stdout.is_empty(), "stdout was not clean: {:?}", output.stdout);

    let written = fs::read_to_string(temp.path().join("report.jsonl")).expect("report file");
    assert_eq!(written, expected);
}

#[test]
fn json_format_is_a_single_array_of_the_same_records() {
    let temp = fixture();
    let parsed: Value = serde_json::from_str(&scan(temp.path(), &["--format", "json"])).expect("array parses");
    let array = parsed.as_array().unwrap_or_else(|| panic!("not an array: {parsed}"));
    assert_eq!(array.len(), records(temp.path(), &[]).len());
    assert_eq!(array[0], records(temp.path(), &[])[0]);
}

#[test]
fn json_format_of_an_empty_inventory_is_an_empty_array() {
    let temp = TempDir::new().expect("temp dir");
    fs::write(temp.path().join("bare.rs"), "fn a() {}\n").expect("write");

    assert_eq!(scan(temp.path(), &["--format", "json"]).trim(), "[]");
    assert_eq!(scan(temp.path(), &[]), "");
}

#[test]
fn text_format_is_one_line_per_comment() {
    let temp = fixture();
    let lines: Vec<String> = scan(temp.path(), &["--format", "text"])
        .lines()
        .map(str::to_string)
        .collect();

    assert_eq!(lines.len(), records(temp.path(), &[]).len());
    for line in &lines {
        assert!(
            line.contains("[remove]") || line.contains("[preserve]"),
            "no verdict in {line}"
        );
        assert!(line.contains(".rs:") || line.contains(".py:"), "no location in {line}");
    }
}

#[test]
fn a_non_utf8_file_is_reported_and_skipped_rather_than_fatal() {
    let temp = fixture();
    fs::write(temp.path().join("src/binary.rs"), [0x2f, 0x2f, 0xff, 0xfe, 0x0a]).expect("write bytes");

    let output = run_in(temp.path(), &["scan", "."]);
    assert!(output.status.success(), "a bad file must not be fatal");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("skipping"), "no skip on stderr: {stderr}");
    assert!(stderr.contains("binary.rs"), "the skipped path was not named: {stderr}");

    let report = String::from_utf8_lossy(&output.stdout);
    assert!(report.contains("legacy shim"), "the good files were not reported");
    assert!(!report.contains("binary.rs"), "the skipped file leaked into the report");
}

#[test]
fn source_files_are_byte_identical_after_a_scan() {
    let temp = fixture();
    let before = snapshot(temp.path());

    scan(temp.path(), &[]);
    scan(temp.path(), &["--remove-doc", "--only", "removable"]);
    scan(temp.path(), &["--group-identical", "--format", "text"]);

    assert_eq!(snapshot(temp.path()), before);
}

#[test]
fn paths_are_repo_relative_with_forward_slashes() {
    let temp = fixture();
    fs::create_dir_all(temp.path().join(".git")).expect("fake repo root");

    // Run from a subdirectory: the report must still be rooted at the repository, so a decisions
    // file does not depend on where it was produced.
    let report = scan(&temp.path().join("src"), &[]);
    for line in report.lines() {
        let record: Value = serde_json::from_str(line).expect("json");
        let path = record["path"].as_str().unwrap_or_default().to_string();
        assert!(path.starts_with("src/"), "not repo-relative: {path}");
        assert!(!path.contains('\\'));
    }
}

#[test]
fn ids_are_unique_and_stable_across_runs() {
    let temp = fixture();
    let first: Vec<String> = records(temp.path(), &[])
        .iter()
        .map(|record| record["id"].as_str().unwrap_or_default().to_string())
        .collect();

    let unique: std::collections::BTreeSet<&String> = first.iter().collect();
    assert_eq!(unique.len(), first.len(), "duplicate ids: {first:?}");

    // An edit above a comment must not move its id.
    let main = temp.path().join("src/main.rs");
    let content = fs::read_to_string(&main).expect("read");
    fs::write(&main, format!("use std::fmt;\n{content}")).expect("write");

    let second: Vec<String> = records(temp.path(), &[])
        .iter()
        .map(|record| record["id"].as_str().unwrap_or_default().to_string())
        .collect();
    assert_eq!(first, second);
}

#[test]
fn no_paths_is_an_error_rather_than_an_empty_report() {
    let temp = fixture();
    let output = run_in(temp.path(), &["scan"]);
    assert!(!output.status.success(), "scan with no paths must fail");
    assert!(output.stdout.is_empty());
}

fn snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut entries: Vec<(PathBuf, Vec<u8>)> = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| {
            let bytes = fs::read(entry.path()).unwrap_or_default();
            (entry.path().to_path_buf(), bytes)
        })
        .collect();
    entries.sort();
    entries
}
