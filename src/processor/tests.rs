use super::*;
use crate::config::{Config, ConfigManager, ResolvedConfig};
use crate::languages::config::LanguageConfig;
use tempfile::tempdir;

fn default_resolved_config() -> ResolvedConfig {
    ResolvedConfig {
        remove_todos: false,
        remove_fixme: false,
        remove_docs: false,
        preserve_patterns: Vec::new(),
        use_default_ignores: true,
        respect_gitignore: true,
        traverse_git_repos: false,
        language_config: None,
    }
}

fn process_rust(source: &str) -> String {
    let mut processor = Processor::new();
    let language_config = LanguageConfig::rust();
    let resolved_config = default_resolved_config();
    let ProcessOutcome { content: output, .. } = processor
        .process_content_with_config(source, &language_config, &resolved_config)
        .expect("processing rust source");
    output
}

#[test]
fn plan_removals_reports_removable_comments_with_ranges() {
    let source = "// remove me\nfn main() {\n    let x = 1; // trailing\n    // TODO: keep\n    // ~keep\n}\n";
    let mut processor = Processor::new();
    let removals = processor
        .plan_removals(source, std::path::Path::new("sample.rs"), &default_resolved_config())
        .expect("plan removals");
    // TODO (remove_todos=false) and ~keep are preserved; two comments remain.
    let previews: Vec<&str> = removals.iter().map(|removal| removal.preview.as_str()).collect();
    assert_eq!(previews, vec!["// remove me", "// trailing"]);
    assert_eq!(removals[0].remove_start, 0);
    assert_eq!(
        &source[removals[0].remove_start..removals[0].remove_end],
        "// remove me\n"
    );
    assert_eq!(&source[removals[1].remove_start..removals[1].remove_end], "// trailing");
}

fn process_rust_with(source: &str, remove_docs: bool) -> ProcessOutcome {
    let mut processor = Processor::new();
    let language_config = LanguageConfig::rust();
    let resolved_config = ResolvedConfig {
        remove_docs,
        ..default_resolved_config()
    };
    processor
        .process_content_with_config(source, &language_config, &resolved_config)
        .expect("processing rust source")
}

#[test]
fn above_line_marker_preserves_doc_comment_under_remove_doc() {
    let source = "// ~keep\n/// Parent element ID.\npub fn a() {}\n";
    let outcome = process_rust_with(source, true);
    assert!(
        outcome.content.contains("/// Parent element ID."),
        "above-line marker protects the doc comment: {}",
        outcome.content
    );
    assert!(outcome.content.contains("// ~keep"), "the marker itself survives");
}

#[test]
fn above_line_marker_does_not_reach_past_a_blank_line() {
    let source = "// ~keep\n\n/// Unrelated doc.\npub fn a() {}\n";
    let outcome = process_rust_with(source, true);
    assert!(
        !outcome.content.contains("/// Unrelated doc."),
        "a blank line ends the run: {}",
        outcome.content
    );
}

#[test]
fn above_line_marker_does_not_reach_past_code() {
    // The `///` node swallows its trailing newline, so row adjacency alone
    // would make the comment after `pub fn a()` look like a neighbour.
    let source = "// ~keep\n/// Protected doc.\npub fn a() {}\n// unrelated removable\npub fn b() {}\n";
    let outcome = process_rust_with(source, true);
    assert!(outcome.content.contains("/// Protected doc."), "target kept");
    assert!(
        !outcome.content.contains("// unrelated removable"),
        "code between comments ends the run: {}",
        outcome.content
    );
}

#[test]
fn redundant_keep_marker_is_stripped_from_doc_comment() {
    let source = "/// Parent element ID. ~keep Resolves elsewhere.\npub fn a() {}\n";
    let outcome = process_rust_with(source, false);
    assert!(
        outcome.content.contains("/// Parent element ID. Resolves elsewhere."),
        "marker stripped and spacing collapsed: {}",
        outcome.content
    );
    assert_eq!(outcome.redundant_markers.len(), 1, "one marker reported");
    assert_eq!(outcome.redundant_markers[0].line, 0);
}

#[test]
fn trailing_redundant_marker_takes_the_space_before_it() {
    let source = "/// Parent element ID. ~keep\npub fn a() {}\n";
    let outcome = process_rust_with(source, false);
    assert!(
        outcome.content.contains("/// Parent element ID.\n"),
        "no trailing space left behind: {}",
        outcome.content
    );
}

#[test]
fn load_bearing_keep_marker_survives_under_remove_doc() {
    let source = "/// Parent element ID. ~keep\npub fn a() {}\n";
    let outcome = process_rust_with(source, true);
    assert!(
        outcome.content.contains("~keep"),
        "the marker is the only thing preserving this doc comment: {}",
        outcome.content
    );
    assert!(outcome.redundant_markers.is_empty(), "nothing reported as redundant");
}

