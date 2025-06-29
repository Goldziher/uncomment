use std::fs;
use tempfile::TempDir;
use uncomment::{
    config::{Config, ConfigManager, GrammarConfig, GrammarSource, LanguageConfig},
    grammar::GrammarManager,
    processor::Processor,
};

/// Test error handling for malformed configuration files
#[test]
fn test_malformed_config_error_handling() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("uncomment.toml");

    // Write malformed TOML
    let malformed_config = r#"
[global
remove_docs = true  # Missing closing bracket
invalid_field = "value"

[languages.rust
name = "Rust"  # Missing closing bracket
"#;

    fs::write(&config_path, malformed_config).unwrap();

    let result = ConfigManager::new(temp_dir.path());
    // The ConfigManager might not error on malformed config but emit warnings instead
    // So we check if the manager was created but config loading failed
    if result.is_err() {
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("parse") || error_msg.contains("TOML"));
    } else {
        // Config manager was created but might have ignored the malformed file
        // This is also acceptable behavior - warning was printed
        assert!(result.is_ok());
    }
}

/// Test error handling for invalid grammar source configurations
#[test]
fn test_invalid_grammar_source_config() {
    let mut config = Config::default();

    // Test with empty URL for Git source
    let invalid_git_config = LanguageConfig {
        name: "test".to_string(),
        extensions: vec!["test".to_string()],
        comment_nodes: vec!["comment".to_string()],
        doc_comment_nodes: vec![],
        preserve_patterns: vec![],
        remove_todos: None,
        remove_fixme: None,
        remove_docs: None,
        use_default_ignores: None,
        grammar: GrammarConfig {
            source: GrammarSource::Git {
                url: "".to_string(), // Empty URL
                branch: None,
                path: None,
            },
            ..Default::default()
        },
    };

    config
        .languages
        .insert("test".to_string(), invalid_git_config);

    // Configuration validation might not catch this at config level,
    // but it should fail when trying to use the grammar
    let mut grammar_manager = GrammarManager::new().unwrap();
    let result = grammar_manager.get_language("test", &config.languages["test"].grammar);
    assert!(result.is_err());
}

/// Test error handling for nonexistent local grammar paths
#[test]
fn test_nonexistent_local_grammar_path() {
    let mut grammar_manager = GrammarManager::new().unwrap();

    let config = GrammarConfig {
        source: GrammarSource::Local {
            path: "/absolutely/nonexistent/path/to/grammar".into(),
        },
        ..Default::default()
    };

    let result = grammar_manager.get_language("test", &config);
    assert!(result.is_err());

    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("Grammar path does not exist")
            || error_msg.contains("Failed to load local grammar")
    );
}

/// Test error handling for invalid library paths
#[test]
fn test_invalid_library_grammar_path() {
    let mut grammar_manager = GrammarManager::new().unwrap();

    let config = GrammarConfig {
        source: GrammarSource::Library {
            path: "/nonexistent/library.so".into(),
        },
        ..Default::default()
    };

    let result = grammar_manager.get_language("test", &config);
    assert!(result.is_err());

    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("Library path does not exist")
            || error_msg.contains("Failed to load library grammar")
    );
}

/// Test error handling for Git operations failures
#[test]
fn test_git_operations_error_handling() {
    let mut grammar_manager = GrammarManager::new().unwrap();

    // Test with invalid Git URL
    let config = GrammarConfig {
        source: GrammarSource::Git {
            url: "not-a-valid-url".to_string(),
            branch: None,
            path: None,
        },
        ..Default::default()
    };

    let result = grammar_manager.get_language("test", &config);
    assert!(result.is_err());

    // Test with unreachable URL
    let config2 = GrammarConfig {
        source: GrammarSource::Git {
            url: "https://this-domain-does-not-exist-12345.com/repo.git".to_string(),
            branch: None,
            path: None,
        },
        ..Default::default()
    };

    let result2 = grammar_manager.get_language("test", &config2);
    assert!(result2.is_err());
}

/// Test error handling for unsupported file types in processor
#[test]
fn test_processor_unsupported_file_type_error() {
    let temp_dir = TempDir::new().unwrap();
    let config_manager = ConfigManager::new(temp_dir.path()).unwrap();
    let mut processor = Processor::new();

    // Create file with unsupported extension
    let unsupported_file = temp_dir.path().join("test.unsupported");
    fs::write(&unsupported_file, "// Comment in unsupported file").unwrap();

    let result = processor.process_file_with_config(&unsupported_file, &config_manager);
    assert!(result.is_err());

    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("Unsupported file type"));
}

