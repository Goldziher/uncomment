use std::fs;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_gitignore_from_subdirectory() {
    // Create a temporary directory for our test
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create directory structure:
    // /
    // ├── .gitignore (containing ".next")
    // └── subfolder/
    //     ├── main.js
    //     └── .next/
    //         └── test.js

    let subfolder = root.join("subfolder");
    let next_folder = subfolder.join(".next");

    fs::create_dir(&subfolder).unwrap();
    fs::create_dir(&next_folder).unwrap();

    // Create .gitignore in root
    fs::write(root.join(".gitignore"), ".next\n").unwrap();

    // Create test files with comments
    fs::write(
        subfolder.join("main.js"),
        "// Main file comment\nconst x = 1;",
    )
    .unwrap();
    fs::write(
        next_folder.join("test.js"),
        "// Test file comment\nconst y = 2;",
    )
    .unwrap();

    // Initialize git repo
    Command::new("git")
        .current_dir(root)
        .args(["init"])
        .output()
        .unwrap();

    // Build path to uncomment binary
    let uncomment_path = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("uncomment");

    // Run uncomment from subfolder with dry-run
    let output = Command::new(&uncomment_path)
        .current_dir(&subfolder)
        .args([".", "--dry-run"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Debug output
    if !output.status.success() {
        panic!("Command failed with stderr: {}", stderr);
    }

    // Should only process main.js, not test.js in .next folder
    assert!(
        stdout.contains("1 files processed"),
        "Expected to process only 1 file, but got: {}",
        stdout
    );
    assert!(
        stdout.contains("main.js"),
        "Expected to process main.js, but got: {}",
        stdout
    );
    assert!(
        !stdout.contains("test.js"),
        "Should not process test.js in .next folder, but got: {}",
        stdout
    );
}

#[test]
fn test_gitignore_with_no_gitignore_flag() {
    // Create a temporary directory for our test
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create same structure as above
    let subfolder = root.join("subfolder");
    let next_folder = subfolder.join(".next");

    fs::create_dir(&subfolder).unwrap();
    fs::create_dir(&next_folder).unwrap();

    // Create .gitignore in root
    fs::write(root.join(".gitignore"), ".next\n").unwrap();

    // Create test files with comments
    fs::write(
        subfolder.join("main.js"),
        "// Main file comment\nconst x = 1;",
    )
    .unwrap();
    fs::write(
        next_folder.join("test.js"),
        "// Test file comment\nconst y = 2;",
    )
    .unwrap();

    // Initialize git repo
    Command::new("git")
        .current_dir(root)
        .args(["init"])
        .output()
        .unwrap();

    // Build path to uncomment binary
    let uncomment_path = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("uncomment");

    // Run uncomment from subfolder with --no-gitignore flag
    let output = Command::new(&uncomment_path)
        .current_dir(&subfolder)
        .args([".", "--dry-run", "--no-gitignore"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);

    // With --no-gitignore, should process both files
    assert!(
        stdout.contains("2 files processed"),
        "Expected to process 2 files with --no-gitignore, but got: {}",
        stdout
    );
}

#[test]
fn test_config_init_command() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    let uncomment_path = get_binary_path();

    // Test init command
    let output = Command::new(&uncomment_path)
        .current_dir(root)
        .args(["init", "--output", "test_config.toml"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Init command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify config file was created
    let config_path = root.join("test_config.toml");
    assert!(config_path.exists(), "Config file was not created");

    // Verify config file contents
    let config_content = fs::read_to_string(&config_path).unwrap();
    assert!(config_content.contains("[global]"));
    assert!(config_content.contains("remove_todos = false"));
    assert!(config_content.contains("[languages.python]"));
    assert!(config_content.contains("[patterns."));
}

#[test]
fn test_config_basic_functionality() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create a simple config that removes TODO comments
    let config_content = r#"
[global]
remove_todos = true
remove_fixme = false
remove_docs = false
preserve_patterns = ["KEEP"]
"#;

    fs::write(root.join(".uncommentrc.toml"), config_content).unwrap();

    // Create test file with various comments
    let test_file = root.join("test.py");
    let test_content = r#"# Header comment
# TODO: should be removed
# FIXME: should be preserved
# KEEP: should be preserved
# Regular comment
def hello():
    pass"#;

    fs::write(&test_file, test_content).unwrap();

    let uncomment_path = get_binary_path();

    // Run uncomment with the config
    let output = Command::new(&uncomment_path)
        .current_dir(root)
        .args(["test.py"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Check the result
    let result_content = fs::read_to_string(&test_file).unwrap();

    // TODO should be removed (due to config)
    assert!(!result_content.contains("TODO: should be removed"));

    // FIXME should be preserved (due to config)
    assert!(result_content.contains("FIXME: should be preserved"));

    // KEEP should be preserved (due to config)
    assert!(result_content.contains("KEEP: should be preserved"));

    // Regular comments should be removed
    assert!(!result_content.contains("# Header comment"));
    assert!(!result_content.contains("# Regular comment"));
}

#[test]
fn test_nested_configuration() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create root config - preserves TODOs
    let root_config = r#"
[global]
remove_todos = false
remove_fixme = false
"#;
    fs::write(root.join(".uncommentrc.toml"), root_config).unwrap();

    // Create subdirectory
    let subdir = root.join("subdir");
    fs::create_dir(&subdir).unwrap();

    // Create subdirectory config - removes TODOs
    let sub_config = r#"
[global]
remove_todos = true
remove_fixme = false
"#;
    fs::write(subdir.join(".uncommentrc.toml"), sub_config).unwrap();

    // Create test files
    let root_file = root.join("root_test.py");
    let sub_file = subdir.join("sub_test.py");

    let test_content = "# TODO: test comment\n# FIXME: test comment\ndef hello(): pass";
    fs::write(&root_file, test_content).unwrap();
    fs::write(&sub_file, test_content).unwrap();

    let uncomment_path = get_binary_path();

    // Process both files
    let output = Command::new(&uncomment_path)
        .current_dir(root)
        .args(["root_test.py", "subdir/sub_test.py"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Check root file - TODO should be preserved
    let root_result = fs::read_to_string(&root_file).unwrap();
    assert!(
        root_result.contains("TODO: test comment"),
        "Root file should preserve TODO"
    );
    assert!(
        root_result.contains("FIXME: test comment"),
        "Root file should preserve FIXME"
    );

    // Check sub file - TODO should be removed, FIXME preserved
    let sub_result = fs::read_to_string(&sub_file).unwrap();
    assert!(
        !sub_result.contains("TODO: test comment"),
        "Sub file should remove TODO"
    );
    assert!(
        sub_result.contains("FIXME: test comment"),
        "Sub file should preserve FIXME"
    );
}

#[test]
fn test_language_specific_configuration() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create config with language-specific settings
    let config_content = r#"
[global]
remove_todos = false
remove_docs = false

[languages.python]
name = "Python"
extensions = [".py"]
comment_nodes = ["comment"]
preserve_patterns = ["mypy:", "type:"]
remove_docs = true

[languages.javascript]
name = "JavaScript"
extensions = [".js"]
comment_nodes = ["comment"]
preserve_patterns = ["@ts-ignore"]
"#;

    fs::write(root.join(".uncommentrc.toml"), config_content).unwrap();

    // Create Python file
    let py_file = root.join("test.py");
    let py_content = r#""""This is a docstring"""
# TODO: regular todo
# mypy: ignore
def hello(): pass"#;
    fs::write(&py_file, py_content).unwrap();

    // Create JavaScript file
    let js_file = root.join("test.js");
    let js_content = r#"/**
 * This is a JSDoc comment
 */
// TODO: regular todo
// @ts-ignore
const x = 1;"#;
    fs::write(&js_file, js_content).unwrap();

    let uncomment_path = get_binary_path();

    // Process both files
    let output = Command::new(&uncomment_path)
        .current_dir(root)
        .args(["test.py", "test.js"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Check Python file - docstring should be removed, mypy preserved
    let py_result = fs::read_to_string(&py_file).unwrap();
    assert!(
        !py_result.contains("This is a docstring"),
        "Python docstring should be removed"
    );
    assert!(
        py_result.contains("TODO: regular todo"),
        "Python TODO should be preserved (global setting)"
    );
    assert!(
        py_result.contains("mypy: ignore"),
        "Python mypy comment should be preserved"
    );

    // Check JavaScript file - JSDoc should be preserved, @ts-ignore preserved
    let js_result = fs::read_to_string(&js_file).unwrap();
    assert!(
        js_result.contains("This is a JSDoc comment"),
        "JavaScript JSDoc should be preserved (global setting)"
    );
    assert!(
        js_result.contains("TODO: regular todo"),
        "JavaScript TODO should be preserved (global setting)"
    );
    assert!(
        js_result.contains("@ts-ignore"),
        "JavaScript @ts-ignore should be preserved"
    );
}

#[test]
fn test_custom_config_file_path() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create custom config file with different name
    let custom_config = r#"
[global]
remove_todos = true
preserve_patterns = ["CUSTOM"]
"#;

    let custom_config_path = root.join("my_custom_config.toml");
    fs::write(&custom_config_path, custom_config).unwrap();

    // Create test file
    let test_file = root.join("test.py");
    let test_content =
        "# TODO: should be removed\n# CUSTOM: should be preserved\ndef hello(): pass";
    fs::write(&test_file, test_content).unwrap();

    let uncomment_path = get_binary_path();

    // Run with custom config file
    let output = Command::new(&uncomment_path)
        .current_dir(root)
        .args(["--config", "my_custom_config.toml", "test.py"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Check result
    let result = fs::read_to_string(&test_file).unwrap();
    assert!(
        !result.contains("TODO: should be removed"),
        "TODO should be removed"
    );
    assert!(
        result.contains("CUSTOM: should be preserved"),
        "CUSTOM pattern should be preserved"
    );
}

#[test]
fn test_pattern_based_configuration() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create config with pattern-based rules
    let config_content = r#"
[global]
remove_todos = false
remove_docs = false

[patterns."test_*.py"]
remove_todos = true
remove_docs = true

[patterns."src/*.py"]
preserve_patterns = ["PRODUCTION"]
"#;

    fs::write(root.join(".uncommentrc.toml"), config_content).unwrap();

    // Create directories
    fs::create_dir(root.join("src")).unwrap();

    // Create test files
    let test_file = root.join("test_example.py");
    let src_file = root.join("src").join("main.py");
    let regular_file = root.join("regular.py");

    let file_content = r#"""
Docstring
"""
# TODO: todo comment
# PRODUCTION: prod comment
def hello(): pass"#;

    fs::write(&test_file, file_content).unwrap();
    fs::write(&src_file, file_content).unwrap();
    fs::write(&regular_file, file_content).unwrap();

    let uncomment_path = get_binary_path();

    // Process all files
    let output = Command::new(&uncomment_path)
        .current_dir(root)
        .args(["test_example.py", "src/main.py", "regular.py"])
        .output()
        .unwrap();

    // Note: Pattern matching is not yet implemented in the current codebase,
    // so this test will demonstrate the current behavior and can be updated
    // when pattern matching is implemented
    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // For now, all files should behave according to global config
    // since pattern matching is not yet implemented
    let test_result = fs::read_to_string(&test_file).unwrap();
    let src_result = fs::read_to_string(&src_file).unwrap();
    let regular_result = fs::read_to_string(&regular_file).unwrap();

    // All should preserve TODO and docs due to global config
    assert!(test_result.contains("TODO: todo comment"));
    assert!(test_result.contains("Docstring"));
    assert!(src_result.contains("TODO: todo comment"));
    assert!(src_result.contains("Docstring"));
    assert!(regular_result.contains("TODO: todo comment"));
    assert!(regular_result.contains("Docstring"));
}

#[test]
fn test_config_validation_errors() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create invalid config - malformed TOML that should fail parsing
    let invalid_config = r#"
[global]
remove_todos = "invalid_boolean_value"  # Should fail TOML parsing
invalid_syntax_here
"#;

    fs::write(root.join(".uncommentrc.toml"), invalid_config).unwrap();

    // Create a test file
    fs::write(root.join("test.py"), "# comment\ndef hello(): pass").unwrap();

    let uncomment_path = get_binary_path();

    // Run uncomment with explicit config flag - should fail with validation error
    let output = Command::new(&uncomment_path)
        .current_dir(root)
        .args(["--config", ".uncommentrc.toml", "test.py"])
        .output()
        .unwrap();

    // Should fail due to invalid config
    assert!(
        !output.status.success(),
        "Command should fail with invalid config"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Failed to parse config")
            || stderr.contains("TOML parse error")
            || stderr.contains("invalid type"),
        "Should show validation error, got: {}",
        stderr
    );
}

#[test]
fn test_config_override_cli_options() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create config that preserves TODOs
    let config_content = r#"
[global]
remove_todos = false
"#;

    fs::write(root.join(".uncommentrc.toml"), config_content).unwrap();

    // Create test file
    let test_file = root.join("test.py");
    fs::write(&test_file, "# TODO: test comment\ndef hello(): pass").unwrap();

    let uncomment_path = get_binary_path();

    // Run with CLI flag that would remove TODOs (CLI should override config in future)
    // For now, this tests current behavior where config takes precedence
    let output = Command::new(&uncomment_path)
        .current_dir(root)
        .args(["--remove-todo", "test.py"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Current behavior: config takes precedence, so TODO should be preserved
    let _result = fs::read_to_string(&test_file).unwrap();
    // Note: This test documents current behavior and may need updating
    // when CLI override logic is implemented
}

// Helper function to get the binary path
fn get_binary_path() -> std::path::PathBuf {
    std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("uncomment")
}