#[test]
fn keep_marker_on_a_line_comment_is_never_stripped() {
    let source = "// rationale ~keep\npub fn a() {}\n";
    let outcome = process_rust_with(source, false);
    assert!(
        outcome.content.contains("// rationale ~keep"),
        "line comments do not render into docs, so the marker stays: {}",
        outcome.content
    );
    assert!(outcome.redundant_markers.is_empty());
}

#[test]
fn nested_comment_nodes_are_counted_once() {
    // Rust records each `///` line twice: the outer `line_comment` and the
    // inner doc node. Both describe one comment.
    let source = "/// Doc one.\npub fn a() {}\n\n/// Doc two.\npub fn b() {}\n";
    let outcome = process_rust_with(source, true);
    assert_eq!(outcome.removed_comments.len(), 2, "two doc comments, not four nodes");
    let spans: Vec<(usize, usize)> = outcome
        .removed_comments
        .iter()
        .map(|comment| (comment.start_row, comment.end_row))
        .collect();
    assert_eq!(spans, vec![(0, 1), (3, 4)], "no duplicated ranges");
}

#[test]
fn prose_about_the_marker_is_left_alone() {
    let cases = [
        "/// Comments containing `~keep` are preserved.\npub fn a() {}\n",
        "/// Write `/// ~keep Parent element ID.` to protect it.\npub fn a() {}\n",
        "/// ```text\n/// // ~keep\n/// ```\npub fn a() {}\n",
        "/// A ~keepsake is not a marker.\npub fn a() {}\n",
    ];
    for source in cases {
        let outcome = process_rust_with(source, false);
        assert_eq!(
            outcome.content, source,
            "documentation discussing the marker must survive intact: {source}"
        );
        assert!(outcome.redundant_markers.is_empty(), "nothing reported for: {source}");
    }
}

#[test]
fn keep_marker_preserves_whole_contiguous_line_comment_block() {
    let source = "fn f() {\n    // line one\n    // line two\n    // line three ~keep\n    let x = 1;\n}\n";
    let output = process_rust(source);
    assert!(output.contains("// line one"), "first block line kept: {output}");
    assert!(output.contains("// line two"), "middle block line kept: {output}");
    assert!(output.contains("// line three ~keep"), "marked line kept: {output}");
}

#[test]
fn keep_marker_on_first_line_preserves_block() {
    let source = "fn f() {\n    // one ~keep\n    // two\n    // three\n    let x = 1;\n}\n";
    let output = process_rust(source);
    assert!(output.contains("// one ~keep"), "marked line kept: {output}");
    assert!(output.contains("// two"), "following line kept: {output}");
    assert!(output.contains("// three"), "following line kept: {output}");
}

#[test]
fn blank_line_breaks_keep_block() {
    let source =
        "fn f() {\n    // kept ~keep\n    // kept two\n\n    // dropped one\n    // dropped two\n    let x = 1;\n}\n";
    let output = process_rust(source);
    assert!(output.contains("// kept ~keep"), "marked line kept: {output}");
    assert!(output.contains("// kept two"), "same-block line kept: {output}");
    assert!(
        !output.contains("// dropped one"),
        "separate paragraph stripped: {output}"
    );
    assert!(
        !output.contains("// dropped two"),
        "separate paragraph stripped: {output}"
    );
}

#[test]
fn trailing_keep_does_not_extend_to_standalone_neighbor() {
    let source = "fn f() {\n    let x = 1; // trailing ~keep\n    // standalone removable\n    let y = 2;\n}\n";
    let output = process_rust(source);
    assert!(
        output.contains("// trailing ~keep"),
        "trailing keep preserved per-comment: {output}"
    );
    assert!(
        !output.contains("// standalone removable"),
        "a trailing keep must not anchor a block: {output}"
    );
}

#[test]
fn code_between_comments_breaks_keep_block() {
    let source = "fn f() {\n    // block a ~keep\n    let x = 1;\n    // block b removable\n    let y = 2;\n}\n";
    let output = process_rust(source);
    assert!(output.contains("// block a ~keep"), "marked line kept: {output}");
    assert!(
        !output.contains("// block b removable"),
        "code between comments ends the block: {output}"
    );
}

#[test]
fn merge_ranges_combines_touching_and_overlapping() {
    assert_eq!(merge_ranges(&[(0, 5), (5, 10)]), vec![(0, 10)], "touching ranges merge");
    assert_eq!(
        merge_ranges(&[(0, 5), (6, 10)]),
        vec![(0, 5), (6, 10)],
        "disjoint stay split"
    );
    assert_eq!(merge_ranges(&[(0, 7), (3, 10)]), vec![(0, 10)], "overlapping merge");
    assert_eq!(
        merge_ranges(&[(6, 10), (0, 5)]),
        vec![(0, 5), (6, 10)],
        "unsorted input is sorted"
    );
    assert_eq!(merge_ranges(&[]), Vec::<(usize, usize)>::new(), "empty input");
}