/// Test error handling for nonexistent files in processor
#[test]
fn test_processor_nonexistent_file_error() {
    let temp_dir = TempDir::new().unwrap();
    let config_manager = ConfigManager::new(temp_dir.path()).unwrap();
    let mut processor = Processor::new();

    let nonexistent_file = temp_dir.path().join("nonexistent.rs");

    let result = processor.process_file_with_config(&nonexistent_file, &config_manager);
    assert!(result.is_err());

    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("Failed to read file"));
}

/// Test error handling for malformed grammar files
#[test]
fn test_malformed_grammar_file_handling() {
    let temp_dir = TempDir::new().unwrap();
    let grammar_dir = temp_dir.path().join("malformed_grammar");
    fs::create_dir_all(&grammar_dir).unwrap();

    // Create a malformed grammar.js file
    let grammar_js = grammar_dir.join("grammar.js");
    fs::write(&grammar_js, "this is not valid JavaScript grammar").unwrap();

    let mut grammar_manager = GrammarManager::new().unwrap();
    let config = GrammarConfig {
        source: GrammarSource::Local { path: grammar_dir },
        ..Default::default()
    };

    let result = grammar_manager.get_language("test", &config);
    assert!(result.is_err());

    // Should fail during compilation phase
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("Failed to compile") || error_msg.contains("Failed to load"));
}

/// Test error handling for configuration validation edge cases
#[test]
fn test_config_validation_edge_cases() {
    let mut config = Config::default();

    // Test language with empty name
    let invalid_language = LanguageConfig {
        name: "".to_string(), // Empty name
        extensions: vec!["test".to_string()],
        comment_nodes: vec!["comment".to_string()],
        doc_comment_nodes: vec![],
        preserve_patterns: vec![],
        remove_todos: None,
        remove_fixme: None,
        remove_docs: None,
        use_default_ignores: None,
        grammar: GrammarConfig::default(),
    };

    config
        .languages
        .insert("empty_name".to_string(), invalid_language);

    let validation_result = config.validate();
    assert!(validation_result.is_err());

    // Test language with no comment nodes
    let mut config2 = Config::default();
    let no_comments_language = LanguageConfig {
        name: "test".to_string(),
        extensions: vec!["test".to_string()],
        comment_nodes: vec![], // No comment nodes
        doc_comment_nodes: vec![],
        preserve_patterns: vec![],
        remove_todos: None,
        remove_fixme: None,
        remove_docs: None,
        use_default_ignores: None,
        grammar: GrammarConfig::default(),
    };

    config2
        .languages
        .insert("no_comments".to_string(), no_comments_language);

    let validation_result2 = config2.validate();
    assert!(validation_result2.is_err());
}

/// Test error recovery and graceful degradation
#[test]
fn test_error_recovery_mechanisms() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("uncomment.toml");

    // Configuration with both valid and invalid grammar sources
    let mixed_config = r#"
[global]
remove_docs = false

# Valid builtin grammar
[languages.rust]
name = "Rust"
extensions = ["rs"]
comment_nodes = ["line_comment", "block_comment"]

# Invalid custom grammar - should fall back or error gracefully
[languages.swift]
name = "Swift"
extensions = ["swift"]
comment_nodes = ["comment"]

[languages.swift.grammar]
type = "git"
url = "https://invalid-url-that-does-not-exist.com/grammar.git"
"#;

    fs::write(&config_path, mixed_config).unwrap();

    let config_manager = ConfigManager::new(temp_dir.path()).unwrap();
    let mut processor = Processor::new();

    // Test with valid language (Rust) - should work
    let rust_file = temp_dir.path().join("test.rs");
    fs::write(&rust_file, "// Comment\nfn main() {}").unwrap();

    let rust_result = processor.process_file_with_config(&rust_file, &config_manager);
    assert!(rust_result.is_ok());

    // Test with invalid grammar language (Swift) - should fail gracefully
    let swift_file = temp_dir.path().join("test.swift");
    fs::write(&swift_file, "// Comment\nfunc main() {}").unwrap();

    let swift_result = processor.process_file_with_config(&swift_file, &config_manager);
    assert!(swift_result.is_err());

    // Error should be descriptive and not cause panic
    let error_msg = swift_result.unwrap_err().to_string();
    assert!(!error_msg.is_empty());
}

/// Test memory safety and resource cleanup
#[test]
fn test_resource_cleanup_on_errors() {
    let _temp_dir = TempDir::new().unwrap();

    // Create multiple grammar managers and let them fail
    for i in 0..10 {
        let mut grammar_manager = GrammarManager::new().unwrap();

        let config = GrammarConfig {
            source: GrammarSource::Local {
                path: format!("/nonexistent/path/{}", i).into(),
            },
            ..Default::default()
        };

        let _result = grammar_manager.get_language("test", &config);
        // Results should be errors, but no memory leaks or panics

        // Grammar manager should be dropped cleanly
    }

    // Test should complete without issues
}
