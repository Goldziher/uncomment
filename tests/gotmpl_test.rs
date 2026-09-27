//! Go template (`gotmpl`) support, and the empty-action hazard specific to it.
//!
//! The `gotmpl` grammar models a comment action as three *bare siblings* — `{{`, `comment`, `}}` —
//! rather than as one node spanning the delimiters the way `if_action`, `range_action` and
//! `define_action` all do. Deleting only the `comment` node therefore leaves `{{}}`, which the
//! grammar itself rejects (it parses as an `ERROR`) and which makes Helm fail to render the chart.
//! Removal has to widen to the whole action.
//!
//! Filed upstream as ngalaiko/tree-sitter-go-template#56.

use std::path::Path;
use std::process::Command;
use tempfile::TempDir;
use uncomment::languages::LanguageRegistry;

const BINARY: &str = env!("CARGO_BIN_EXE_uncomment");

/// Writes `source` to a file with the given name, runs the binary over it, and returns the rewritten
/// text. `extra` carries any additional flags.
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

/// Convenience wrapper for the overwhelmingly common case: a Helm `.tpl` file, no extra flags.
fn uncomment_tpl(source: &str) -> String {
    uncomment_file("chart.tpl", source, &[])
}

/// Counts `ERROR` and missing nodes the `gotmpl` grammar reports for `source`. Zero means the text
/// is syntactically valid Go template; anything else means a rewrite corrupted it.
fn parse_faults(source: &str) -> usize {
    let language = tree_sitter_language_pack::get_language("gotmpl").expect("gotmpl grammar");
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language).expect("set language");
    let tree = parser.parse(source, None).expect("parse");

    let mut faults = 0;
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        if node.is_error() || node.is_missing() {
            faults += 1;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
    faults
}

/// `.tpl` is the extension Helm uses for its chart helper files, so it is the one that matters most
/// here; the other three are the names Go's own tooling and the grammar itself use.
#[test]
fn go_template_extensions_are_detected() {
    let registry = LanguageRegistry::new();

    for extension in ["tpl", "tmpl", "gotmpl", "gohtml"] {
        let detected = registry.detect_language_by_extension(extension);
        assert_eq!(
            detected.map(|config| config.name.as_str()),
            Some("gotmpl"),
            ".{extension} should be detected as gotmpl"
        );
    }

    let detected = registry.detect_language(Path::new("templates/_helpers.tpl"));
    assert_eq!(detected.map(|config| config.name.as_str()), Some("gotmpl"));
}

/// The bug in one assertion: the empty action must never survive a removal.
#[test]
fn removing_a_comment_action_never_leaves_an_empty_action() {
    let result = uncomment_tpl("{{/* a comment */}}\n");

    assert!(
        !result.contains("{{}}"),
        "removal left an empty action, which Helm cannot render: {result:?}"
    );
    assert_eq!(
        parse_faults(&result),
        0,
        "rewritten template does not parse: {result:?}"
    );
}

/// A comment action alone on its line takes the whole line with it, exactly as a standalone comment
/// does in every other language.
#[test]
fn a_standalone_comment_action_removes_its_whole_line() {
    let result = uncomment_tpl("line one\n{{/* a comment */}}\nline two\n");

    assert_eq!(result, "line one\nline two\n");
    assert_eq!(parse_faults(&result), 0);
}

/// Whitespace-trim markers are separate tokens in this grammar — `{{-` (which absorbs the space that
/// must follow it) and `-}}` — so the widened span has to reach for the delimiter kinds rather than
/// assume a fixed two-character `{{`.
#[test]
fn a_trimmed_comment_action_is_removed_whole() {
    for source in [
        "line one\n{{- /* a comment */ -}}\nline two\n",
        "line one\n{{- /* a comment */}}\nline two\n",
        "line one\n{{/* a comment */ -}}\nline two\n",
    ] {
        let result = uncomment_tpl(source);
        assert_eq!(result, "line one\nline two\n", "source: {source:?}");
        assert!(!result.contains("{{}}"));
        assert_eq!(parse_faults(&result), 0);
    }
}

/// A comment action that sits inline in text leaves the text on either side of it alone. Two spaces
/// remain, because `uncomment` only widens a removal to line bounds when the comment is alone on its
/// line — the same rule every other language gets, and the only one that cannot eat a space that
/// carries meaning in rendered output.
#[test]
fn an_inline_comment_action_leaves_the_surrounding_text() {
    let result = uncomment_tpl("hello {{/* x */}} world\n");

    assert_eq!(result, "hello  world\n");
    assert_eq!(parse_faults(&result), 0);
}