#[test]
fn cut_ranges_removes_only_covered_bytes() {
    let content = "abcdefghij";
    assert_eq!(cut_ranges(content, 0, 10, &[(3, 6)]), "abcghij", "range mid-window");
    assert_eq!(
        cut_ranges(content, 2, 8, &[(2, 4)]),
        "efgh",
        "range flush to window start"
    );
    assert_eq!(
        cut_ranges(content, 2, 8, &[(6, 8)]),
        "cdef",
        "range flush to window end"
    );
    assert_eq!(cut_ranges(content, 2, 8, &[(0, 10)]), "", "range covers whole window");
    assert_eq!(
        cut_ranges(content, 2, 5, &[(6, 9)]),
        "cde",
        "range outside window is ignored"
    );
    assert_eq!(
        cut_ranges(content, 0, 5, &[(3, 3)]),
        "abcde",
        "zero-length range is a no-op"
    );
}

#[test]
fn records_removed_comment_locations_and_previews() {
    let source = "// standalone\nfn main() {\n    let x = 1; // trailing\n    /* block\n       two */\n}\n";
    let mut processor = Processor::new();
    let language_config = LanguageConfig::rust();
    let outcome = processor
        .process_content_with_config(source, &language_config, &default_resolved_config())
        .expect("processing rust source");

    let spans: Vec<(usize, usize)> = outcome
        .removed_comments
        .iter()
        .map(|comment| (comment.start_row, comment.end_row))
        .collect();
    assert_eq!(spans, vec![(0, 0), (2, 2), (3, 4)]);

    let previews: Vec<&str> = outcome
        .removed_comments
        .iter()
        .map(|comment| comment.preview.as_str())
        .collect();
    assert_eq!(previews, vec!["// standalone", "// trailing", "/* block"]);

    assert!(outcome.removed_comments.iter().all(|comment| !comment.is_documentation));
    assert_eq!(outcome.removed_comments.len(), 3);
}

#[test]
fn plan_removals_preserves_python_docstrings_by_default() {
    let source = "def f():\n    \"\"\"docstring\"\"\"\n    # remove me\n    return 1\n";
    let mut processor = Processor::new();
    let removals = processor
        .plan_removals(source, std::path::Path::new("module.py"), &default_resolved_config())
        .expect("plan removals");
    let previews: Vec<&str> = removals.iter().map(|removal| removal.preview.as_str()).collect();
    assert_eq!(previews, vec!["# remove me"]);
}

#[test]
fn plan_removals_unsupported_extension_errors() {
    let mut processor = Processor::new();
    let result = processor.plan_removals(
        "noop",
        std::path::Path::new("file.unknownext"),
        &default_resolved_config(),
    );
    assert!(result.is_err());
}

fn process_go(source: &str, use_default_ignores: bool, remove_docs: bool) -> String {
    let mut processor = Processor::new();
    let language_config = LanguageConfig::go();
    let mut resolved_config = default_resolved_config();
    resolved_config.use_default_ignores = use_default_ignores;
    resolved_config.remove_docs = remove_docs;
    let ProcessOutcome { content: output, .. } = processor
        .process_content_with_config(source, &language_config, &resolved_config)
        .expect("processing go source");
    output
}

fn process_language(source: &str, language_config: LanguageConfig) -> String {
    let mut processor = Processor::new();
    let resolved_config = default_resolved_config();
    let ProcessOutcome { content: output, .. } = processor
        .process_content_with_config(source, &language_config, &resolved_config)
        .expect("processing source");
    output
}

fn process_language_with_default_ignores(
    source: &str,
    language_config: LanguageConfig,
    use_default_ignores: bool,
) -> String {
    let mut processor = Processor::new();
    let mut resolved_config = default_resolved_config();
    resolved_config.use_default_ignores = use_default_ignores;
    let ProcessOutcome { content: output, .. } = processor
        .process_content_with_config(source, &language_config, &resolved_config)
        .expect("processing source");
    output
}

#[test]
fn preserves_strings_matching_comment_text() {
    let source = r#"fn main() {
    let pattern = "// comment";
    println!("{}", pattern); // comment
}
"#;

    let processed = process_rust(source);

    assert!(processed.contains("\"// comment\""));
    assert!(!processed.contains("; // comment"));
}

#[test]
fn preserves_macro_invocations_with_comment_like_strings() {
    let source = r#"macro_rules! announce {
    ($msg:expr) => {{
        println!("{}", $msg); // keep
    }};
}

fn main() {
    announce!("// keep");
}
"#;

    let processed = process_rust(source);

    assert!(processed.contains("announce!(\"// keep\");"));
    assert!(!processed.contains("// keep\n"));
}

#[test]
fn preserves_attributes_when_removing_doc_comments() {
    let source = r#"#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Create smart config
    #[command(about = "Create a template configuration file")]
    Init,
}
"#;

    let mut processor = Processor::new();
    let language_config = LanguageConfig::rust();
    let mut config = default_resolved_config();
    config.remove_docs = true;

    let ProcessOutcome { content: processed, .. } = processor
        .process_content_with_config(source, &language_config, &config)
        .expect("process doc comments");

    assert!(processed.contains("#[command(about = \"Create a template configuration file\")]"));
    assert!(!processed.contains("Create smart config"));
}

