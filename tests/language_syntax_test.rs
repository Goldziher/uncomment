//! Literal comment-syntax coverage for every registered language.
//!
//! The `keep` subcommand writes a marker line above a comment it cannot append to
//! in-body, so it needs each language's literal delimiters. Two properties have to
//! hold for that marker to work at all: every language the registry knows must
//! resolve to a syntax entry, and no line token may be a documentation form — a
//! `/// ~keep` marker is classified as documentation and silently does nothing.

use std::collections::BTreeSet;

use uncomment::ast::visitor::CommentInfo;
use uncomment::languages::config::{CommentSyntax, CommentSyntaxResolution, LanguageConfig};
use uncomment::languages::registry::LanguageRegistry;
use uncomment::rules::preservation::PreservationRule;

/// Languages whose only comment form is a block pair, so a marker line cannot be
/// written with a line token. Adding a language here is a deliberate statement
/// that the language has no line-comment form at all.
const NO_LINE_COMMENT: &[&str] = &["css", "html", "json", "ocaml", "svelte", "vue", "xml"];

/// Languages with no comment syntax whatsoever.
const NO_COMMENT_SYNTAX: &[&str] = &["json"];

fn resolve(config: &LanguageConfig) -> CommentSyntax {
    match config.resolve_comment_syntax() {
        CommentSyntaxResolution::Resolved(syntax) => syntax,
        CommentSyntaxResolution::Unknown => {
            panic!(
                "language `{}` resolves to Unknown; add a comment_syntax entry",
                config.name
            )
        }
    }
}

fn syntax_for_extension(registry: &LanguageRegistry, extension: &str) -> CommentSyntax {
    let config = registry
        .detect_language_by_extension(extension)
        .unwrap_or_else(|| panic!("no language registered for extension `{extension}`"));
    resolve(config)
}

fn comment(node_type: &str, content: &str) -> (CommentInfo, String) {
    let info = CommentInfo {
        start_byte: 0,
        end_byte: content.len(),
        start_row: 42,
        end_row: 42,
        node_type: node_type.to_string(),
        should_preserve: false,
        is_documentation: false,
    };
    (info, content.to_string())
}

#[test]
fn every_registered_language_resolves_to_a_syntax_entry() {
    let registry = LanguageRegistry::new();
    let mut checked = 0;

    for (name, config) in registry.get_all_languages() {
        let syntax = resolve(config);
        if syntax.line.is_none() && syntax.block.is_none() {
            assert!(
                NO_COMMENT_SYNTAX.contains(&name.as_str()),
                "language `{name}` resolves to no comment syntax at all but is not in NO_COMMENT_SYNTAX"
            );
        }
        checked += 1;
    }

    assert!(
        checked >= 51,
        "expected the registry to cover at least 51 languages, saw {checked}"
    );
}

#[test]
fn languages_without_a_line_comment_are_exactly_the_documented_set() {
    let registry = LanguageRegistry::new();

    let observed: BTreeSet<String> = registry
        .get_all_languages()
        .filter(|(_, config)| resolve(config).line.is_none())
        .map(|(name, _)| name.clone())
        .collect();
    let expected: BTreeSet<String> = NO_LINE_COMMENT.iter().map(|&name| name.to_string()).collect();

    assert_eq!(
        observed, expected,
        "the set of languages with no line-comment token drifted; a new language must either \
         declare a line token or be added to NO_LINE_COMMENT deliberately"
    );
}

#[test]
fn no_line_comment_token_is_classified_as_documentation() {
    let registry = LanguageRegistry::new();
    let rule = PreservationRule::Documentation;

    for (name, config) in registry.get_all_languages() {
        let Some(line) = resolve(config).line else {
            continue;
        };

        let marker = format!("{line} ~keep");
        let (info, content) = comment("comment", &marker);
        assert!(
            !rule.matches(&info, &content),
            "language `{name}`: marker `{marker}` is classified as documentation, so the marker \
             would be inert"
        );

        let bare = line.to_string();
        let (info, content) = comment("comment", &bare);
        assert!(
            !rule.matches(&info, &content),
            "language `{name}`: bare line token `{bare}` is a documentation form"
        );
    }
}

