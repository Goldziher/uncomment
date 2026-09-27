//! End-to-end tests for `uncomment lint`.
//!
//! Every test drives the command's real entry point over real files, with a real tree-sitter parse
//! and a real `.git/HEAD`, and asserts on the structured outcome rather than on printed text.
//!
//! [`uncomment::lint::lint`] and [`uncomment::lint::run_in`] take the base directory explicitly
//! instead of reading the process's working directory, which is what lets these run in parallel: a
//! test that `chdir`-ed into its own fixture would corrupt every other test in the binary.
//!
//! Arguments are built by parsing an argv through clap, so the flag names, defaults and `requires`
//! relationships under test are the ones the CLI actually exposes.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Parser;
use tempfile::TempDir;
use uncomment::lint::config::{Rule, Severity};
use uncomment::lint::{LintArgs, Outcome, lint, run_in};

#[derive(Parser, Debug)]
struct Harness {
    #[command(flatten)]
    lint: LintArgs,
}

fn args(argv: &[&str]) -> LintArgs {
    Harness::parse_from(std::iter::once("lint").chain(argv.iter().copied())).lint
}

const ENABLED: &str = "[lint]\nenabled = true\n";

/// A fixture repository: a hand-written `HEAD` so the branch is whatever the test needs, and a
/// config file holding only `[lint]`.
struct Fixture {
    dir: TempDir,
}

impl Fixture {
    fn new(branch: &str, config: &str) -> Self {
        let fixture = Self {
            dir: TempDir::new().expect("temp dir"),
        };
        fixture.write(".git/HEAD", &format!("ref: refs/heads/{branch}\n"));
        fixture.write(".uncomment.toml", config);
        fixture
    }

    fn enabled(branch: &str) -> Self {
        Self::new(branch, ENABLED)
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn write(&self, relative: &str, content: &str) -> PathBuf {
        let path = self.dir.path().join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(&path, content).expect("write fixture file");
        path
    }

    fn read(&self, relative: &str) -> String {
        fs::read_to_string(self.dir.path().join(relative)).expect("read fixture file")
    }

    fn lint(&self, argv: &[&str]) -> Outcome {
        lint(self.root(), &args(argv)).expect("lint run")
    }

    fn run(&self, argv: &[&str]) -> i32 {
        run_in(self.root(), &args(argv)).expect("lint run")
    }
}

fn rules(outcome: &Outcome) -> Vec<&'static str> {
    outcome.violations.iter().map(|v| v.rule.as_str()).collect()
}

// --- one rule at a time, in a `//` language and a `#` language -----------------------------------

#[test]
fn a_missing_key_is_reported_in_both_comment_styles() {
    let fixture = Fixture::enabled("feat/no-key-here");
    fixture.write("src/a.rs", "// TODO: wire this up\nfn main() {}\n");
    fixture.write("src/b.py", "# TODO: wire this up\nvalue = 1\n");

    let outcome = fixture.lint(&["src"]);
    assert_eq!(rules(&outcome), vec!["todo-missing-key", "todo-missing-key"]);
    assert_eq!(outcome.violations[0].path, PathBuf::from("src/a.rs"));
    assert_eq!(outcome.violations[0].line, 1);
    assert_eq!(outcome.violations[0].column, 4);
    assert_eq!(outcome.violations[1].path, PathBuf::from("src/b.py"));
    assert_eq!(outcome.violations[1].column, 3);
}

#[test]
fn a_keyed_canonical_tag_is_clean_in_both_comment_styles() {
    let fixture = Fixture::enabled("feat/no-key-here");
    fixture.write("src/a.rs", "// TODO(AMVP-1): wire this up\nfn main() {}\n");
    fixture.write("src/b.py", "# TODO(AMVP-1): wire this up\nvalue = 1\n");

    let outcome = fixture.lint(&["src"]);
    assert!(outcome.violations.is_empty(), "{:?}", outcome.violations);
    assert_eq!(outcome.summary.files_linted, 2);
    assert_eq!(outcome.exit_code(), 0);
}

#[test]
fn a_non_canonical_tag_is_reported_in_both_comment_styles() {
    let fixture = Fixture::enabled("feat/no-key-here");
    fixture.write("src/a.rs", "// FIXME(AMVP-1): rework\nfn main() {}\n");
    fixture.write("src/b.py", "# HACK(AMVP-1): rework\nvalue = 1\n");

    let outcome = fixture.lint(&["src"]);
    assert_eq!(rules(&outcome), vec!["tag-not-canonical", "tag-not-canonical"]);
    assert!(outcome.violations.iter().all(|v| v.severity == Severity::Error));
}

