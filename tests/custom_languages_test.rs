use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use clap::Parser;
use serde_json::Value;
use tempfile::TempDir;
use uncomment::{
    config::{Config, LanguageConfig},
    lint::{LintArgs, Outcome, lint},
    processor::Processor,
};

/// Test that custom language configurations can be loaded from TOML
#[test]
fn test_custom_language_config_loading() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("uncomment.toml");

    let config_content = r#"
[global]
remove_docs = false

[languages.ruby]
name = "Ruby"
extensions = ["rb"]
comment_nodes = ["comment"]
doc_comment_nodes = ["comment"]

[languages.swift]
name = "Swift"
extensions = ["swift"]
comment_nodes = ["comment", "multiline_comment"]

[languages.vue]
name = "Vue"
extensions = ["vue"]
comment_nodes = ["comment"]
"#;

    fs::write(&config_path, config_content).unwrap();

    let config = Config::from_file(&config_path).unwrap();

    assert!(config.languages.contains_key("ruby"));
    let ruby_config = &config.languages["ruby"];
    assert_eq!(ruby_config.name, "Ruby");
    assert_eq!(ruby_config.extensions, vec!["rb"]);

    assert!(config.languages.contains_key("swift"));
    let swift_config = &config.languages["swift"];
    assert_eq!(swift_config.name, "Swift");
    assert_eq!(swift_config.extensions, vec!["swift"]);

    assert!(config.languages.contains_key("vue"));
    let vue_config = &config.languages["vue"];
    assert_eq!(vue_config.name, "Vue");
    assert_eq!(vue_config.extensions, vec!["vue"]);
}

/// Test mixed configuration with builtin and custom languages
#[test]
fn test_mixed_builtin_and_custom_languages() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("uncomment.toml");

    let config_content = r#"
[global]
remove_docs = false

# Builtin language with custom settings
[languages.rust]
name = "Rust"
extensions = ["rs"]
comment_nodes = ["line_comment", "block_comment"]
remove_docs = true

# Another builtin with overrides
[languages.kotlin]
name = "Kotlin"
extensions = ["kt", "kts"]
comment_nodes = ["line_comment", "multiline_comment"]
"#;

    fs::write(&config_path, config_content).unwrap();

    let config = Config::from_file(&config_path).unwrap();

    let rust_config = &config.languages["rust"];
    assert_eq!(rust_config.name, "Rust");
    assert_eq!(rust_config.remove_docs, Some(true));

    let kotlin_config = &config.languages["kotlin"];
    assert_eq!(kotlin_config.name, "Kotlin");
}

/// Test processor with custom language configuration overrides
#[test]
fn test_processor_with_custom_language() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("uncomment.toml");

    let config_content = r#"
[global]
remove_docs = false

[languages.elixir]
name = "Elixir"
extensions = ["ex", "exs"]
comment_nodes = ["comment"]
"#;

    fs::write(&config_path, config_content).unwrap();

    let config_manager = uncomment::config::ConfigManager::new(temp_dir.path()).unwrap();
    let mut processor = Processor::new();

    let elixir_file = temp_dir.path().join("test.ex");
    let elixir_content = r#"
# This is an Elixir comment
defmodule Example do
  # Function comment
  def hello do
    IO.puts("Hello, Elixir!")
  end
end
"#;
    fs::write(&elixir_file, elixir_content).unwrap();

    let result = processor.process_file_with_config(&elixir_file, &config_manager, None);
    assert!(result.is_ok());
    let processed = result.unwrap();
    assert!(!processed.processed_content.contains("# This is an Elixir comment"));
    assert!(processed.processed_content.contains("IO.puts"));
}

/// Test configuration with language overrides
#[test]
fn test_language_config_overrides() {
    let config_content = r#"
[global]
remove_docs = false

[languages.python]
name = "Python"
extensions = ["py"]
comment_nodes = ["comment"]

[languages.rust]
name = "Rust"
extensions = ["rs"]
comment_nodes = ["line_comment", "block_comment"]
"#;

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("uncomment.toml");
    fs::write(&config_path, config_content).unwrap();

    let config = Config::from_file(&config_path).unwrap();
    assert_eq!(config.languages.len(), 2);
}

/// Test Vue.js configuration
#[test]
fn test_vuejs_configuration() {
    let config = LanguageConfig {
        name: "Vue".to_string(),
        extensions: vec!["vue".to_string()],
        comment_nodes: vec!["comment".to_string()],
        doc_comment_nodes: vec![],
        preserve_patterns: vec!["eslint-".to_string(), "@ts-".to_string()],
        remove_todos: None,
        remove_fixme: None,
        remove_docs: None,
        use_default_ignores: None,
    };

    assert_eq!(config.name, "Vue");
    assert!(config.extensions.contains(&"vue".to_string()));
    assert!(config.comment_nodes.contains(&"comment".to_string()));
}

// --- a declared extension has to survive file collection -----------------------------------------
//
// Everything above tests that a `[languages]` section parses. None of it tests the only thing that
// makes such a section useful: that a file carrying the extension it declares is picked up. File
// collection decides that, and it decides it before any config is read, so a registry that was
// never told about the declaration discards the file and the section does nothing at all.

/// Maps an extension no built-in language claims onto the Python grammar, which is enough for a
/// `#` comment in a `.zork` file to be removed if — and only if — the declaration reached the
/// registry that collection filters with.
const ZORK: &str = r#"
[languages.python]
name = "python"
extensions = ["zork"]
comment_nodes = ["comment"]
"#;