#[test]
fn respects_no_default_ignores_override() {
    let dir = tempdir().expect("create temp dir");
    let file_path = dir.path().join("sample.rs");
    let source = r#"/// #![feature(never_type)]
// NOTE: this would normally be preserved
fn main() {}
"#;

    std::fs::write(&file_path, source).expect("write test file");

    let config_manager = ConfigManager::from_single_config(dir.path(), Config::default()).expect("config manager");

    let mut processor = Processor::new();

    let overrides_with_defaults = ProcessingOptions {
        remove_todo: true,
        remove_fixme: true,
        remove_doc: true,
        custom_preserve_patterns: Vec::new(),
        use_default_ignores: true,
        dry_run: true,
        show_diff: false,
        respect_gitignore: true,
        traverse_git_repos: false,
    };

    let with_defaults = processor
        .process_file_with_config(&file_path, &config_manager, Some(&overrides_with_defaults))
        .expect("process with defaults");
    assert!(with_defaults.processed_content.contains("NOTE"));
    assert!(with_defaults.processed_content.contains("#![feature"));

    let overrides_without_defaults = ProcessingOptions {
        use_default_ignores: false,
        ..overrides_with_defaults
    };

    let without_defaults = processor
        .process_file_with_config(&file_path, &config_manager, Some(&overrides_without_defaults))
        .expect("process without defaults");
    assert!(!without_defaults.processed_content.contains("NOTE"));
    assert!(!without_defaults.processed_content.contains("#![feature"));
    assert!(without_defaults.processed_content.contains("fn main()"));
}

#[test]
fn honors_config_file_disabling_default_ignores() {
    // Regression for #106: a config with use_default_ignores = false must be honored
    // when --no-default-ignores was NOT passed. Previously the CLI default clobbered it,
    // leaving hardcoded NOTE/HACK patterns unremovable.
    let dir = tempdir().expect("create temp dir");
    let file_path = dir.path().join("sample.lua");
    std::fs::write(&file_path, "-- NOTE: remove me\n-- HACK: me too\nlocal x = 1\n").expect("write test file");

    let mut config = Config::default();
    config.global.use_default_ignores = false;
    let config_manager = ConfigManager::from_single_config(dir.path(), config).expect("config manager");

    // CLI defaults: use_default_ignores = true because --no-default-ignores absent.
    let overrides = ProcessingOptions {
        remove_todo: false,
        remove_fixme: false,
        remove_doc: false,
        custom_preserve_patterns: Vec::new(),
        use_default_ignores: true,
        dry_run: true,
        show_diff: false,
        respect_gitignore: true,
        traverse_git_repos: false,
    };

    let mut processor = Processor::new();
    let result = processor
        .process_file_with_config(&file_path, &config_manager, Some(&overrides))
        .expect("process lua file");

    assert!(!result.processed_content.contains("NOTE"));
    assert!(!result.processed_content.contains("HACK"));
    assert!(result.processed_content.contains("local x = 1"));
}

#[test]
fn preserves_go_embed_directives_even_without_default_ignores() {
    let source = r#"package main

//go:embed hello.txt
var embedded string

func main() { /* regular comment should be removed */ }
"#;

    let processed = process_go(source, false, true);
    assert!(processed.contains("//go:embed hello.txt"));
    assert!(!processed.contains("regular comment should be removed"));
}

#[test]
fn preserves_go_cgo_preamble_comments() {
    let source = r#"package htmltomarkdown

// #cgo LDFLAGS: -lhtml_to_markdown_ffi
// #include <stdlib.h>
// extern const char* html_to_markdown_version();
import "C"

func Version() string { return C.GoString(C.html_to_markdown_version()) /* regular comment should be removed */ }
"#;

    for use_default_ignores in [true, false] {
        let processed = process_go(source, use_default_ignores, true);
        assert!(
            processed.contains("// #cgo LDFLAGS: -lhtml_to_markdown_ffi"),
            "expected to preserve cgo preamble with use_default_ignores={use_default_ignores}"
        );
        assert!(
            processed.contains("// #include <stdlib.h>"),
            "expected to preserve cgo preamble with use_default_ignores={use_default_ignores}"
        );
        assert!(
            processed.contains("// extern const char* html_to_markdown_version();"),
            "expected to preserve cgo preamble with use_default_ignores={use_default_ignores}"
        );
        assert!(processed.contains("import \"C\""));
        assert!(!processed.contains("regular comment should be removed"));
    }
}

#[test]
fn removes_ruby_comments_without_touching_strings() {
    let source = r#"# remove me
puts "Hello # not a comment"
"#;

    let processed = process_language(source, LanguageConfig::ruby());
    assert!(!processed.contains("# remove me"));
    assert!(processed.contains("Hello # not a comment"));
}

#[test]
fn preserves_ruby_frozen_string_literal_magic_comment() {
    let source = r#"# frozen_string_literal: true
# remove me
puts "ok"
"#;

    let processed = process_language(source, LanguageConfig::ruby());
    assert!(processed.contains("# frozen_string_literal: true"));
    assert!(!processed.contains("# remove me"));
}

