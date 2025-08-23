use std::fs;
use tempfile::TempDir;
use uncomment::{
    config::{Config, GrammarConfig, GrammarSource, LanguageConfig},
    grammar::GrammarManager,
    processor::Processor,
};

/// Test that custom language configurations can be loaded
#[test]
fn test_custom_language_config_loading() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("uncomment.toml");

    // Write a configuration with custom languages
    let config_content = r#"
[global]
remove_docs = false

# Ruby via Git
[languages.ruby]
name = "Ruby"
extensions = ["rb"]
comment_nodes = ["comment"]
doc_comment_nodes = ["comment"]

[languages.ruby.grammar]
source = { type = "git", url = "https://github.com/tree-sitter/tree-sitter-ruby", branch = "master" }

# Swift via Git
[languages.swift]
name = "Swift"
extensions = ["swift"]
comment_nodes = ["comment", "multiline_comment"]

[languages.swift.grammar]
source = { type = "git", url = "https://github.com/alex-pinkus/tree-sitter-swift", branch = "main" }

# Vue.js via Git
[languages.vue]
name = "Vue"
extensions = ["vue"]
comment_nodes = ["comment"]

[languages.vue.grammar]
source = { type = "git", url = "https://github.com/tree-sitter-grammars/tree-sitter-vue", branch = "main" }
"#;

    fs::write(&config_path, config_content).unwrap();

    // Load and validate the configuration
    let config = Config::from_file(&config_path).unwrap();

    // Verify Ruby configuration
    assert!(config.languages.contains_key("ruby"));
    let ruby_config = &config.languages["ruby"];
    assert_eq!(ruby_config.name, "Ruby");
    assert_eq!(ruby_config.extensions, vec!["rb"]);
    assert!(matches!(
        ruby_config.grammar.source,
        GrammarSource::Git { .. }
    ));

    // Verify Swift configuration
    assert!(config.languages.contains_key("swift"));
    let swift_config = &config.languages["swift"];
    assert_eq!(swift_config.name, "Swift");
    assert_eq!(swift_config.extensions, vec!["swift"]);

    // Verify Vue configuration
    assert!(config.languages.contains_key("vue"));
    let vue_config = &config.languages["vue"];
    assert_eq!(vue_config.name, "Vue");
    assert_eq!(vue_config.extensions, vec!["vue"]);

    if let GrammarSource::Git { url, branch, .. } = &vue_config.grammar.source {
        assert_eq!(
            url,
            "https://github.com/tree-sitter-grammars/tree-sitter-vue"
        );
        assert_eq!(branch, &Some("main".to_string()));
    } else {
        panic!("Expected Git source for Vue grammar");
    }
}

/// Test that grammar manager can handle custom language requests
#[test]
fn test_custom_language_grammar_manager() {
    let mut grammar_manager = GrammarManager::new().unwrap();

    // Test that requesting a non-builtin language with Git source would attempt to load it
    let config = GrammarConfig {
        source: GrammarSource::Git {
            url: "https://github.com/tree-sitter/tree-sitter-ruby".to_string(),
            branch: Some("master".to_string()),
            path: None,
        },
        ..Default::default()
    };

    // This will fail in tests due to no network access, but we can verify the error
    let result = grammar_manager.get_language("ruby", &config);
    assert!(result.is_err());

    // The error should be related to Git operations, not "language not found"
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("Failed to load Git grammar")
            || error_msg.contains("git")
            || error_msg.contains("Failed to execute")
    );
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
remove_docs = true  # Override for Rust

# Custom language
[languages.kotlin]
name = "Kotlin"
extensions = ["kt", "kts"]
comment_nodes = ["line_comment", "multiline_comment"]

