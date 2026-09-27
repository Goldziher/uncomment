//! Removing a trailing comment must take the whitespace that separated it from the code.
//!
//! Leaving that run behind turns every rewritten line into a trailing-whitespace violation, which
//! `buildifier` and the standard pre-commit hooks reject — a clean run would hand back a dirty tree.
//! The separator only exists to hold the comment off the code, so it goes with the comment. Nothing
//! here is language-specific: the expansion happens on byte ranges, so every language is covered.

use std::process::Command;
use tempfile::TempDir;

const BINARY: &str = env!("CARGO_BIN_EXE_uncomment");

/// Writes `source` to a file named `name`, runs the binary over it, and returns the rewritten text.
fn uncomment_file(name: &str, source: &str) -> String {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join(name);
    std::fs::write(&path, source).expect("write fixture");

    let output = Command::new(BINARY)
        .arg(&path)
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

/// The motivating case: a Bazel `BUILD` file, whose trailing comments are aligned with spaces.
#[test]
fn a_trailing_comment_in_a_build_file_takes_the_spaces_before_it() {
    let result = uncomment_file("BUILD", "py_library(\n    name = \"z\",  # why\n)\n");

    assert_eq!(result, "py_library(\n    name = \"z\",\n)\n");
}

#[test]
fn a_trailing_python_comment_takes_the_spaces_before_it() {
    let result = uncomment_file("module.py", "name = \"z\"  # why\n");

    assert_eq!(result, "name = \"z\"\n");
}

#[test]
fn a_trailing_hcl_comment_takes_the_spaces_before_it() {
    let result = uncomment_file("main.tf", "resource \"x\" \"y\" {\n  count = 1  # why\n}\n");

    assert_eq!(result, "resource \"x\" \"y\" {\n  count = 1\n}\n");
}

#[test]
fn a_tab_separator_goes_too() {
    let result = uncomment_file("module.py", "name = \"z\"\t# why\n");

    assert_eq!(result, "name = \"z\"\n");
}

#[test]
fn a_mixed_run_of_spaces_and_tabs_goes_whole() {
    let result = uncomment_file("module.py", "name = \"z\" \t \t# why\n");

    assert_eq!(result, "name = \"z\"\n");
}

/// The newline is the line boundary, not part of the separator: swallowing it would splice the line
/// onto the next one.
#[test]
fn the_newline_after_a_trailing_comment_survives() {
    let result = uncomment_file("module.py", "a = 1  # why\nb = 2\n");

    assert_eq!(result, "a = 1\nb = 2\n");
    assert!(result.contains("a = 1\nb"), "the two statements stay on separate lines");
}

/// A trailing block comment is the same shape as a trailing line comment — the code is on one side
/// only — so the separator goes for it too.
#[test]
fn a_trailing_block_comment_takes_the_spaces_before_it() {
    let result = uncomment_file("sample.rs", "fn main() {\n    let x = 1;  /* why */\n}\n");

    assert_eq!(result, "fn main() {\n    let x = 1;\n}\n");
}

/// A trailing comment on an unterminated last line has no newline to preserve.
#[test]
fn a_trailing_comment_at_end_of_file_without_a_newline_takes_its_separator() {
    let result = uncomment_file("sample.rs", "fn main() {}  // why");

    assert_eq!(result, "fn main() {}");
}

/// A comment alone on its line already loses the whole line, indentation included. That path must
/// keep behaving the same way.
#[test]
fn a_standalone_comment_still_loses_its_whole_line() {
    let result = uncomment_file("module.py", "a = 1\n    # why\nb = 2\n");

    assert_eq!(result, "a = 1\nb = 2\n");
}

/// Code on *both* sides makes the comment inline, not trailing: the whitespace before it separates
/// the comment from the code on the left, but the whitespace after it separates that same code from
/// the code on the right, and collapsing either changes the line. Inline comments keep both runs.
#[test]
fn an_inline_comment_between_code_keeps_the_whitespace_on_both_sides() {
    let result = uncomment_file("sample.c", "int main(void) {\n  return /* why */ 0;\n}\n");

    assert_eq!(result, "int main(void) {\n  return  0;\n}\n");
}

/// Indentation before an inline comment is the code's own indentation, not a separator — the
/// comment is leading the code that follows it on the line, so nothing before it is removed.
#[test]
fn indentation_before_an_inline_comment_is_left_alone() {
    let result = uncomment_file("sample.c", "int main(void) {\n  /* why */ return 0;\n}\n");

    assert_eq!(result, "int main(void) {\n   return 0;\n}\n");
}

/// The whole point: a rewrite introduces no trailing whitespace on any line it touched.
#[test]
fn a_rewritten_file_has_no_trailing_whitespace() {
    let source = "py_library(\n    name = \"z\",  # why\n    srcs = [\"a.py\"],\t# also why\n)\n";
    let result = uncomment_file("BUILD.bazel", source);

    let offenders: Vec<&str> = result
        .lines()
        .filter(|line| line.ends_with(' ') || line.ends_with('\t'))
        .collect();
    assert!(offenders.is_empty(), "trailing whitespace left behind: {offenders:?}");
}