#[test]
fn preserves_shebangs_even_without_default_ignores() {
    let source = r#"#!/usr/bin/env bash
# remove me
echo "ok"
"#;

    let processed = process_language_with_default_ignores(source, LanguageConfig::shell(), false);

    assert!(processed.starts_with("#!/usr/bin/env bash\n"));
    assert!(!processed.contains("# remove me"));
    assert!(processed.contains("echo \"ok\""));
}

#[test]
fn preserves_ruby_yard_doc_comments_by_default() {
    let source = r#"# @param x [Integer]
def foo(x)
  x + 1
end
"#;

    let processed = process_language(source, LanguageConfig::ruby());
    assert!(processed.contains("# @param x [Integer]"));
}

#[test]
fn removes_php_comments_without_touching_strings() {
    let source = r#"<?php
// remove me
$s = "// not a comment";
echo $s;
"#;

    let processed = process_language(source, LanguageConfig::php());
    assert!(!processed.contains("// remove me"));
    assert!(processed.contains("\"// not a comment\""));
}

#[test]
fn preserves_c_header_guard_trailing_comments() {
    let source = r#"#ifndef HTML_TO_MARKDOWN_H
#define HTML_TO_MARKDOWN_H

// remove me
int x;

#endif  /* HTML_TO_MARKDOWN_H */
"#;

    let processed = process_language(source, LanguageConfig::c());
    assert!(processed.contains("#endif  /* HTML_TO_MARKDOWN_H */"));
    assert!(!processed.contains("remove me"));
    assert!(processed.contains("int x;"));
}

#[test]
fn removes_elixir_comments_without_touching_strings() {
    let source = r##"# remove me
IO.puts("# not a comment")
"##;

    let processed = process_language(source, LanguageConfig::elixir());
    assert!(!processed.contains("# remove me"));
    assert!(processed.contains("\"# not a comment\""));
}

#[test]
fn removes_toml_comments_without_touching_strings() {
    let source = r##"# remove me
key = "# not a comment"
"##;

    let processed = process_language(source, LanguageConfig::toml());
    assert!(!processed.contains("# remove me"));
    assert!(processed.contains("\"# not a comment\""));
}

#[test]
fn removes_csharp_comments_without_touching_strings() {
    let source = r#"// remove me
class C { void M() { var s = "// not a comment"; } }
"#;

    let processed = process_language(source, LanguageConfig::csharp());
    assert!(!processed.contains("// remove me"));
    assert!(processed.contains("\"// not a comment\""));
}

#[test]
fn removes_haskell_comments_without_touching_strings() {
    let source = r#"-- remove me
main = putStrLn "-- not a comment"
"#;

    let processed = process_language(source, LanguageConfig::haskell());
    assert!(!processed.contains("-- remove me"));
    assert!(processed.contains("\"-- not a comment\""));
}

#[test]
fn removes_html_comments_without_touching_content() {
    let source = r#"<!-- remove me -->
<div>Hello</div>
"#;

    let processed = process_language(source, LanguageConfig::html());
    assert!(!processed.contains("remove me"));
    assert!(processed.contains("<div>Hello</div>"));
}

#[test]
fn removes_css_comments_without_touching_strings() {
    let source = r#"/* remove me */
.a::before { content: "/* not a comment */"; }
"#;

    let processed = process_language(source, LanguageConfig::css());
    assert!(!processed.contains("remove me"));
    assert!(processed.contains("\"/* not a comment */\""));
}

#[test]
fn removes_xml_comments_without_touching_text() {
    let source = r#"<!-- remove me -->
<root>hello</root>
"#;

    let processed = process_language(source, LanguageConfig::xml());
    assert!(!processed.contains("remove me"));
    assert!(processed.contains("<root>hello</root>"));
}

#[test]
fn removes_sql_comments_without_touching_strings() {
    let source = r#"-- remove me
SELECT '-- not a comment' as val;
"#;

    let processed = process_language(source, LanguageConfig::sql());
    assert!(!processed.contains("-- remove me"));
    assert!(processed.contains("'-- not a comment'"));
}

#[test]
fn removes_kotlin_comments_without_touching_strings() {
    let source = r#"// remove me
fun main() { val s = "// not a comment" }
"#;

    let processed = process_language(source, LanguageConfig::kotlin());
    assert!(!processed.contains("// remove me"));
    assert!(processed.contains("\"// not a comment\""));
}

#[test]
fn removes_swift_comments_without_touching_strings() {
    let source = r#"// remove me
let s = "// not a comment"
"#;

    let processed = process_language(source, LanguageConfig::swift());
    assert!(!processed.contains("// remove me"));
    assert!(processed.contains("\"// not a comment\""));
}

#[test]
fn removes_objc_comments_without_touching_strings() {
    let source = r#"// remove me
NSString *s = @"// not a comment";
"#;

    let processed = process_language(source, LanguageConfig::objc());
    assert!(!processed.contains("// remove me"));
    assert!(processed.contains("@\"// not a comment\""));
}

