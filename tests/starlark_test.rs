//! Starlark docstrings.
//!
//! Bazel's build language borrows Python's documentation form: a `.bzl` module or function documents
//! itself with a leading string expression, not with a comment, and Stardoc reads exactly that
//! string. The Starlark grammar models it the way the Python grammar does — `module >
//! expression_statement > string`, and `block > expression_statement > string` under a
//! `function_definition` — so [`uncomment::languages::handlers::PythonHandler`] is the classifier
//! Starlark needs too.
//!
//! Without it a `.bzl` docstring is invisible: `--remove-doc` walks past it, and `scan` never
//! reports it. Declaring `string` as a doc-comment kind *without* the classifier would be worse than
//! the gap — every string literal in a `BUILD` file would become a comment the default run deletes —
//! so the two halves only make sense together, and the tests below pin both: the docstrings are
//! seen, and nothing else is.

use std::process::Command;
use tempfile::TempDir;
use uncomment::languages::handlers::get_handler;

const BINARY: &str = env!("CARGO_BIN_EXE_uncomment");

/// A `.bzl` module in the shape Bazel rule files actually take: a leading provenance comment, a
/// module docstring after it, a macro with an `Args:` docstring, and strings in every position that
/// is *not* a docstring — a named constant, a bare string that is not the first statement, a keyword
/// argument, and a multi-line `cmd`.
const RULE_FILE: &str = r#"# Adapted from rules_foo, with local changes.

"""The build rules for widget programs."""

load("@rules_python//python:defs.bzl", "py_library")

ARM_FLAGS = """
CC=aarch64-linux-gnu-gcc
"""

"""Not a docstring: the module already had one, and this is not the first statement."""

def widget_program(name, src, **kwargs):
    """Generates a widget object file from source.

    Args:
      name: target name for the widget program.
      src: widget source.
    """

    # The `.` directory is the project root.
    cmd = """
clang -O2 -o $@ $(location {src})
"""

    native.genrule(
        name = name,
        srcs = [src],
        cmd = cmd,
        message = "Building widget",  # trailing comment
        **kwargs
    )

def undocumented(name):
    x = "assignment first"
    """Not a docstring: a statement came before it."""
    native.filegroup(name = name)
"#;

