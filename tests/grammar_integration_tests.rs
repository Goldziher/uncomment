use std::fs;
use tempfile::TempDir;
use uncomment::{config::ConfigManager, processor::Processor};

/// Test processor integration with tslp grammars for builtin languages
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

    let test_file = temp_dir.path().join("test.rs");
    let test_content = r#"
// This is a line comment
fn main() {
    /* Block comment */
    println!("Hello, world!");
}
"#;
    fs::write(&test_file, test_content).unwrap();

    let result = processor.process_file_with_config(&test_file, &config_manager, None);
    assert!(result.is_ok());

    let processed = result.unwrap();
    assert!(processed.comments_removed > 0);

    assert!(
        !processed
            .processed_content
            .contains("This is a line comment")
    );
    assert!(!processed.processed_content.contains("Block comment"));
}

/// Test processor with swift - now available via tslp
#[test]
fn test_processor_with_swift() {
    let temp_dir = TempDir::new().unwrap();
    let config_manager = ConfigManager::new(temp_dir.path()).unwrap();
    let mut processor = Processor::new();

    let test_file = temp_dir.path().join("test.swift");
    let test_content = r#"
// This is a Swift comment
func hello() {
    /* Block comment */
    print("Hello, Swift!")
}
"#;
    fs::write(&test_file, test_content).unwrap();

    let result = processor.process_file_with_config(&test_file, &config_manager, None);
    assert!(result.is_ok());

    let processed = result.unwrap();
    assert!(
        !processed
            .processed_content
            .contains("This is a Swift comment")
    );
    assert!(!processed.processed_content.contains("Block comment"));
    assert!(
        processed
            .processed_content
            .contains("print(\"Hello, Swift!\")")
    );
}

/// Test multiple languages with tslp
#[test]
fn test_processor_multiple_languages() {
    let temp_dir = TempDir::new().unwrap();
    let config_manager = ConfigManager::new(temp_dir.path()).unwrap();
    let mut processor = Processor::new();

    // Rust
    let rust_file = temp_dir.path().join("test.rs");
    fs::write(&rust_file, "// Rust comment\nfn main() {}").unwrap();
    let rust_result = processor.process_file_with_config(&rust_file, &config_manager, None);
    assert!(rust_result.is_ok());

    // Python
    let python_file = temp_dir.path().join("test.py");
    fs::write(&python_file, "# Python comment\nprint('hello')").unwrap();
    let python_result = processor.process_file_with_config(&python_file, &config_manager, None);
    assert!(python_result.is_ok());

    // JavaScript
    let js_file = temp_dir.path().join("test.js");
    fs::write(&js_file, "// JS comment\nconsole.log('hello');").unwrap();
    let js_result = processor.process_file_with_config(&js_file, &config_manager, None);
    assert!(js_result.is_ok());
    assert!(js_result.unwrap().processed_content.contains("console.log"));
}

/// Test error handling for unsupported file types
#[test]
fn test_processor_unsupported_file_type() {
    let temp_dir = TempDir::new().unwrap();
    let config_manager = ConfigManager::new(temp_dir.path()).unwrap();
    let mut processor = Processor::new();

    let unsupported_file = temp_dir.path().join("test.unknown");
    fs::write(&unsupported_file, "// Some comment").unwrap();

    let result = processor.process_file_with_config(&unsupported_file, &config_manager, None);
    assert!(result.is_err());

    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("Unsupported file type"));
}

/// Test processor grammar caching (processing same language multiple times)
#[test]
fn test_processor_grammar_caching() {
    let temp_dir = TempDir::new().unwrap();
    let config_manager = ConfigManager::new(temp_dir.path()).unwrap();
    let mut processor = Processor::new();

    let rust_file1 = temp_dir.path().join("test1.rs");
    let rust_file2 = temp_dir.path().join("test2.rs");

    let test_content = "// Comment\nfn main() {}";
    fs::write(&rust_file1, test_content).unwrap();
    fs::write(&rust_file2, test_content).unwrap();

    let result1 = processor.process_file_with_config(&rust_file1, &config_manager, None);
    let result2 = processor.process_file_with_config(&rust_file2, &config_manager, None);

    assert!(result1.is_ok());
    assert!(result2.is_ok());

    assert_eq!(
        result1.unwrap().comments_removed,
        result2.unwrap().comments_removed
    );
}