#[test]
fn preserves_objc_preprocessor_trailing_comments() {
    let source = r#"#import "Local.h" // fallback
#define kTimeout 30 // seconds
// remove me
NSString *s = @"// not a comment";
"#;

    let processed = process_language(source, LanguageConfig::objc());
    assert!(processed.contains("#import \"Local.h\" // fallback"));
    assert!(processed.contains("#define kTimeout 30 // seconds"));
    assert!(!processed.contains("remove me"));
    assert!(processed.contains("@\"// not a comment\""));
}

#[test]
fn removes_lua_comments_without_touching_strings() {
    let source = r#"-- remove me
local s = "-- not a comment"
"#;

    let processed = process_language(source, LanguageConfig::lua());
    assert!(!processed.contains("-- remove me"));
    assert!(processed.contains("\"-- not a comment\""));
}

#[test]
fn removes_nix_comments_without_touching_strings() {
    let source = r##"# remove me
let s = "# not a comment"; in s
"##;

    let processed = process_language(source, LanguageConfig::nix());
    assert!(!processed.contains("# remove me"));
    assert!(processed.contains("\"# not a comment\""));
}

#[test]
fn removes_powershell_comments_without_touching_strings() {
    let source = r##"# remove me
$s = "# not a comment"
Write-Output $s
"##;

    let processed = process_language(source, LanguageConfig::powershell());
    assert!(!processed.contains("# remove me"));
    assert!(processed.contains("\"# not a comment\""));
}

#[test]
fn removes_proto_comments_without_touching_strings() {
    let source = r#"// remove me
syntax = "proto3";
message A { string s = 1 [default = "// not a comment"]; }
"#;

    let processed = process_language(source, LanguageConfig::proto());
    assert!(!processed.contains("// remove me"));
    assert!(processed.contains("\"// not a comment\""));
}

#[test]
fn removes_ini_comments_without_touching_values() {
    let source = r#"; remove me
[section]
key = # not a comment
"#;

    let processed = process_language(source, LanguageConfig::ini());
    assert!(!processed.contains("; remove me"));
    assert!(processed.contains("key = # not a comment"));
}

#[test]
fn removes_python_docstrings_when_remove_docs_enabled() {
    let source = r#""""This is a docstring"""
# TODO: regular todo
# mypy: ignore
def hello(): pass"#;

    let mut processor = Processor::new();
    let language_config = LanguageConfig::python();
    let mut resolved_config = default_resolved_config();
    resolved_config.remove_docs = true;

    let ProcessOutcome { content: output, .. } = processor
        .process_content_with_config(source, &language_config, &resolved_config)
        .expect("processing python source");

    assert!(
        !output.contains("This is a docstring"),
        "Python docstring should be removed when remove_docs=true"
    );
    assert!(output.contains("TODO: regular todo"), "TODO should be preserved");
    assert!(output.contains("mypy: ignore"), "mypy should be preserved");
}

#[test]
fn handles_utf8_multibyte_in_comments() {
    let source = "// Comment with emoji 🎉\nfn main() {}\n";

    let processed = process_rust(source);
    assert!(!processed.contains("🎉"));
    assert!(processed.contains("fn main()"));
}

#[test]
fn handles_file_with_only_comments() {
    let source = "// Only comments\n// Nothing else\n";

    let processed = process_rust(source);
    assert!(processed.trim().is_empty());
}

#[test]
fn handles_empty_file() {
    let source = "";

    let mut processor = Processor::new();
    let language_config = LanguageConfig::rust();
    let resolved_config = default_resolved_config();
    let outcome = processor
        .process_content_with_config(source, &language_config, &resolved_config)
        .expect("processing empty source");
    assert_eq!(outcome.content, "");
    assert_eq!(outcome.removed_comments.len(), 0);
}

#[test]
fn handles_comment_at_end_of_file_no_trailing_newline() {
    let source = "fn main() {} // trailing";

    let processed = process_rust(source);
    assert!(!processed.contains("// trailing"));
    assert!(processed.contains("fn main()"));
}

/// Sources covering the comment shapes that behave differently from each other:
/// Rust `///` and `//!` (recorded as nested node pairs) plus a `/** */` block,
/// Python docstrings (`string` nodes, not comments), JSDoc, and a `#`-comment
/// language with a shebang.
fn inventory_cases() -> Vec<(&'static str, LanguageConfig, &'static str)> {
    vec![
        (
            "sample.rs",
            LanguageConfig::rust(),
            "//! Crate entry point.\n\n/// Documented.\npub fn a() {}\n\n/** Block doc. */\npub fn b() {}\n\n// plain removable\npub fn c() {\n    let x = 1; // trailing removable\n    /* block\n       removable */\n}\n",
        ),
        (
            "module.py",
            LanguageConfig::python(),
            "\"\"\"Module docstring.\"\"\"\n\n\ndef f():\n    \"\"\"Function docstring.\"\"\"\n    # remove me\n    return 1  # trailing removable\n",
        ),
        (
            "app.js",
            LanguageConfig::javascript(),
            "/** JSDoc summary. */\nfunction f() {\n  // remove me\n  return 1; /* block removable */\n}\n",
        ),
        (
            "script.sh",
            LanguageConfig::shell(),
            "#!/usr/bin/env bash\n# remove me\necho \"ok\"  # trailing removable\n",
        ),
    ]
}