#[test]
fn an_untagged_comment_is_never_reported() {
    let fixture = Fixture::enabled("feat/no-key-here");
    fixture.write("src/a.rs", "// a plain explanatory comment\n/// docs\nfn main() {}\n");
    fixture.write("src/b.py", "# a plain explanatory comment\nvalue = 1\n");

    assert!(fixture.lint(&["src"]).violations.is_empty());
}

// --- tag casing ----------------------------------------------------------------------------------

#[test]
fn a_tag_written_in_any_casing_is_recognised_and_fixed_to_the_canonical_form() {
    let fixture = Fixture::enabled("feat/casing");
    let source = "\
# fixme: a
# todo: b
# Todo: c
# XXX: d
# TODO(PROJ-1): e
value = 1
";
    fixture.write("src/a.py", source);

    let outcome = fixture.lint(&["src"]);
    assert_eq!(
        rules(&outcome),
        vec![
            "tag-not-canonical",
            "todo-missing-key",
            "tag-not-canonical",
            "todo-missing-key",
            "tag-not-canonical",
            "todo-missing-key",
            "tag-not-canonical",
            "todo-missing-key",
        ],
        "every casing is a tag; only the keyed canonical one is clean"
    );
    assert_eq!(
        outcome.violations.iter().map(|v| v.tag.as_str()).collect::<Vec<_>>(),
        vec!["fixme", "fixme", "todo", "todo", "Todo", "Todo", "XXX", "XXX"],
        "the tag is reported as it was written"
    );

    fixture.lint(&["src", "--fix"]);
    assert_eq!(
        fixture.read("src/a.py"),
        "\
# TODO: a
# TODO: b
# TODO: c
# TODO: d
# TODO(PROJ-1): e
value = 1
"
    );

    // Idempotent: the casing rewrite leaves nothing for a second run to do.
    let after_first = fixture.read("src/a.py");
    fixture.lint(&["src", "--fix"]);
    assert_eq!(fixture.read("src/a.py"), after_first);
}

#[test]
fn a_casing_only_violation_reads_differently_from_a_differently_spelled_tag() {
    let fixture = Fixture::enabled("feat/casing");
    fixture.write("src/a.py", "# todo(PROJ-1): a\n# FIXME(PROJ-2): b\nvalue = 1\n");

    let outcome = fixture.lint(&["src"]);
    assert_eq!(rules(&outcome), vec!["tag-not-canonical", "tag-not-canonical"]);
    assert!(
        outcome.violations[0].message.contains("casing"),
        "a casing defect must say so rather than `TODO` should be written as `TODO`: {}",
        outcome.violations[0].message
    );
    assert_eq!(outcome.violations[1].message, "`FIXME` should be written as `TODO`");
}

#[test]
fn a_miscased_tag_reports_its_missing_key_the_same_way_an_uppercase_one_does() {
    let fixture = Fixture::enabled("naaman.AMVP-1.testbed");
    fixture.write("src/a.py", "# fixme: no key\nvalue = 1\n");
    fixture.write("src/b.py", "# FIXME: no key\nvalue = 1\n");

    let outcome = fixture.lint(&["src"]);
    assert_eq!(
        rules(&outcome),
        vec![
            "tag-not-canonical",
            "todo-missing-key",
            "tag-not-canonical",
            "todo-missing-key",
        ]
    );
}

#[test]
fn a_miscased_tag_still_self_references_because_its_key_is_found() {
    let fixture = Fixture::enabled("naaman.AMVP-1.testbed");
    fixture.write("src/a.py", "# fixme(AMVP-1): the current issue\nvalue = 1\n");

    let outcome = fixture.lint(&["src"]);
    assert_eq!(
        rules(&outcome),
        vec!["tag-not-canonical", "todo-self-reference"],
        "a lowercase tag must not hide the key from `key_pattern`"
    );
    assert_eq!(outcome.violations[1].key.as_deref(), Some("AMVP-1"));
}

#[test]
fn a_miscased_key_is_not_accepted_as_a_key() {
    let fixture = Fixture::enabled("feat/casing");
    fixture.write("src/a.py", "# todo(proj-1): a\nvalue = 1\n");

    let outcome = fixture.lint(&["src"]);
    assert_eq!(
        rules(&outcome),
        vec!["tag-not-canonical", "todo-missing-key"],
        "loosening the tag half of key_pattern must not loosen the key half"
    );
    assert_eq!(outcome.violations[1].key, None);
}

#[test]
fn a_miscased_tag_inside_a_string_literal_is_still_not_flagged() {
    let fixture = Fixture::enabled("feat/casing");
    fixture.write(
        "src/a.rs",
        "fn main() {\n    let note = \"todo: this is data\";\n    let other = \"fixme: also data\";\n}\n",
    );
    fixture.write(
        "src/b.py",
        "NOTE = \"todo: this is data\"\nOTHER = 'Fixme: also data'\n",
    );

    let outcome = fixture.lint(&["src"]);
    assert!(
        outcome.violations.is_empty(),
        "case-insensitive matching must not turn a string literal into a comment: {:?}",
        outcome.violations
    );
    assert_eq!(outcome.summary.files_linted, 2);
}

