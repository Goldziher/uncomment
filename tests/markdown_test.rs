//! Markdown as a built-in language.
//!
//! The markdown grammar emits no `comment` node at all: an HTML comment surfaces as an
//! `html_block`, which is CommonMark-correct — an HTML comment is raw HTML in markdown, not a
//! comment in the markdown language. But `html_block` also spans real embedded HTML (`<div>`,
//! `<details>`, an `<img>` badge row), so the node kind alone cannot decide. The language handler
//! carries a content predicate: an `html_block` is a comment only when it is *nothing but* an HTML
//! comment.
//!
//! These tests pin both halves — the comments that must go, and the far larger set of
//! `html_block`s that must not be touched.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;
use uncomment::languages::config::{CommentSyntax, CommentSyntaxResolution};
use uncomment::languages::registry::LanguageRegistry;

/// A fixture repository. `.git/HEAD` makes the temporary directory a repository root, which is what
/// bounds config discovery.
fn repo(files: &[(&str, &str)]) -> TempDir {
    let temp = TempDir::new().expect("temp dir");
    let entries = [(".git/HEAD", "ref: refs/heads/markdown\n")];
    for (relative, content) in entries.iter().chain(files.iter()) {
        let path = temp.path().join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(&path, content).expect("write fixture file");
    }
    temp
}

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_uncomment"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run uncomment")
}