[languages.kotlin.grammar]
source = { type = "git", url = "https://github.com/fwcd/tree-sitter-kotlin", branch = "main" }
"#;

    fs::write(&config_path, config_content).unwrap();

    let config = Config::from_file(&config_path).unwrap();

    // Rust should have builtin grammar (no grammar config)
    let rust_config = &config.languages["rust"];
    assert!(matches!(rust_config.grammar.source, GrammarSource::Builtin));

    // Kotlin should have Git grammar
    let kotlin_config = &config.languages["kotlin"];
    assert!(matches!(
        kotlin_config.grammar.source,
        GrammarSource::Git { .. }
    ));
}

/// Test processor with custom language configuration (will fail gracefully)
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

[languages.elixir.grammar]
source = { type = "git", url = "https://github.com/elixir-lang/tree-sitter-elixir", branch = "main" }
"#;

    fs::write(&config_path, config_content).unwrap();

    let config_manager = uncomment::config::ConfigManager::new(temp_dir.path()).unwrap();
    let mut processor = Processor::new();

    // Create an Elixir file
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

    // Processing should fail gracefully with appropriate error
    let result = processor.process_file_with_config(&elixir_file, &config_manager, None);
    assert!(result.is_err());

    let error_msg = result.unwrap_err().to_string();
    // Should fail because it can't load the Git grammar (no network in tests)
    assert!(
        error_msg.contains("Failed to load dynamic grammar")
            || error_msg.contains("Failed to load Git grammar")
            || error_msg.contains("Unsupported file type")
    );
}

/// Test configuration with all grammar source types
#[test]
fn test_all_grammar_source_types() {
    let config_content = r#"
[global]
remove_docs = false

# Builtin (default)
[languages.python]
name = "Python"
extensions = ["py"]
comment_nodes = ["comment"]

# Explicit builtin
[languages.rust]
name = "Rust"
extensions = ["rs"]
comment_nodes = ["line_comment", "block_comment"]

[languages.rust.grammar]
source = { type = "builtin" }

# Git source
[languages.ruby]
name = "Ruby"
extensions = ["rb"]
comment_nodes = ["comment"]

[languages.ruby.grammar]
source = { type = "git", url = "https://github.com/tree-sitter/tree-sitter-ruby" }

# Local source
[languages.custom]
name = "Custom"
extensions = ["cst"]
comment_nodes = ["comment"]

[languages.custom.grammar]
source = { type = "local", path = "/path/to/grammar" }

# Library source
[languages.proprietary]
name = "Proprietary"
extensions = ["prop"]
comment_nodes = ["comment"]

[languages.proprietary.grammar]
source = { type = "library", path = "/path/to/lib.so" }
"#;

    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("uncomment.toml");
    fs::write(&config_path, config_content).unwrap();

    let config = Config::from_file(&config_path).unwrap();

    // Verify all languages loaded
    assert_eq!(config.languages.len(), 5);

    // Check each grammar source type
    assert!(matches!(
        config.languages["python"].grammar.source,
        GrammarSource::Builtin
    ));
    assert!(matches!(
        config.languages["rust"].grammar.source,
        GrammarSource::Builtin
    ));
    assert!(matches!(
        config.languages["ruby"].grammar.source,
        GrammarSource::Git { .. }
    ));
    assert!(matches!(
        config.languages["custom"].grammar.source,
        GrammarSource::Local { .. }
    ));
    assert!(matches!(
        config.languages["proprietary"].grammar.source,
        GrammarSource::Library { .. }
    ));
}

/// Test Vue.js configuration specifically
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
        grammar: GrammarConfig {
            source: GrammarSource::Git {
                url: "https://github.com/tree-sitter-grammars/tree-sitter-vue".to_string(),
                branch: Some("main".to_string()),
                path: None,
            },
            ..Default::default()
        },
    };

    // Verify configuration is valid
    assert_eq!(config.name, "Vue");
    assert!(config.extensions.contains(&"vue".to_string()));
    assert!(config.comment_nodes.contains(&"comment".to_string()));

    // Test with grammar manager
    let mut grammar_manager = GrammarManager::new().unwrap();
    let result = grammar_manager.get_language("vue", &config.grammar);

    // Will fail in test environment, but should attempt Git operations
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("Git") || error_msg.contains("grammar"));
}