#[test]
fn case_sensitive_tags_reverts_to_literal_matching() {
    let fixture = Fixture::new("feat/casing", "[lint]\nenabled = true\ncase_sensitive_tags = true\n");
    let source = "# fixme: a\n# todo: b\n# Todo: c\nvalue = 1\n";
    fixture.write("src/a.py", source);
    fixture.write("src/b.py", "# FIXME: a\nvalue = 1\n");

    let outcome = fixture.lint(&["src", "--fix"]);
    assert_eq!(
        rules(&outcome),
        vec!["tag-not-canonical", "todo-missing-key"],
        "only the literal casing in lint.tags is a tag"
    );
    assert_eq!(outcome.violations[0].path, PathBuf::from("src/b.py"));
    assert_eq!(fixture.read("src/a.py"), source, "--fix must leave it byte-identical");
}

// --- todo-self-reference ------------------------------------------------------------------------

#[test]
fn a_self_reference_fires_only_when_the_key_is_the_issue_the_branch_names() {
    let fixture = Fixture::enabled("naaman.hirschfeld.AMVP-160815.testbed");
    fixture.write("src/a.rs", "// TODO(AMVP-160815): drop the shim\nfn main() {}\n");
    fixture.write("src/b.rs", "// TODO(AMVP-999999): drop the shim\nfn main() {}\n");

    let outcome = fixture.lint(&["src"]);
    assert_eq!(rules(&outcome), vec!["todo-self-reference"]);
    assert_eq!(outcome.violations[0].path, PathBuf::from("src/a.rs"));
    assert_eq!(outcome.violations[0].key.as_deref(), Some("AMVP-160815"));
    assert_eq!(outcome.exit_code(), 1);
}

#[test]
fn the_short_armis_branch_shape_resolves_the_same_issue() {
    let fixture = Fixture::enabled("naaman.AMVP-160815.testbed");
    fixture.write("src/a.rs", "// TODO(AMVP-160815): drop the shim\nfn main() {}\n");

    assert_eq!(rules(&fixture.lint(&["src"])), vec!["todo-self-reference"]);
}

#[test]
fn a_branch_carrying_no_key_skips_the_rule_with_a_note_rather_than_an_error() {
    let fixture = Fixture::enabled("feat/comment-inventory");
    fixture.write("src/a.rs", "// TODO(AMVP-160815): drop the shim\nfn main() {}\n");

    let outcome = fixture.lint(&["src"]);
    assert!(outcome.violations.is_empty(), "{:?}", outcome.violations);
    assert_eq!(outcome.exit_code(), 0);
    assert!(
        outcome
            .notes
            .iter()
            .any(|note| note.contains("todo-self-reference not checked") && note.contains("feat/comment-inventory")),
        "{:?}",
        outcome.notes
    );
}

#[test]
fn a_detached_head_skips_the_rule_with_a_note() {
    let fixture = Fixture::enabled("placeholder");
    fixture.write(".git/HEAD", "1077b28f9a4c5d6e7f8091a2b3c4d5e6f7089a1b\n");
    fixture.write("src/a.rs", "// TODO(AMVP-160815): drop the shim\nfn main() {}\n");

    let outcome = fixture.lint(&["src"]);
    assert!(outcome.violations.is_empty(), "{:?}", outcome.violations);
    assert!(
        outcome.notes.iter().any(|note| note.contains("HEAD is detached")),
        "{:?}",
        outcome.notes
    );
}

#[test]
fn no_git_repository_skips_the_rule_with_a_note() {
    // No `.git` at all: `find_repo_root` returns nothing and there is no branch to compare against.
    let dir = TempDir::new().expect("temp dir");
    fs::write(dir.path().join(".uncomment.toml"), ENABLED).expect("write config");
    fs::write(
        dir.path().join("a.rs"),
        "// TODO(AMVP-160815): drop the shim\nfn main() {}\n",
    )
    .expect("write source");

    let outcome = lint(dir.path(), &args(&["a.rs"])).expect("lint run");
    assert!(outcome.violations.is_empty(), "{:?}", outcome.violations);
    assert!(
        outcome.notes.iter().any(|note| note.contains("no git repository")),
        "{:?}",
        outcome.notes
    );
}

// --- the tree-sitter payoff ---------------------------------------------------------------------