fn run_ok(dir: &Path, args: &[&str]) -> Output {
    let output = run(dir, args);
    assert!(
        output.status.success(),
        "uncomment {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

/// Process one markdown document through the real binary and return what is left of it.
fn uncommented(source: &str) -> String {
    let temp = repo(&[("doc.md", source)]);
    run_ok(temp.path(), &["doc.md"]);
    fs::read_to_string(temp.path().join("doc.md")).expect("read back")
}

/// Assert `source` survives a run byte for byte.
fn assert_untouched(label: &str, source: &str) {
    let after = uncommented(source);
    assert_eq!(
        after, source,
        "{label}: markdown was modified but should have been left alone"
    );
}

#[test]
fn markdown_is_detected_and_configured() {
    let registry = LanguageRegistry::new();

    let config = registry
        .detect_language(Path::new("README.md"))
        .expect("README.md detects as a language");
    assert_eq!(config.name, "markdown");
    assert!(
        config.is_comment_type("html_block"),
        "html_block is the node an HTML comment surfaces as"
    );
    assert!(
        config.get_doc_comment_types().is_empty(),
        "markdown has no documentation-comment form"
    );
    assert_eq!(
        config.resolve_comment_syntax(),
        CommentSyntaxResolution::Resolved(CommentSyntax::MARKUP),
        "`<!-- -->` is the only comment form markdown has"
    );

    for name in ["doc.markdown", "doc.mdown", "doc.mkd", "DOC.MD"] {
        assert_eq!(
            registry
                .detect_language(Path::new(name))
                .map(|config| config.name.as_str()),
            Some("markdown"),
            "{name} should detect as markdown"
        );
    }
}

#[test]
fn a_standalone_html_comment_is_removed() {
    let after = uncommented("# Title\n\n<!-- an aside -->\n\nBody text.\n");
    assert!(!after.contains("an aside"), "comment survived: {after:?}");
    assert!(after.contains("# Title"), "heading was lost: {after:?}");
    assert!(after.contains("Body text."), "body was lost: {after:?}");
}

#[test]
fn a_multi_line_html_comment_is_removed() {
    let after = uncommented("# Title\n\n<!--\nfirst line\nsecond line\n-->\n\nBody text.\n");
    assert!(!after.contains("first line"), "comment survived: {after:?}");
    assert!(!after.contains("second line"), "comment survived: {after:?}");
    assert!(after.contains("Body text."), "body was lost: {after:?}");
}

#[test]
fn an_indented_html_comment_is_removed() {
    let after = uncommented("# Title\n\n   <!-- indented three spaces -->\n\nBody text.\n");
    assert!(!after.contains("indented three spaces"), "comment survived: {after:?}");
}

/// Removing a comment must not also consume the blank line that separates two blocks. The
/// `html_block` node swallows its own trailing newline, so a span taken verbatim from the node plus
/// the usual whole-line expansion would delete *two* newlines and silently merge the blocks around
/// it — two paragraphs becoming one, or a trailing paragraph being absorbed into a list.
#[test]
fn removal_does_not_merge_the_blocks_around_the_comment() {
    let after = uncommented("First paragraph.\n<!-- an aside -->\n\nSecond paragraph.\n");
    assert!(!after.contains("an aside"), "comment survived: {after:?}");
    assert!(
        after.contains("First paragraph.\n\nSecond paragraph.") || after.contains("First paragraph.\n\n\nSecond"),
        "the blank line between the two paragraphs was eaten: {after:?}"
    );

    let after = uncommented("- item one\n- item two\n  <!-- an aside -->\n\nTrailing paragraph.\n");
    assert!(!after.contains("an aside"), "comment survived: {after:?}");
    assert!(
        !after.contains("- item two\nTrailing paragraph."),
        "the blank line after the list was eaten, absorbing the paragraph into the list: {after:?}"
    );
}

/// Every one of these is an `html_block`, and none of them is a comment.
#[test]
fn embedded_html_is_never_touched() {
    assert_untouched(
        "centred div with an image",
        "# Title\n\n<div align=\"center\">\n  <img src=\"logo.png\" alt=\"logo\">\n</div>\n\nBody.\n",
    );
    assert_untouched(
        "collapsible section",
        "<details>\n<summary>Click me</summary>\n\nHidden body.\n\n</details>\n",
    );
    assert_untouched("line break", "line one<br>line two\n");
    assert_untouched("doctype", "<!DOCTYPE html>\n\nBody.\n");
    assert_untouched("table with html", "<table>\n<tr><td>a</td></tr>\n</table>\n");
}

/// An `html_block` can hold a comment *and* other HTML: CommonMark ends an HTML comment block at
/// the line carrying `-->`, so anything after it on that line joins the same node. Deleting the
/// node would delete that HTML, so such a block is left alone entirely.
#[test]
fn a_comment_sharing_its_block_with_other_html_is_left_alone() {
    assert_untouched("comment then html, same line", "<!-- an aside --><div>kept</div>\n");
    assert_untouched("comment then text, same line", "<!-- an aside --> trailing text\n");
    assert_untouched("two comments, same line", "<!-- first --> <!-- second -->\n");
    assert_untouched("html first, comment after", "<div>\n</div>\n<!-- an aside -->\n");
    assert_untouched("comment inside a div", "<div>\n<!-- an aside -->\n</div>\n");
}

/// An unterminated `<!--` opens an HTML block that runs to the end of the document, so its node
/// spans every following line. Removing it would delete the rest of the file.
#[test]
fn an_unterminated_comment_is_left_alone() {
    assert_untouched(
        "unterminated",
        "# Title\n\n<!-- never closed\n\nReal content.\n\nMore real content.\n",
    );
}

/// A `<!-- -->` inside a fence is example text, not a comment — the grammar models it as
/// `code_fence_content`, and nothing may touch it.
#[test]
fn a_comment_inside_a_code_block_is_left_alone() {
    assert_untouched(
        "fenced",
        "Text.\n\n```html\n<!-- example comment -->\n```\n\nMore text.\n",
    );
    assert_untouched(
        "fenced, no language",
        "Text.\n\n```\n<!-- example comment -->\n```\n\nMore text.\n",
    );
    assert_untouched("indented", "Text.\n\n    <!-- example comment -->\n\nMore text.\n");
}

/// A comment written mid-paragraph is inline content, not an `html_block`, so it is invisible to
/// the tool. Pinned as the known limitation it is rather than left to surprise someone.
#[test]
fn an_inline_comment_inside_a_paragraph_is_not_reached() {
    assert_untouched("inline", "Some text <!-- inline aside --> more text.\n");
    assert_untouched("table cell", "| a | b |\n|---|---|\n| <!-- x --> | y |\n");
}

#[test]
fn a_keep_marker_inside_the_comment_preserves_it() {
    assert_untouched("own marker", "# Title\n\n<!-- ~keep an aside -->\n\nBody.\n");
}

/// `uncomment keep` writes `<!-- ~keep -->` on the line above, because markdown has no line-comment
/// form to append to. A following real run must then leave the comment it protects.
#[test]
fn keep_marker_round_trips() {
    let source = "# Title\n\n<!-- an aside -->\n\nBody.\n";
    let temp = repo(&[("doc.md", source)]);

    let scanned = run_ok(temp.path(), &["scan", "doc.md"]);
    let scanned = String::from_utf8_lossy(&scanned.stdout);
    assert!(
        scanned.contains("an aside"),
        "scan did not report the markdown comment: {scanned}"
    );

    run_ok(temp.path(), &["keep", "doc.md", "--all-removable"]);
    let marked = fs::read_to_string(temp.path().join("doc.md")).expect("read back");
    assert!(
        marked.contains("<!-- ~keep -->"),
        "keep did not write a markdown marker line: {marked:?}"
    );
    assert!(marked.contains("an aside"), "keep dropped the comment: {marked:?}");

    run_ok(temp.path(), &["doc.md"]);
    let after = fs::read_to_string(temp.path().join("doc.md")).expect("read back");
    assert_eq!(after, marked, "a real run removed a ~keep-marked comment: {after:?}");
}

#[test]
fn crlf_line_endings_survive() {
    let after = uncommented("# Title\r\n\r\n<!-- an aside -->\r\n\r\nBody.\r\n");
    assert!(!after.contains("an aside"), "comment survived: {after:?}");
    assert!(
        !after.bytes().any(|byte| byte == b'\n') || after.contains("\r\n"),
        "CRLF was lost: {after:?}"
    );
    assert!(after.contains("Body."), "body was lost: {after:?}");
}

/// Directory collection has to reach `.md` files, not just an explicitly named one.
#[test]
fn markdown_files_are_collected_from_a_directory() {
    let temp = repo(&[
        ("docs/one.md", "<!-- first aside -->\n\nOne.\n"),
        ("docs/two.md", "<!-- second aside -->\n\nTwo.\n"),
        ("docs/keepme.md", "<div>kept</div>\n"),
    ]);

    run_ok(temp.path(), &["docs"]);

    assert_eq!(
        fs::read_to_string(temp.path().join("docs/one.md")).expect("read"),
        "\nOne.\n"
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("docs/two.md")).expect("read"),
        "\nTwo.\n"
    );
    assert_eq!(
        fs::read_to_string(temp.path().join("docs/keepme.md")).expect("read"),
        "<div>kept</div>\n"
    );
}
