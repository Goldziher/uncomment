use std::fs;
use tempfile::TempDir;
use uncomment::{config::ConfigManager, processor::Processor};

/// Test processor integration with grammar manager for builtin grammars
#[test]
fn test_processor_with_builtin_grammars() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("uncomment.toml");

    let config_content = r#"
[global]
remove_docs = false

[languages.rust]
name = "Rust"
extensions = ["rs"]
comment_nodes = ["line_comment", "block_comment"]
doc_comment_nodes = ["doc_comment"]
remove_docs = true
"#;

    fs::write(&config_path, config_content).unwrap();

    let config_manager = ConfigManager::new(temp_dir.path()).unwrap();
    let mut processor = Processor::new();

    // Create a test Rust file
    let test_file = temp_dir.path().join("test.rs");
    let test_content = r#"
// This is a line comment
fn main() {
    /* Block comment */
    println!("Hello, world!");
}
"#;
    fs::write(&test_file, test_content).unwrap();

    // Process the file
    let result = processor.process_file_with_config(&test_file, &config_manager);
    assert!(result.is_ok());

    let processed = result.unwrap();
    assert!(processed.comments_removed > 0);

    // The processed content should have comments removed
    assert!(!processed
        .processed_content
        .contains("This is a line comment"));
    assert!(!processed.processed_content.contains("Block comment"));
}

/// Test processor with custom grammar configuration (will fail in test environment)
#[test]
fn test_processor_with_custom_grammar_config() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("uncomment.toml");

    let config_content = r#"
[global]
remove_docs = false

[languages.swift]
name = "Swift"
extensions = ["swift"]
comment_nodes = ["comment", "multiline_comment"]
doc_comment_nodes = ["doc_comment"]

[languages.swift.grammar]
source = { type = "git", url = "https://github.com/alex-pinkus/tree-sitter-swift", branch = "main" }
"#;

    fs::write(&config_path, config_content).unwrap();

    let config_manager = ConfigManager::new(temp_dir.path()).unwrap();
    let mut processor = Processor::new();

    // Create a test Swift file
    let test_file = temp_dir.path().join("test.swift");
    let test_content = r#"
// This is a Swift comment
func hello() {
    /* Block comment */
    print("Hello, Swift!")
}
"#;
    fs::write(&test_file, test_content).unwrap();

    // This will fail because we can't clone repos in test environment
    // But we can verify that the processor attempts to handle the custom grammar
    let result = processor.process_file_with_config(&test_file, &config_manager);

    // Should fail gracefully with a descriptive error
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("Unsupported file type")
            || error_msg.contains("Failed to load Git grammar")
            || error_msg.contains("Failed to load dynamic grammar")
    );
}

/// Test processor fallback behavior when dynamic grammar fails
#[test]
fn test_processor_fallback_to_builtin() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("uncomment.toml");

    // Configure Rust with an invalid custom grammar using inline config
    let config_content = r#"
[global]
remove_docs = false

[languages.rust]
name = "Rust"
extensions = ["rs"]
comment_nodes = ["line_comment", "block_comment"]
doc_comment_nodes = ["doc_comment"]

[languages.rust.grammar]
source = { type = "local", path = "/nonexistent/path/to/grammar" }
"#;

    fs::write(&config_path, config_content).unwrap();

    let config_manager = ConfigManager::new(temp_dir.path()).unwrap();
    let mut processor = Processor::new();

    // Create a test Rust file
    let test_file = temp_dir.path().join("test.rs");
    let test_content = r#"
// This is a line comment
fn main() {
    println!("Hello, world!");
}
"#;
    fs::write(&test_file, test_content).unwrap();

    // Should fail because the custom grammar path doesn't exist
    let result = processor.process_file_with_config(&test_file, &config_manager);
    assert!(result.is_err());

    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("Failed to load dynamic grammar")
            || error_msg.contains("Grammar path does not exist")
    );
}

/// Test processor with library grammar configuration
#[test]
fn test_processor_with_library_grammar() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("uncomment.toml");

    let config_content = r#"
[global]
remove_docs = false

[languages.kotlin]
name = "Kotlin"
extensions = ["kt"]
comment_nodes = ["comment"]

[languages.kotlin.grammar]
source = { type = "library", path = "/nonexistent/lib/libtree-sitter-kotlin.so" }
"#;

    fs::write(&config_path, config_content).unwrap();

    let config_manager = ConfigManager::new(temp_dir.path()).unwrap();
    let mut processor = Processor::new();

    // Create a test Kotlin file
    let test_file = temp_dir.path().join("test.kt");
    let test_content = r#"