#[test]
fn a_todo_inside_a_string_literal_is_not_flagged() {
    let fixture = Fixture::enabled("feat/strings");
    fixture.write(
        "src/a.rs",
        r#"fn main() {
    let note = "TODO: this is data, not a comment";
    let raw = r"FIXME: also data";
    println!("{note} {raw}");
}
"#,
    );
    fixture.write(
        "src/b.py",
        "NOTE = \"TODO: this is data, not a comment\"\nOTHER = 'FIXME: also data'\n",
    );

    let outcome = fixture.lint(&["src"]);
    assert!(
        outcome.violations.is_empty(),
        "a text-matching linter would flag all four: {:?}",
        outcome.violations
    );
    assert_eq!(outcome.summary.files_linted, 2);
}

#[test]
fn the_same_text_in_a_comment_is_flagged_so_the_string_test_is_not_vacuous() {
    let fixture = Fixture::enabled("feat/strings");
    fixture.write("src/a.rs", "fn main() {\n    // TODO: this is data, not a comment\n}\n");
    fixture.write("src/b.py", "# TODO: this is data, not a comment\nvalue = 1\n");

    assert_eq!(
        rules(&fixture.lint(&["src"])),
        vec!["todo-missing-key", "todo-missing-key"]
    );
}

// --- --fix --------------------------------------------------------------------------------------

#[test]
fn fix_rewrites_every_non_canonical_tag_and_preserves_punctuation() {
    let fixture = Fixture::enabled("feat/fixing");
    let source = "\
// FIXME(AMVP-1): keeps its key
    // FIXME: no key
/* HACK(AMVP-2): block form */
// XXX(AMVP-3): shouting
fn main() {}
";
    fixture.write("src/a.rs", source);

    let outcome = fixture.lint(&["src", "--fix"]);
    assert_eq!(
        fixture.read("src/a.rs"),
        "\
// TODO(AMVP-1): keeps its key
    // TODO: no key
/* TODO(AMVP-2): block form */
// TODO(AMVP-3): shouting
fn main() {}
"
    );

    // The rewritten-but-still-unkeyed comment is reported in the same run.
    assert_eq!(outcome.summary.fixed, 4);
    let missing = outcome.by_rule(Rule::TodoMissingKey);
    assert_eq!(missing.len(), 1);
    assert!(!missing[0].fixed, "a key cannot be invented");
    assert_eq!(outcome.exit_code(), 1, "the missing key is still unresolved");
}

#[test]
fn fix_also_works_in_a_hash_language() {
    let fixture = Fixture::enabled("feat/fixing");
    fixture.write("src/b.py", "# FIXME(PPSC-7): rework\nvalue = 1  # XXX(PPSC-8): why\n");

    fixture.lint(&["src", "--fix"]);
    assert_eq!(
        fixture.read("src/b.py"),
        "# TODO(PPSC-7): rework\nvalue = 1  # TODO(PPSC-8): why\n"
    );
}

#[test]
fn fix_is_idempotent_on_already_canonical_input() {
    let fixture = Fixture::enabled("feat/fixing");
    let canonical = "// TODO(AMVP-1): keep\n// TODO(AMVP-2): keep\nfn main() {}\n";
    fixture.write("src/a.rs", canonical);

    for _ in 0..2 {
        let outcome = fixture.lint(&["src", "--fix"]);
        assert_eq!(outcome.summary.fixed, 0);
        assert_eq!(fixture.read("src/a.rs"), canonical);
    }
}

#[test]
fn fixing_a_rewrite_twice_changes_nothing_the_second_time() {
    let fixture = Fixture::enabled("feat/fixing");
    fixture.write("src/a.rs", "// FIXME(AMVP-1): rework\nfn main() {}\n");

    fixture.lint(&["src", "--fix"]);
    let after_first = fixture.read("src/a.rs");
    fixture.lint(&["src", "--fix"]);
    assert_eq!(fixture.read("src/a.rs"), after_first);
    assert_eq!(after_first, "// TODO(AMVP-1): rework\nfn main() {}\n");
}