/// Apply a removal plan to `source` the way a host tool would, so the plan can be
/// compared against what the rewriting path actually produces.
fn apply_removals(source: &str, removals: &[Removal]) -> String {
    let merged = merge_ranges(
        &removals
            .iter()
            .map(|removal| (removal.remove_start, removal.remove_end))
            .collect::<Vec<_>>(),
    );
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;
    for (start, end) in merged {
        if cursor < start {
            output.push_str(&source[cursor..start]);
        }
        cursor = cursor.max(end);
    }
    output.push_str(&source[cursor..]);
    output
}

#[test]
fn plan_removals_is_inspect_filtered_to_remove_verdicts() {
    for (path, _, source) in inventory_cases() {
        let mut processor = Processor::new();
        let config = default_resolved_config();

        let expected: Vec<Removal> = processor
            .inspect(source, Path::new(path), &config)
            .expect("inspect")
            .into_iter()
            .filter_map(|comment| match comment.verdict {
                Verdict::Remove {
                    expanded_start,
                    expanded_end,
                } => Some(Removal {
                    comment_start: comment.start_byte,
                    comment_end: comment.end_byte,
                    remove_start: expanded_start,
                    remove_end: expanded_end,
                    start_row: comment.start_row,
                    is_documentation: comment.is_documentation,
                    preview: first_line_preview(&comment.text),
                }),
                Verdict::Preserve => None,
            })
            .collect();

        let removals = processor
            .plan_removals(source, Path::new(path), &config)
            .expect("plan removals");
        assert_eq!(removals, expected, "plan_removals diverged from inspect for {path}");
        assert!(
            removals.len() >= 2,
            "{path} must exercise several removals, got {removals:?}"
        );
    }
}

#[test]
fn inspect_removals_reproduce_the_rewritten_source() {
    for (path, language_config, source) in inventory_cases() {
        let mut processor = Processor::new();
        let removals = processor
            .plan_removals(source, Path::new(path), &default_resolved_config())
            .expect("plan removals");
        assert_eq!(
            apply_removals(source, &removals),
            process_language(source, language_config),
            "the inventory's removals disagree with the rewriting path for {path}"
        );
    }
}

#[test]
fn inspect_is_ordered_and_every_preserved_comment_names_a_reason() {
    for (path, _, source) in inventory_cases() {
        let mut processor = Processor::new();
        let inspected = processor
            .inspect(source, Path::new(path), &default_resolved_config())
            .expect("inspect");

        assert!(
            inspected
                .windows(2)
                .all(|pair| pair[0].start_byte <= pair[1].start_byte),
            "{path} is not ordered by start_byte: {inspected:?}"
        );
        assert!(
            inspected.iter().any(|comment| comment.verdict == Verdict::Preserve),
            "{path} must exercise at least one preserved comment"
        );
        for comment in &inspected {
            match comment.verdict {
                Verdict::Preserve => assert!(
                    comment.reason.is_some(),
                    "preserved comment has no reason in {path}: {comment:?}"
                ),
                Verdict::Remove { .. } => assert!(
                    comment.reason.is_none(),
                    "removed comment carries a reason in {path}: {comment:?}"
                ),
            }
            assert_eq!(
                comment.text,
                &source[comment.start_byte..comment.end_byte],
                "text does not match its byte range in {path}"
            );
        }
    }
}

#[test]
fn inspect_reports_one_entry_per_comment_for_nested_node_pairs() {
    // Rust records each `///` line twice, as the outer `line_comment` and as the
    // inner doc node; the inventory is per comment, not per node.
    let source = "/// Doc one.\npub fn a() {}\n\n/// Doc two.\npub fn b() {}\n";
    let mut processor = Processor::new();
    let inspected = processor
        .inspect(source, Path::new("sample.rs"), &default_resolved_config())
        .expect("inspect");
    assert_eq!(inspected.len(), 2, "two doc comments, not four nodes: {inspected:?}");
    assert!(
        inspected
            .iter()
            .all(|comment| comment.kind == CommentKind::Doc && comment.reason == Some(PreserveReason::Documentation))
    );
}