const ZORK_LINT: &str = r#"
[lint]
enabled = true

[languages.python]
name = "python"
extensions = ["zork"]
comment_nodes = ["comment"]
"#;

#[derive(Parser, Debug)]
struct LintHarness {
    #[command(flatten)]
    lint: LintArgs,
}

fn lint_args(argv: &[&str]) -> LintArgs {
    LintHarness::parse_from(std::iter::once("lint").chain(argv.iter().copied())).lint
}

/// A fixture repository. The `.git/HEAD` makes the temporary directory a repository root, which is
/// what bounds config discovery, and gives `lint` a branch name that names no issue key.
fn repo(files: &[(&str, &str)]) -> TempDir {
    let temp = TempDir::new().expect("temp dir");
    let entries = [(".git/HEAD", "ref: refs/heads/feat/no-key-here\n")];
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

/// Every path in a `scan` report, which is the inventory's account of what it collected.
fn scanned_paths(dir: &Path, args: &[&str]) -> Vec<String> {
    let output = run(dir, args);
    assert!(
        output.status.success(),
        "scan {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).expect("report is json");
    report
        .as_array()
        .expect("the json format is an array")
        .iter()
        .filter_map(|record| record.get("path")?.as_str().map(str::to_owned))
        .collect()
}

#[test]
fn a_custom_extension_from_the_root_config_is_processed_by_a_default_run() {
    let temp = repo(&[
        (".uncommentrc.toml", ZORK),
        ("thing.zork", "# a plain comment\nx = 1\n"),
    ]);

    let output = run(temp.path(), &["."]);

    assert!(
        output.status.success(),
        "the run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let processed = fs::read_to_string(temp.path().join("thing.zork")).expect("read fixture file");
    assert!(
        !processed.contains("# a plain comment"),
        "the extension the root config declares must be collected and processed, got: {processed:?}"
    );
    assert!(processed.contains("x = 1"), "the code must survive, got: {processed:?}");
}

#[test]
fn a_custom_extension_from_a_subdirectory_config_is_processed_by_a_default_run() {
    let temp = repo(&[
        ("sub/.uncommentrc.toml", ZORK),
        ("sub/thing.zork", "# a plain comment\nx = 1\n"),
    ]);

    let output = run(temp.path(), &["."]);

    assert!(
        output.status.success(),
        "the run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let processed = fs::read_to_string(temp.path().join("sub/thing.zork")).expect("read fixture file");
    assert!(
        !processed.contains("# a plain comment"),
        "a config under the requested paths declares the extension, so the file must be processed, got: \
         {processed:?}"
    );
    assert!(processed.contains("x = 1"), "the code must survive, got: {processed:?}");
}

#[test]
fn scan_inventories_a_custom_extension_from_the_root_config() {
    let temp = repo(&[
        (".uncommentrc.toml", ZORK),
        ("thing.zork", "# a plain comment\nx = 1\n"),
    ]);

    let paths = scanned_paths(temp.path(), &["scan", ".", "--format", "json"]);

    assert!(
        paths.iter().any(|path| path.ends_with("thing.zork")),
        "scan collects with its own registry, which must know the declared extension too, got: {paths:?}"
    );
}

#[test]
fn scan_inventories_a_custom_extension_from_a_subdirectory_config() {
    let temp = repo(&[
        ("sub/.uncommentrc.toml", ZORK),
        ("sub/thing.zork", "# a plain comment\nx = 1\n"),
    ]);

    let paths = scanned_paths(temp.path(), &["scan", ".", "--format", "json"]);

    assert!(paths.iter().any(|path| path.ends_with("thing.zork")), "got: {paths:?}");
}

/// The violated rule does not matter here; that a file was inspected at all does.
fn linted(temp: &TempDir) -> Outcome {
    lint(temp.path(), &lint_args(&["."])).expect("lint run")
}

#[test]
fn lint_inspects_a_custom_extension_from_the_root_config() {
    let temp = repo(&[
        (".uncommentrc.toml", ZORK_LINT),
        ("thing.zork", "# TODO: wire this up\nx = 1\n"),
    ]);

    let outcome = linted(&temp);

    assert!(
        !outcome.violations.is_empty(),
        "lint collects with its own registry, which must know the declared extension too"
    );
}

/// Declaring a language no grammar exists for registers nothing at all — not the language, not its
/// extensions — so the run behaves exactly as if the section were absent. Now that a declared
/// extension really does decide what is collected, that has to be said out loud rather than left for
/// the user to infer from a file that was skipped.
#[test]
fn a_declared_language_without_a_grammar_is_reported() {
    let temp = repo(&[
        (
            ".uncommentrc.toml",
            r#"
[languages.zorklang]
name = "zorklang"
extensions = ["zz"]
comment_nodes = ["comment"]
"#,
        ),
        ("thing.zz", "# a plain comment\nx = 1\n"),
    ]);

    let output = run(temp.path(), &["."]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stderr.contains("zorklang"),
        "the ignored declaration must name the language it could not register, got: {stderr}"
    );
}

#[test]
fn lint_inspects_a_custom_extension_from_a_subdirectory_config() {
    let temp = repo(&[
        ("sub/.uncommentrc.toml", ZORK_LINT),
        ("sub/thing.zork", "# TODO: wire this up\nx = 1\n"),
    ]);

    let outcome = linted(&temp);

    assert!(!outcome.violations.is_empty());
}