#[test]
fn fix_never_touches_anything_outside_a_comments_byte_range() {
    let fixture = Fixture::enabled("feat/fixing");
    // Every `FIXME` here is code or data, not a comment; only the last line holds a real one.
    let source = r#"fn main() {
    let fixme = "FIXME: in a string";
    let raw = r"XXX: also in a string";
    let ident = FIXME_CONSTANT;
    println!("{fixme} {raw} {ident}"); // FIXME(AMVP-1): the only comment
}
"#;
    fixture.write("src/a.rs", source);

    fixture.lint(&["src", "--fix"]);
    assert_eq!(
        fixture.read("src/a.rs"),
        source.replace("// FIXME(AMVP-1)", "// TODO(AMVP-1)")
    );
    assert!(fixture.read("src/a.rs").contains(r#""FIXME: in a string""#));
    assert!(fixture.read("src/a.rs").contains("FIXME_CONSTANT"));
}

#[test]
fn todo_key_supplies_the_key_that_cannot_be_invented() {
    let fixture = Fixture::enabled("feat/fixing");
    fixture.write("src/a.rs", "// TODO: no key\n// FIXME no colon either\nfn main() {}\n");

    let outcome = fixture.lint(&["src", "--fix", "--todo-key", "AMVP-42"]);
    assert_eq!(
        fixture.read("src/a.rs"),
        "// TODO(AMVP-42): no key\n// TODO(AMVP-42): no colon either\nfn main() {}\n"
    );
    assert!(outcome.violations.iter().all(|v| v.fixed));
    assert_eq!(outcome.exit_code(), 0);

    // And the result is clean, so the fix is idempotent.
    assert!(fixture.lint(&["src"]).violations.is_empty());
}

// --- lint removes nothing -----------------------------------------------------------------------

#[test]
fn a_run_without_fix_leaves_every_file_byte_identical() {
    let fixture = Fixture::enabled("naaman.AMVP-160815.testbed");
    let sources = [
        (
            "src/a.rs",
            "// FIXME: everything wrong at once\n// TODO(AMVP-160815): self reference\nfn main() {}\n",
        ),
        ("src/b.py", "# XXX: no key\n\"\"\"docstring\"\"\"\nvalue = 1\n"),
        ("src/c.js", "// HACK(AMVP-1): rework\nconst x = 1;\n"),
    ];
    for (path, content) in sources {
        fixture.write(path, content);
    }

    let outcome = fixture.lint(&["src"]);
    assert!(!outcome.violations.is_empty(), "the fixture must actually violate");
    assert_eq!(outcome.summary.fixed, 0);

    for (path, content) in sources {
        assert_eq!(fixture.read(path), content, "{path} was modified by a lint-only run");
    }
}

// --- exit codes ---------------------------------------------------------------------------------

#[test]
fn the_exit_code_is_one_with_violations_and_zero_without() {
    let fixture = Fixture::enabled("feat/exit-codes");
    fixture.write("src/clean.rs", "// TODO(AMVP-1): fine\nfn main() {}\n");
    assert_eq!(fixture.run(&["src/clean.rs"]), 0);

    fixture.write("src/dirty.rs", "// TODO: not fine\nfn main() {}\n");
    assert_eq!(fixture.run(&["src/dirty.rs"]), 1);
}

#[test]
fn a_warn_severity_reports_without_failing_the_run() {
    let fixture = Fixture::new(
        "feat/warnings",
        "[lint]\nenabled = true\n\n[lint.rules]\ntodo-missing-key = \"warn\"\n",
    );
    fixture.write("src/a.rs", "// TODO: not fine\nfn main() {}\n");

    let outcome = fixture.lint(&["src"]);
    assert_eq!(outcome.summary.warnings, 1);
    assert_eq!(outcome.summary.errors, 0);
    assert_eq!(outcome.exit_code(), 0);
}

#[test]
fn lint_disabled_reports_nothing_at_all() {
    let fixture = Fixture::new("feat/disabled", "[lint]\nenabled = false\n");
    fixture.write("src/a.rs", "// FIXME: everything wrong\nfn main() {}\n");

    let outcome = fixture.lint(&["src"]);
    assert!(outcome.violations.is_empty());
    assert_eq!(outcome.summary.files_linted, 0);
    assert_eq!(outcome.summary.files_skipped_disabled, 1);
    assert_eq!(outcome.exit_code(), 0);
}

#[test]
fn an_absent_lint_table_leaves_lint_inert() {
    let fixture = Fixture::new("feat/absent", "[global]\nremove_todos = false\n");
    fixture.write("src/a.rs", "// FIXME: everything wrong\nfn main() {}\n");

    let outcome = fixture.lint(&["src"]);
    assert!(outcome.violations.is_empty());
    assert_eq!(outcome.exit_code(), 0);
}

#[test]
fn an_invalid_key_pattern_is_a_load_time_error_naming_the_key() {
    let fixture = Fixture::new("feat/broken", "[lint]\nenabled = true\nkey_pattern = '([unclosed'\n");
    fixture.write("src/a.rs", "// TODO: whatever\nfn main() {}\n");

    let error = lint(fixture.root(), &args(&["src"])).expect_err("a bad pattern must fail the load");
    let message = format!("{error:#}");
    assert!(message.contains("lint.key_pattern"), "{message}");
    assert!(message.contains("([unclosed"), "{message}");
}

#[test]
fn an_invalid_pattern_is_refused_before_fix_rewrites_anything() {
    let fixture = Fixture::new(
        "feat/broken",
        "[lint]\nenabled = true\ncurrent_issue_from_branch = '(?P<'\n",
    );
    let source = "// FIXME: would otherwise be rewritten\nfn main() {}\n";
    fixture.write("src/a.rs", source);

    assert!(lint(fixture.root(), &args(&["src", "--fix"])).is_err());
    assert_eq!(fixture.read("src/a.rs"), source);
}

// --- nested `[lint]` tables ---------------------------------------------------------------------

/// Violations as `(path, rule, severity)`, with the path slash-separated so the assertions read the
/// same on every platform.
fn located(outcome: &Outcome) -> Vec<(String, &'static str, &'static str)> {
    outcome
        .violations
        .iter()
        .map(|v| (slash(&v.path), v.rule.as_str(), v.severity.as_str()))
        .collect()
}

fn slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[test]
fn a_nested_lint_table_applies_to_that_directory_and_not_above_it() {
    let fixture = Fixture::enabled("feat/nested");
    fixture.write(
        "src/nested/.uncomment.toml",
        "[lint]\ntags = [\"NOTE\"]\ncanonical_tag = \"NOTE\"\n",
    );
    fixture.write(
        "src/a.rs",
        "// NOTE: not a tag up here\n// TODO: no key\nfn main() {}\n",
    );
    fixture.write(
        "src/nested/b.rs",
        "// NOTE: no key\n// TODO: not a tag down here\nfn main() {}\n",
    );

    // Named file by file: a `.uncomment.toml` is itself a TOML file a directory walk would lint.
    let outcome = fixture.lint(&["src/a.rs", "src/nested/b.rs"]);
    assert_eq!(
        outcome.summary.files_linted, 2,
        "the nested table names no `enabled`, so it inherits the one above: {:?}",
        outcome.notes
    );
    assert_eq!(
        located(&outcome),
        vec![
            ("src/a.rs".to_string(), "todo-missing-key", "error"),
            ("src/nested/b.rs".to_string(), "todo-missing-key", "error"),
        ]
    );
    // The tag each was reported for is what shows the tables did not leak into one another.
    assert_eq!(outcome.violations[0].tag, "TODO");
    assert_eq!(outcome.violations[0].line, 2);
    assert_eq!(outcome.violations[1].tag, "NOTE");
    assert_eq!(outcome.violations[1].line, 1);
}

#[test]
fn a_nested_lint_table_overrides_only_the_keys_it_names() {
    let fixture = Fixture::new(
        "feat/nested",
        "[lint]\nenabled = true\n\n[lint.rules]\ntag-not-canonical = \"warn\"\n",
    );
    fixture.write(
        "src/nested/.uncomment.toml",
        "[lint.rules]\ntodo-missing-key = \"off\"\n",
    );
    fixture.write("src/a.rs", "// FIXME: no key\nfn main() {}\n");
    fixture.write("src/nested/b.rs", "// FIXME: no key\nfn main() {}\n");

    let outcome = fixture.lint(&["src/a.rs", "src/nested/b.rs"]);
    assert_eq!(outcome.summary.files_linted, 2, "{:?}", outcome.notes);
    assert_eq!(
        located(&outcome),
        vec![
            // Above: the root's `warn`, and the default `error` for the missing key.
            ("src/a.rs".to_string(), "tag-not-canonical", "warn"),
            ("src/a.rs".to_string(), "todo-missing-key", "error"),
            // Below: `todo-missing-key` silenced, `tag-not-canonical` still the root's `warn` and
            // `enabled` still the root's `true` — the nested table named neither.
            ("src/nested/b.rs".to_string(), "tag-not-canonical", "warn"),
        ]
    );
    assert_eq!(outcome.exit_code(), 1, "only the violation above is an error");
}

#[test]
fn a_broken_regex_in_a_nested_lint_table_is_an_error_naming_that_file() {
    let fixture = Fixture::enabled("feat/nested");
    fixture.write("src/nested/.uncomment.toml", "[lint]\nkey_pattern = '([unclosed'\n");
    fixture.write("src/nested/b.rs", "// TODO: whatever\nfn main() {}\n");

    let error = lint(fixture.root(), &args(&["src"])).expect_err("a bad nested pattern must fail the load");
    let message = format!("{error:#}");
    assert!(message.contains("lint.key_pattern"), "{message}");
    assert!(message.contains("([unclosed"), "{message}");
    assert!(
        message.contains("nested") && message.contains(".uncomment.toml"),
        "the file carrying the broken pattern must be the one named: {message}"
    );
}

#[test]
fn a_nested_config_that_cannot_be_loaded_fails_the_lint_run() {
    // Reading the table through `ConfigManager` means lint sees the same rejection every other
    // command sees, including from a directory below the one it was invoked in.
    let fixture = Fixture::enabled("feat/nested");
    fixture.write("src/nested/.uncomment.toml", "[global]\nremove_todos = 'not a bool'\n");
    fixture.write("src/nested/b.rs", "// TODO: whatever\nfn main() {}\n");

    let error = lint(fixture.root(), &args(&["src"])).expect_err("a rejected config must stop the run");
    let message = format!("{error:#}");
    assert!(
        message.contains("nested") && message.contains(".uncomment.toml"),
        "{message}"
    );
}

#[test]
fn one_config_file_serves_both_the_lint_table_and_the_removal_settings() {
    // `Config` carries `deny_unknown_fields`, so before `[lint]` became a field on it this very
    // file was `unknown field `lint`` for every command except `lint` itself.
    let fixture = Fixture::new(
        "feat/one-file",
        "[global]\nremove_todos = true\n\n[lint]\nenabled = true\n",
    );
    fixture.write("src/a.rs", "// TODO: no key\nfn main() {}\n");

    let config = uncomment::config::Config::from_file(fixture.root().join(".uncomment.toml"))
        .expect("one file must serve both readers");
    assert!(config.global.remove_todos);
    assert_eq!(rules(&fixture.lint(&["src"])), vec!["todo-missing-key"]);
}

// --- baselines ----------------------------------------------------------------------------------

#[test]
fn every_baselined_violation_is_informational_and_the_run_succeeds() {
    let fixture = Fixture::enabled("feat/baseline");
    fixture.write(
        "src/a.rs",
        "// TODO: pre-existing\n// FIXME: also pre-existing\nfn main() {}\n",
    );
    let baseline = fixture.root().join("lint-baseline.json");
    let baseline_arg = baseline.to_string_lossy().to_string();

    assert_eq!(
        fixture.run(&["src", "--baseline", &baseline_arg, "--write-baseline"]),
        0
    );
    assert!(baseline.is_file());

    let outcome = fixture.lint(&["src", "--baseline", &baseline_arg]);
    assert_eq!(outcome.violations.len(), 3, "{:?}", rules(&outcome));
    assert!(outcome.violations.iter().all(|v| v.baselined));
    assert_eq!(outcome.summary.baselined, 3);
    assert_eq!(outcome.exit_code(), 0);
}

#[test]
fn a_new_violation_alongside_baselined_ones_still_fails_the_run() {
    let fixture = Fixture::enabled("feat/baseline");
    fixture.write("src/a.rs", "// TODO: pre-existing\nfn main() {}\n");
    let baseline = fixture.root().join("lint-baseline.json");
    let baseline_arg = baseline.to_string_lossy().to_string();
    fixture.run(&["src", "--baseline", &baseline_arg, "--write-baseline"]);

    fixture.write("src/b.rs", "// TODO: brand new\nfn main() {}\n");
    let outcome = fixture.lint(&["src", "--baseline", &baseline_arg]);
    assert_eq!(outcome.summary.baselined, 1);
    assert_eq!(outcome.failing(), 1);
    assert_eq!(outcome.exit_code(), 1);
}

#[test]
fn a_baseline_survives_lines_being_inserted_above_the_violating_comment() {
    let fixture = Fixture::enabled("feat/baseline");
    fixture.write("src/a.rs", "// TODO: pre-existing\nfn main() {}\n");
    let baseline = fixture.root().join("lint-baseline.json");
    let baseline_arg = baseline.to_string_lossy().to_string();
    fixture.run(&["src", "--baseline", &baseline_arg, "--write-baseline"]);

    // The comment id carries no line or byte offset, which is the whole point.
    fixture.write(
        "src/a.rs",
        "use std::fmt;\n\nfn helper() {}\n\n// TODO: pre-existing\nfn main() {}\n",
    );

    let outcome = fixture.lint(&["src", "--baseline", &baseline_arg]);
    assert_eq!(outcome.violations.len(), 1);
    assert_eq!(outcome.violations[0].line, 5, "it did move");
    assert!(outcome.violations[0].baselined, "but it is still the same comment");
    assert_eq!(outcome.exit_code(), 0);
}

#[test]
fn editing_a_baselined_comments_text_un_baselines_it() {
    let fixture = Fixture::enabled("feat/baseline");
    fixture.write("src/a.rs", "// TODO: pre-existing\nfn main() {}\n");
    let baseline = fixture.root().join("lint-baseline.json");
    let baseline_arg = baseline.to_string_lossy().to_string();
    fixture.run(&["src", "--baseline", &baseline_arg, "--write-baseline"]);

    fixture.write(
        "src/a.rs",
        "// TODO: reworded, so a new decision is due\nfn main() {}\n",
    );
    assert_eq!(fixture.lint(&["src", "--baseline", &baseline_arg]).exit_code(), 1);
}

#[test]
fn a_missing_baseline_file_is_an_error_rather_than_an_empty_one() {
    let fixture = Fixture::enabled("feat/baseline");
    fixture.write("src/a.rs", "// TODO: pre-existing\nfn main() {}\n");

    let error = lint(fixture.root(), &args(&["src", "--baseline", "does-not-exist.json"]))
        .expect_err("a typo'd baseline path must not read as a clean sheet");
    assert!(format!("{error:#}").contains("--write-baseline"), "{error:#}");
}

#[test]
fn write_baseline_records_nothing_when_there_is_nothing_to_record() {
    let fixture = Fixture::enabled("feat/baseline");
    fixture.write("src/a.rs", "// TODO(AMVP-1): fine\nfn main() {}\n");
    let baseline = fixture.root().join("lint-baseline.json");
    let baseline_arg = baseline.to_string_lossy().to_string();

    assert_eq!(
        fixture.run(&["src", "--baseline", &baseline_arg, "--write-baseline"]),
        0
    );
    let recorded: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&baseline).expect("read")).expect("parse");
    assert_eq!(recorded["violations"].as_array().map(Vec::len), Some(0));
    assert_eq!(recorded["version"], 1);
}