// This is a Kotlin comment
fun main() {
    println("Hello, Kotlin!")
}
"#;
    fs::write(&test_file, test_content).unwrap();

    // Should fail because the library doesn't exist
    let result = processor.process_file_with_config(&test_file, &config_manager);
    assert!(result.is_err());

    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("Unsupported file type")
            || error_msg.contains("Library path does not exist")
            || error_msg.contains("Failed to load dynamic grammar")
            || error_msg.contains("Failed to load library grammar")
    );
}

/// Test multiple languages with different grammar configurations
#[test]
fn test_processor_multiple_grammar_types() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("uncomment.toml");

    let config_content = r#"
[global]
remove_docs = false

# Rust uses builtin grammar (no grammar section = default builtin)
[languages.rust]
name = "Rust"
extensions = ["rs"]
comment_nodes = ["line_comment", "block_comment"]

# Python uses builtin explicitly
[languages.python]
name = "Python"
extensions = ["py"]
comment_nodes = ["comment"]

[languages.python.grammar]
source = { type = "builtin" }

# JavaScript with custom git grammar (will fail in tests)
[languages.javascript]
name = "JavaScript"
extensions = ["js"]
comment_nodes = ["comment"]

[languages.javascript.grammar]
source = { type = "git", url = "https://github.com/tree-sitter/tree-sitter-javascript" }
"#;

    fs::write(&config_path, config_content).unwrap();

    let config_manager = ConfigManager::new(temp_dir.path()).unwrap();
    let mut processor = Processor::new();

    // Test Rust file (should work with builtin)
    let rust_file = temp_dir.path().join("test.rs");
    fs::write(&rust_file, "// Rust comment\nfn main() {}").unwrap();

    let rust_result = processor.process_file_with_config(&rust_file, &config_manager);
    assert!(rust_result.is_ok());

    // Test Python file (should work with explicit builtin)
    let python_file = temp_dir.path().join("test.py");
    fs::write(&python_file, "# Python comment\nprint('hello')").unwrap();

    let python_result = processor.process_file_with_config(&python_file, &config_manager);
    assert!(python_result.is_ok());

    // Test JavaScript file (should work normally as JS is still builtin despite config)
    let js_file = temp_dir.path().join("test.js");
    fs::write(&js_file, "// JS comment\nconsole.log('hello');").unwrap();

    let js_result = processor.process_file_with_config(&js_file, &config_manager);
    // This might succeed if JS is builtin, or fail if the grammar config is applied
    // Either way is valid - we're testing the configuration is parsed
    if js_result.is_err() {
        // If it fails, it should be due to git operations
        let error_msg = js_result.unwrap_err().to_string();
        assert!(
            error_msg.contains("git")
                || error_msg.contains("Failed to load")
                || error_msg.contains("grammar")
        );
    } else {
        // If it succeeds, the file should be processed
        assert!(js_result.unwrap().processed_content.contains("console.log"));
    }
}

/// Test error handling for unsupported file types with custom grammars
#[test]
fn test_processor_unsupported_file_type() {
    let temp_dir = TempDir::new().unwrap();
    let config_manager = ConfigManager::new(temp_dir.path()).unwrap();
    let mut processor = Processor::new();

    // Create a file with unsupported extension
    let unsupported_file = temp_dir.path().join("test.unknown");
    fs::write(&unsupported_file, "// Some comment").unwrap();

    let result = processor.process_file_with_config(&unsupported_file, &config_manager);
    assert!(result.is_err());

    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("Unsupported file type"));
}

/// Test grammar manager integration with processor caching
#[test]
fn test_processor_grammar_caching() {
    let temp_dir = TempDir::new().unwrap();
    let config_manager = ConfigManager::new(temp_dir.path()).unwrap();
    let mut processor = Processor::new();

    // Process the same language multiple times to test caching
    let rust_file1 = temp_dir.path().join("test1.rs");
    let rust_file2 = temp_dir.path().join("test2.rs");

    let test_content = "// Comment\nfn main() {}";
    fs::write(&rust_file1, test_content).unwrap();
    fs::write(&rust_file2, test_content).unwrap();

    // Process both files - second should use cached grammar
    let result1 = processor.process_file_with_config(&rust_file1, &config_manager);
    let result2 = processor.process_file_with_config(&rust_file2, &config_manager);

    assert!(result1.is_ok());
    assert!(result2.is_ok());

    // Both should have same number of comments removed
    assert_eq!(
        result1.unwrap().comments_removed,
        result2.unwrap().comments_removed
    );
}