/// Writes `source` to `name` in a fresh directory, runs the binary over it with `extra` flags, and
/// returns the rewritten file.
fn uncomment_file(name: &str, source: &str, extra: &[&str]) -> String {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join(name);
    std::fs::write(&path, source).expect("write fixture");

    let output = Command::new(BINARY)
        .arg(&path)
        .args(extra)
        .env("NO_COLOR", "1")
        .output()
        .expect("run uncomment");
    assert!(
        output.status.success(),
        "uncomment failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    std::fs::read_to_string(&path).expect("read result")
}

/// The JSONL `scan` writes for `source`, one line per comment.
fn scan_file(name: &str, source: &str) -> String {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join(name);
    std::fs::write(&path, source).expect("write fixture");

    let output = Command::new(BINARY)
        .args(["scan"])
        .arg(&path)
        .env("NO_COLOR", "1")
        .output()
        .expect("run uncomment scan");
    assert!(
        output.status.success(),
        "scan failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Every `string` node in `source` as parsed by the Starlark grammar, paired with the verdict the
/// language handler reaches for it: `Some(true)` for a docstring, `Some(false)` for a string that
/// merely happens to be one, `None` for no opinion.
fn docstring_verdicts(language: &str, source: &str) -> Vec<(usize, Option<bool>)> {
    let grammar = tree_sitter_language_pack::get_language(language).expect("grammar");
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&grammar).expect("set language");
    let tree = parser.parse(source, None).expect("parse");
    let handler = get_handler(language);

    let mut verdicts = Vec::new();
    let mut stack = vec![(tree.root_node(), None)];
    while let Some((node, parent)) = stack.pop() {
        if node.kind() == "string" {
            verdicts.push((
                node.start_position().row + 1,
                handler.is_documentation_comment(&node, parent, source),
            ));
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push((child, Some(node)));
        }
    }
    verdicts.sort_unstable();
    verdicts
}

/// The grammar claim this whole change rests on, asserted directly against a parse tree: the handler
/// says "documentation" for the two docstrings in [`RULE_FILE`] — the module's on line 3, the
/// macro's on line 14 — and "not documentation" for every other string in the file.
#[test]
fn only_a_string_in_docstring_position_is_documentation() {
    let verdicts = docstring_verdicts("starlark", RULE_FILE);

    let documentation: Vec<usize> = verdicts
        .iter()
        .filter(|(_, verdict)| *verdict == Some(true))
        .map(|(row, _)| *row)
        .collect();
    assert_eq!(documentation, vec![3, 14], "verdicts were {verdicts:?}");
    assert!(
        verdicts
            .iter()
            .all(|(_, verdict)| matches!(verdict, Some(true) | Some(false))),
        "the handler has to reach a verdict on every string, or a non-docstring is collected as a \
         comment: {verdicts:?}"
    );
}

/// A Starlark docstring is documentation, so it survives a default run — and it has to survive as
/// its own bytes, since Stardoc publishes them.
#[test]
fn a_default_run_keeps_docstrings_and_removes_comments() {
    let result = uncomment_file("rules.bzl", RULE_FILE, &[]);

    assert!(result.contains(r#""""The build rules for widget programs.""""#));
    assert!(result.contains("Generates a widget object file from source."));
    assert!(result.contains("      src: widget source."));
    assert!(!result.contains("# Adapted from rules_foo"));
    assert!(!result.contains("directory is the project root"));
    assert!(!result.contains("# trailing comment"));
}

/// `--remove-doc` removes documentation, and a `.bzl` docstring is the documentation this language
/// has. Leaving it behind is the defect: the same source saved as `.py` loses both docstrings.
#[test]
fn remove_doc_strips_module_and_function_docstrings() {
    let result = uncomment_file("rules.bzl", RULE_FILE, &["--remove-doc"]);

    assert!(
        !result.contains("The build rules for widget programs"),
        "module docstring survived --remove-doc: {result}"
    );
    assert!(
        !result.contains("Generates a widget object file from source"),
        "function docstring survived --remove-doc: {result}"
    );
    assert!(
        !result.contains("Args:"),
        "the docstring was only partly removed: {result}"
    );
}

/// The hazard of declaring `string` a doc-comment kind: a string that is not in docstring position
/// must not be swept up, by either run. `--remove-doc` is the stricter of the two, so it is the one
/// worth pinning — a string it leaves alone the default run leaves alone too.
#[test]
fn strings_outside_docstring_position_are_untouched() {
    for flags in [vec![], vec!["--remove-doc"]] {
        let result = uncomment_file("rules.bzl", RULE_FILE, &flags);

        for survivor in [
            "CC=aarch64-linux-gnu-gcc",
            "Not a docstring: the module already had one",
            "Not a docstring: a statement came before it",
            "clang -O2 -o $@ $(location {src})",
            r#"message = "Building widget","#,
            r#"load("@rules_python//python:defs.bzl", "py_library")"#,
            r#"x = "assignment first""#,
        ] {
            assert!(
                result.contains(survivor),
                "{flags:?} removed a string that is not a docstring ({survivor:?}): {result}"
            );
        }
    }
}

/// A `BUILD` file is nothing but calls with string arguments, and it has no docstrings in practice.
/// Every string in one has to come out the far side of `--remove-doc` unchanged.
#[test]
fn a_build_file_loses_only_its_comments() {
    let source = r#"load("@rules_python//python:defs.bzl", "py_binary")

# Entry point for the widget service.
py_binary(
    name = "widget",
    srcs = ["widget.py"],
    main = "widget.py",  # explicit, because the name does not match
    visibility = ["//visibility:public"],
)
"#;
    let expected = r#"load("@rules_python//python:defs.bzl", "py_binary")

py_binary(
    name = "widget",
    srcs = ["widget.py"],
    main = "widget.py",
    visibility = ["//visibility:public"],
)
"#;

    // Compared line by line with trailing space trimmed: whether removing a trailing comment leaves
    // the space before it is a separate question from whether a string survived.
    let result = uncomment_file("BUILD", source, &["--remove-doc"]);
    let trimmed: Vec<&str> = result.lines().map(str::trim_end).collect();
    let wanted: Vec<&str> = expected.lines().map(str::trim_end).collect();
    assert_eq!(trimmed, wanted);
}

/// `scan` has to report a `.bzl` docstring, since a decision can only be taken over a comment the
/// inventory lists. It reports it as a docstring preserved as documentation, exactly as it does for
/// Python.
#[test]
fn scan_reports_a_bzl_docstring_as_documentation() {
    let output = scan_file("rules.bzl", RULE_FILE);

    let docstrings: Vec<&str> = output
        .lines()
        .filter(|line| line.contains(r#""kind":"docstring""#))
        .collect();
    assert_eq!(docstrings.len(), 2, "scan output was:\n{output}");
    for line in &docstrings {
        assert!(line.contains(r#""verdict":"preserve""#), "{line}");
        assert!(line.contains(r#""reason":"documentation""#), "{line}");
    }
}

/// A docstring's bytes *are* the published documentation, so a `~keep` marker goes on the line above
/// it — appending one in body would rewrite what Stardoc emits. The marker must also be the plain
/// `#` form, which is the only form `starlark` has.
#[test]
fn keep_marks_a_docstring_from_the_line_above() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("rules.bzl");
    std::fs::write(&path, RULE_FILE).expect("write fixture");

    let output = Command::new(BINARY)
        .args(["keep", "--match", "The build rules for widget programs"])
        .arg(&path)
        .env("NO_COLOR", "1")
        .output()
        .expect("run uncomment keep");
    assert!(
        output.status.success(),
        "keep failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let marked = std::fs::read_to_string(&path).expect("read result");
    assert!(
        marked.contains("# ~keep\n\"\"\"The build rules for widget programs.\"\"\""),
        "the marker did not land on the line above the docstring: {marked}"
    );
    assert!(
        marked.contains(r#""""The build rules for widget programs.""""#),
        "the docstring's own bytes changed: {marked}"
    );
}