// --- --changed-only -----------------------------------------------------------------------------

#[test]
fn changed_only_limits_the_run_to_files_changed_against_the_base() {
    let fixture = Fixture::enabled("placeholder");
    let root = fixture.root();
    // A real repository, because `--changed-only` shells out to `git diff`.
    fs::remove_dir_all(root.join(".git")).expect("drop the hand-written .git");
    git(root, &["init", "-b", "main"]);
    git(root, &["config", "user.email", "lint@example.com"]);
    git(root, &["config", "user.name", "Lint Fixture"]);

    fixture.write("src/old.rs", "// TODO: pre-existing\nfn main() {}\n");
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "base"]);

    git(root, &["checkout", "-b", "feat/new"]);
    fixture.write("src/new.rs", "// TODO: brand new\nfn main() {}\n");
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "change"]);

    let everything = fixture.lint(&["src"]);
    assert_eq!(everything.violations.len(), 2);

    let changed = fixture.lint(&["src", "--changed-only"]);
    assert_eq!(changed.violations.len(), 1, "{:?}", changed.violations);
    assert_eq!(changed.violations[0].path, PathBuf::from("src/new.rs"));
    assert!(
        changed
            .notes
            .iter()
            .any(|note| note.contains("--changed-only against main")),
        "the default base ref must be main, never master: {:?}",
        changed.notes
    );

    // An explicit base ref overrides the default.
    let explicit = fixture.lint(&["src", "--changed-only", "--base", "main"]);
    assert_eq!(explicit.violations.len(), 1);
}