#[test]
fn no_block_open_delimiter_is_classified_as_documentation() {
    let registry = LanguageRegistry::new();
    let rule = PreservationRule::Documentation;

    for (name, config) in registry.get_all_languages() {
        let Some((open, close)) = resolve(config).block else {
            continue;
        };

        let marker = format!("{open} ~keep {close}");
        let (info, content) = comment("comment", &marker);
        assert!(
            !rule.matches(&info, &content),
            "language `{name}`: block marker `{marker}` is classified as documentation"
        );
    }
}

#[test]
fn spot_check_line_tokens_by_extension() {
    let registry = LanguageRegistry::new();

    assert_eq!(syntax_for_extension(&registry, "rs").line, Some("//"));
    assert_eq!(syntax_for_extension(&registry, "py").line, Some("#"));
    assert_eq!(syntax_for_extension(&registry, "sql").line, Some("--"));
    assert_eq!(syntax_for_extension(&registry, "lua").line, Some("--"));
    assert_eq!(syntax_for_extension(&registry, "clj").line, Some(";"));
    assert_eq!(syntax_for_extension(&registry, "tex").line, Some("%"));
    assert_eq!(syntax_for_extension(&registry, "erl").line, Some("%"));
}

#[test]
fn spot_check_block_only_languages_by_extension() {
    let registry = LanguageRegistry::new();

    let html = syntax_for_extension(&registry, "html");
    assert_eq!(html.line, None);
    assert_eq!(html.block, Some(("<!--", "-->")));

    let css = syntax_for_extension(&registry, "css");
    assert_eq!(css.line, None);
    assert_eq!(css.block, Some(("/*", "*/")));

    let ocaml = syntax_for_extension(&registry, "ml");
    assert_eq!(ocaml.line, None);
    assert_eq!(ocaml.block, Some(("(*", "*)")));
}

#[test]
fn json_declares_no_comment_syntax_but_jsonc_does() {
    let registry = LanguageRegistry::new();

    let json = syntax_for_extension(&registry, "json");
    assert_eq!(json.line, None);
    assert_eq!(json.block, None);

    let jsonc = syntax_for_extension(&registry, "jsonc");
    assert_eq!(jsonc.line, Some("//"));
    assert_eq!(jsonc.block, Some(("/*", "*/")));
}

#[test]
fn rust_marker_is_the_plain_line_form_not_the_doc_form() {
    let registry = LanguageRegistry::new();
    let rust = syntax_for_extension(&registry, "rs");

    assert_eq!(rust.line, Some("//"));
    assert_ne!(rust.line, Some("///"));
    assert_ne!(rust.line, Some("//!"));
}

#[test]
fn a_language_with_no_declared_syntax_and_no_family_resolves_to_unknown() {
    let config = LanguageConfig::new("mystery", vec!["mys"], vec!["comment"], vec![], "mystery-lang");

    assert_eq!(config.resolve_comment_syntax(), CommentSyntaxResolution::Unknown);
}

#[test]
fn a_language_with_no_declared_syntax_falls_back_to_its_tree_sitter_family() {
    let config = LanguageConfig::new("my-rust-dialect", vec!["myrs"], vec!["line_comment"], vec![], "rust");

    assert_eq!(
        config.resolve_comment_syntax(),
        CommentSyntaxResolution::Resolved(CommentSyntax::C_STYLE)
    );
}

#[test]
fn configured_languages_inherit_comment_syntax_from_the_builtin_they_override() {
    use std::collections::HashMap;
    use uncomment::config::LanguageConfig as UserLanguageConfig;

    let mut registry = LanguageRegistry::new();
    let mut languages = HashMap::new();
    languages.insert(
        "python".to_string(),
        UserLanguageConfig {
            name: "python".to_string(),
            extensions: vec!["py".to_string(), "sage".to_string()],
            comment_nodes: vec!["comment".to_string()],
            doc_comment_nodes: vec![],
            preserve_patterns: vec![],
            remove_todos: None,
            remove_fixme: None,
            remove_docs: None,
            use_default_ignores: None,
        },
    );

    registry.register_configured_languages(&languages);

    assert_eq!(syntax_for_extension(&registry, "sage").line, Some("#"));
}