/// Most real Helm comments live inside a `define` block, where the comment's siblings are the inner
/// action's delimiters and its parent is `define_action` rather than the root.
#[test]
fn a_comment_action_nested_in_a_define_block_is_removed_whole() {
    let source = "{{- define \"chart.name\" -}}\n{{/* pick a name */}}\n{{- .Release.Name -}}\n{{- end -}}\n";
    let result = uncomment_tpl(source);

    assert_eq!(
        result,
        "{{- define \"chart.name\" -}}\n{{- .Release.Name -}}\n{{- end -}}\n"
    );
    assert!(!result.contains("{{}}"));
    assert_eq!(parse_faults(&result), 0);
}

/// Widening is for an action the comment has to itself. A comment that shares its action with
/// anything else keeps the delimiters and its neighbour: only the comment text goes.
#[test]
fn a_comment_sharing_its_action_is_not_widened() {
    let result = uncomment_tpl("{{/* c */ if .A}}x{{end}}\n");

    assert!(
        result.contains("if .A"),
        "the action's own content was swallowed: {result:?}"
    );
    assert!(
        result.starts_with("{{"),
        "the opening delimiter was swallowed: {result:?}"
    );
    assert_eq!(result, "{{ if .A}}x{{end}}\n");
    assert_eq!(parse_faults(&result), 0);
}

/// A `~keep` marker still protects the comment, and the action around it stays intact — a widened
/// span must not become a widened *deletion* for a comment that is being preserved.
#[test]
fn a_keep_marked_comment_action_is_preserved_intact() {
    let source = "line one\n{{/* load-bearing ~keep */}}\nline two\n";
    let result = uncomment_tpl(source);

    assert_eq!(result, source);
    assert_eq!(parse_faults(&result), 0);
}

/// A multi-line comment is one `comment` node between one pair of delimiters, so the same widening
/// applies and the whole block goes.
#[test]
fn a_multiline_comment_action_is_removed_whole() {
    let result = uncomment_tpl("line one\n{{/*\nmulti\nline\n*/}}\nline two\n");

    assert_eq!(result, "line one\nline two\n");
    assert!(!result.contains("{{}}"));
    assert_eq!(parse_faults(&result), 0);
}

/// Two comment actions on one line are two separate actions; each widens to its own delimiters and
/// neither reaches into the other.
#[test]
fn adjacent_comment_actions_are_each_removed_whole() {
    let result = uncomment_tpl("a{{/* one */}}b{{/* two */}}c\n");

    assert_eq!(result, "abc\n");
    assert!(!result.contains("{{}}"));
    assert_eq!(parse_faults(&result), 0);
}

/// Running twice changes nothing the second time.
#[test]
fn a_second_pass_is_a_no_op() {
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("chart.tpl");
    let source = "{{- define \"chart.labels\" -}}\n{{/* labels */}}\napp: x\n{{- end -}}\n";
    std::fs::write(&path, source).expect("write fixture");

    let mut passes = Vec::new();
    for _ in 0..2 {
        let output = Command::new(BINARY)
            .arg(&path)
            .env("NO_COLOR", "1")
            .output()
            .expect("run uncomment");
        assert!(output.status.success());
        passes.push(std::fs::read_to_string(&path).expect("read result"));
    }

    assert_eq!(passes[0], passes[1], "second pass changed the file");
    assert_eq!(parse_faults(&passes[1]), 0);
}

/// A realistic helper file, to check the whole set of shapes together rather than one at a time.
#[test]
fn a_realistic_helper_file_still_parses_after_removal() {
    let source = r#"{{/* Chart helpers. */}}
{{- define "chart.name" -}}
{{- /* Truncate to the 63-char label limit. */ -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "chart.labels" -}}
{{/*
  Labels applied to every object.
*/}}
app.kubernetes.io/name: {{ include "chart.name" . }}
{{- end -}}
"#;

    let result = uncomment_tpl(source);

    assert!(!result.contains("{{}}"), "empty action in output: {result:?}");
    assert!(!result.contains("/*"), "a comment survived: {result:?}");
    assert_eq!(parse_faults(&result), 0, "output does not parse: {result:?}");
    assert!(result.contains(r#"{{- define "chart.name" -}}"#));
    assert!(result.contains("app.kubernetes.io/name:"));
}