fn git(root: &Path, argv: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(argv)
        .output()
        .expect("run git");
    assert!(
        status.status.success(),
        "git {argv:?} failed: {}",
        String::from_utf8_lossy(&status.stderr)
    );
}

// --- output formats -----------------------------------------------------------------------------

#[test]
fn both_output_formats_render_and_agree_on_the_exit_code() {
    let fixture = Fixture::enabled("naaman.AMVP-160815.testbed");
    fixture.write(
        "src/a.rs",
        "// FIXME: no key\n// TODO(AMVP-160815): self reference\nfn main() {}\n",
    );

    assert_eq!(fixture.run(&["src"]), 1);
    assert_eq!(fixture.run(&["src", "--format", "json"]), 1);
    assert_eq!(fixture.run(&["src", "--format", "text"]), 1);
}

// --- argument surface ---------------------------------------------------------------------------

#[test]
fn write_baseline_requires_a_baseline_path() {
    let parsed = Harness::try_parse_from(["lint", "src", "--write-baseline"]);
    assert!(parsed.is_err(), "--write-baseline alone has nowhere to write");
}

#[test]
fn the_process_flags_are_flattened_in() {
    let parsed = args(&["src", "--no-gitignore", "-j", "4", "-c", "custom.toml"]);
    assert!(parsed.process.no_gitignore);
    assert_eq!(parsed.process.threads, 4);
    assert_eq!(parsed.process.config, Some(PathBuf::from("custom.toml")));
    assert_eq!(parsed.process.paths, vec!["src".to_string()]);
}

#[test]
fn no_paths_is_an_error_rather_than_a_silent_whole_tree_run() {
    let fixture = Fixture::enabled("feat/empty");
    let error = lint(fixture.root(), &args(&[])).expect_err("no paths must be rejected");
    assert!(format!("{error:#}").contains("no input paths"), "{error:#}");
}

#[test]
fn threads_greater_than_one_produces_the_same_findings() {
    let fixture = Fixture::enabled("feat/threads");
    for index in 0..8 {
        fixture.write(&format!("src/f{index}.rs"), "// TODO: no key\nfn main() {}\n");
    }

    let sequential = fixture.lint(&["src"]);
    let parallel = fixture.lint(&["src", "-j", "4"]);
    assert_eq!(sequential.violations.len(), 8);
    assert_eq!(
        sequential.violations.iter().map(|v| v.id.clone()).collect::<Vec<_>>(),
        parallel.violations.iter().map(|v| v.id.clone()).collect::<Vec<_>>(),
        "results must be ordered independently of the thread count"
    );
}