#[test]
fn inspect_classifies_comment_shapes() {
    let source = "//! Crate docs.\n\n/// Documented.\npub fn a() {}\n\n/** Block doc. */\npub fn b() {}\n\n// plain\npub fn c() {}\n\n/* block\n   comment */\npub fn d() {}\n";
    let mut processor = Processor::new();
    let inspected = processor
        .inspect(source, Path::new("sample.rs"), &default_resolved_config())
        .expect("inspect");
    let shapes: Vec<(CommentKind, &str)> = inspected
        .iter()
        .map(|comment| (comment.kind, comment.text.lines().next().unwrap_or_default()))
        .collect();
    assert_eq!(
        shapes,
        vec![
            (CommentKind::Doc, "//! Crate docs."),
            (CommentKind::Doc, "/// Documented."),
            (CommentKind::Doc, "/** Block doc. */"),
            (CommentKind::Line, "// plain"),
            (CommentKind::Block, "/* block"),
        ]
    );

    let python = "\"\"\"Module docstring.\"\"\"\n# plain\n";
    let inspected = processor
        .inspect(python, Path::new("module.py"), &default_resolved_config())
        .expect("inspect python");
    assert_eq!(inspected[0].kind, CommentKind::Docstring);
    assert_eq!(inspected[1].kind, CommentKind::Line);
}

#[test]
fn above_line_marker_separates_the_marker_from_what_it_extends_over() {
    let source = "// ~keep\n/// Parent element ID.\npub fn a() {}\n";
    let config = ResolvedConfig {
        remove_docs: true,
        ..default_resolved_config()
    };
    let mut processor = Processor::new();
    let inspected = processor
        .inspect(source, Path::new("sample.rs"), &config)
        .expect("inspect");

    let reasons: Vec<(&str, Option<&PreserveReason>)> = inspected
        .iter()
        .map(|comment| (comment.text.lines().next().unwrap_or_default(), comment.reason.as_ref()))
        .collect();
    assert_eq!(
        reasons,
        vec![
            ("// ~keep", Some(&PreserveReason::KeepMarker)),
            (
                "/// Parent element ID.",
                Some(&PreserveReason::ExtendedByNeighbourMarker)
            ),
        ]
    );
}

#[test]
fn keep_block_members_are_attributed_to_the_marker_they_borrow() {
    // The `// TODO` line has a reason of its own even though the marker also covers
    // it, so `keep` must not read it as marker-dependent.
    let source = "fn f() {\n    // TODO: later\n    // context line\n    // rationale ~keep\n    let x = 1;\n}\n";
    let mut processor = Processor::new();
    let inspected = processor
        .inspect(source, Path::new("sample.rs"), &default_resolved_config())
        .expect("inspect");

    let reasons: Vec<Option<&PreserveReason>> = inspected.iter().map(|comment| comment.reason.as_ref()).collect();
    assert_eq!(
        reasons,
        vec![
            Some(&PreserveReason::Pattern("TODO".to_string())),
            Some(&PreserveReason::ExtendedByNeighbourMarker),
            Some(&PreserveReason::KeepMarker),
        ]
    );
}

#[test]
fn inspect_names_shebang_and_pattern_reasons() {
    let source = "#!/usr/bin/env bash\n# NOTE: load bearing\n# remove me\necho ok\n";
    let mut processor = Processor::new();
    let inspected = processor
        .inspect(source, Path::new("script.sh"), &default_resolved_config())
        .expect("inspect");
    assert_eq!(inspected[0].reason, Some(PreserveReason::Shebang));
    assert_eq!(inspected[1].reason, Some(PreserveReason::Pattern("NOTE".to_string())));
    assert!(matches!(inspected[2].verdict, Verdict::Remove { .. }));
}

#[test]
fn grammar_forced_preservation_is_named_a_language_directive() {
    // The trailing `/* GUARD */` survives because the C handler recognises a
    // comment trailing a preprocessor line, not because any pattern matched it.
    let source = "#ifndef GUARD\n#define GUARD\n// remove me\nint x;\n#endif  /* GUARD */\n";
    let mut processor = Processor::new();
    let inspected = processor
        .inspect(source, Path::new("header.h"), &default_resolved_config())
        .expect("inspect");
    let guard = inspected
        .iter()
        .find(|comment| comment.text.contains("GUARD */"))
        .expect("guard comment inspected");
    assert_eq!(guard.reason, Some(PreserveReason::LanguageDirective));
    assert_eq!(guard.verdict, Verdict::Preserve);
}

#[test]
fn inspect_expanded_range_swallows_a_standalone_comment_line() {
    let source = "// standalone\nfn main() {\n    let x = 1; // trailing\n}\n";
    let mut processor = Processor::new();
    let inspected = processor
        .inspect(source, Path::new("sample.rs"), &default_resolved_config())
        .expect("inspect");
    let ranges: Vec<&str> = inspected
        .iter()
        .filter_map(|comment| match comment.verdict {
            Verdict::Remove {
                expanded_start,
                expanded_end,
            } => Some(&source[expanded_start..expanded_end]),
            Verdict::Preserve => None,
        })
        .collect();
    assert_eq!(ranges, vec!["// standalone\n", "// trailing"]);
}

#[test]
fn inspect_unsupported_extension_errors() {
    let mut processor = Processor::new();
    let result = processor.inspect("noop", Path::new("file.unknownext"), &default_resolved_config());
    assert!(result.is_err());
}
