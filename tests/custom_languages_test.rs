use std::fs;
use tempfile::TempDir;
use uncomment::{
    config::{Config, LanguageConfig},
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
    assert!(
        !processed
            .processed_content
            .contains("# This is an Elixir comment")
    );
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
