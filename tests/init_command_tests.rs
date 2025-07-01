use std::fs;
use tempfile::TempDir;
use uncomment::cli::Cli;

/// Test basic init command functionality
#[test]
fn test_init_command_basic() {
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join(".uncommentrc.toml");

    // Change to temp directory to avoid detecting files from uncomment project
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Test basic init without any files - should generate smart template but fallback to basic
    let result = Cli::handle_init_command(&output_path, false, false, false);

    // Restore original directory
    std::env::set_current_dir(original_dir).unwrap();

    assert!(result.is_ok());
    assert!(output_path.exists());

    let content = fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("[global]"));
    assert!(content.contains("remove_todos = false"));
    // Since no files are detected, it should fall back to the basic template (clean version)
    assert!(content.contains("preserve_patterns = [\"HACK\", \"WORKAROUND\", \"NOTE\"]"));
}

/// Test init command with force flag
#[test]
fn test_init_command_force() {
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join(".uncommentrc.toml");

    // Create existing file
    fs::write(&output_path, "existing content").unwrap();

    // Test without force - should fail
    let result = Cli::handle_init_command(&output_path, false, false, false);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already exists"));

    // Test with force - should succeed
    let result = Cli::handle_init_command(&output_path, true, false, false);
    assert!(result.is_ok());

    let content = fs::read_to_string(&output_path).unwrap();
    assert!(!content.contains("existing content"));
    assert!(content.contains("[global]"));
}

/// Test comprehensive template generation
#[test]
fn test_init_command_comprehensive() {
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("comprehensive.toml");

    let result = Cli::handle_init_command(&output_path, false, true, false);
    assert!(result.is_ok());
    assert!(output_path.exists());

    let content = fs::read_to_string(&output_path).unwrap();

    // Check that comprehensive template includes many languages (clean version)
    assert!(content.contains("[languages.vue]"));
    assert!(content.contains("[languages.swift]"));
    assert!(content.contains("[languages.kotlin]"));
    assert!(content.contains("[languages.zig]"));
    assert!(content.contains("[languages.elixir]"));
    assert!(content.contains("[languages.julia]"));

    // Check for grammar configurations
    assert!(content.contains("source = { type = \"git\""));
    assert!(content.contains("tree-sitter-vue"));
    assert!(content.contains("tree-sitter-swift"));

    // The clean version shouldn't have comments
    assert!(!content.contains("# Web Development Languages"));
    assert!(!content.contains("# Mobile Development"));
}

/// Test smart template generation based on detected files
#[test]
fn test_init_command_smart_detection() {
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("smart.toml");

    // Create various language files
    fs::write(temp_dir.path().join("main.py"), "print('hello')").unwrap();
    fs::write(temp_dir.path().join("app.js"), "console.log('hello')").unwrap();
    fs::write(temp_dir.path().join("lib.rs"), "fn main() {}").unwrap();
    fs::write(temp_dir.path().join("server.go"), "package main").unwrap();
    fs::write(
        temp_dir.path().join("component.vue"),
        "<template></template>",
    )
    .unwrap();
    fs::write(temp_dir.path().join("Dockerfile"), "FROM ubuntu").unwrap();

    // Generate smart config directly using the temp directory
    let template = uncomment::config::Config::smart_template(temp_dir.path()).unwrap();
    fs::write(&output_path, template).unwrap();

    assert!(output_path.exists());
    let content = fs::read_to_string(&output_path).unwrap();

    // Check that smart template detected the languages
    assert!(content.contains("# Smart Uncomment Configuration"));
    assert!(content.contains("# Detected languages in your project:"));
    assert!(content.contains("py files"));
    assert!(content.contains("js files"));
    assert!(content.contains("rs files"));
    assert!(content.contains("go files"));
    assert!(content.contains("vue files"));
    assert!(content.contains("dockerfile files"));

    // Check that relevant language configs are included
    assert!(content.contains("[languages.python]"));
    assert!(content.contains("[languages.javascript]"));
    assert!(content.contains("[languages.rust]"));
    assert!(content.contains("[languages.go]"));
    assert!(content.contains("[languages.vue]"));
    assert!(content.contains("[languages.dockerfile]"));

    // Check for Vue grammar config since it's not builtin
    assert!(content.contains("tree-sitter-vue"));
    assert!(content.contains("tree-sitter-dockerfile"));
}

/// Test smart template with no files
#[test]
fn test_init_command_smart_no_files() {
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("empty.toml");

    // Generate smart config directly using the empty temp directory
    let template = uncomment::config::Config::smart_template(temp_dir.path()).unwrap();
    fs::write(&output_path, template).unwrap();

    assert!(output_path.exists());
    let content = fs::read_to_string(&output_path).unwrap();

    // Should fall back to basic template when no files are detected
    assert!(content.contains("# Uncomment Configuration File"));
    assert!(!content.contains("# Smart Uncomment Configuration"));
}

/// Test smart template with builtin languages only
#[test]
fn test_init_command_smart_builtin_only() {
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("builtin.toml");

    // Create only builtin language files
    fs::write(temp_dir.path().join("main.py"), "print('hello')").unwrap();
    fs::write(temp_dir.path().join("app.ts"), "console.log('hello')").unwrap();
    fs::write(temp_dir.path().join("lib.rs"), "fn main() {}").unwrap();

    // Generate smart config directly using the temp directory
    let template = uncomment::config::Config::smart_template(temp_dir.path()).unwrap();
    fs::write(&output_path, template).unwrap();

    assert!(output_path.exists());
    let content = fs::read_to_string(&output_path).unwrap();

    // Check that smart template detected the languages
    assert!(content.contains("# Smart Uncomment Configuration"));
    assert!(content.contains("[languages.python]"));
    assert!(content.contains("[languages.typescript]"));
    assert!(content.contains("[languages.rust]"));

    // Should not contain grammar sections for builtin languages
    assert!(!content.contains("tree-sitter-python"));
    assert!(!content.contains("tree-sitter-typescript"));
    assert!(!content.contains("tree-sitter-rust"));
}

/// Test error handling for invalid output paths
#[test]
fn test_init_command_invalid_path() {
    // Try to write to a path that doesn't exist (parent directory doesn't exist)
    let invalid_path = std::path::PathBuf::from("/nonexistent/path/config.toml");

    let result = Cli::handle_init_command(&invalid_path, false, false, false);
    assert!(result.is_err());
}

/// Test config file contains all necessary sections
#[test]
fn test_init_command_config_completeness() {
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("complete.toml");

    let result = Cli::handle_init_command(&output_path, false, false, false);
    assert!(result.is_ok());

    let content = fs::read_to_string(&output_path).unwrap();

    // Test that generated config can be parsed
    let parsed_config: Result<uncomment::config::Config, _> = toml::from_str(&content);
    assert!(parsed_config.is_ok());

    let config = parsed_config.unwrap();

    // Verify structure
    assert_eq!(config.global.remove_todos, false);
    assert_eq!(config.global.remove_fixme, false);
    assert_eq!(config.global.remove_docs, false);
    assert_eq!(config.global.use_default_ignores, true);
    assert_eq!(config.global.respect_gitignore, true);
    assert_eq!(config.global.traverse_git_repos, false);

    // Should have some preserve patterns
    assert!(!config.global.preserve_patterns.is_empty());

    // Should have example language configs
    assert!(config.languages.contains_key("python"));
    assert!(config.languages.contains_key("javascript"));
    assert!(config.languages.contains_key("typescript"));

    // Should have pattern configs
    assert!(!config.patterns.is_empty());
}

/// Test that comprehensive config can be parsed correctly
#[test]
fn test_comprehensive_config_parsing() {
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("comprehensive_parse.toml");

    let result = Cli::handle_init_command(&output_path, false, true, false);
    assert!(result.is_ok());

    let content = fs::read_to_string(&output_path).unwrap();

    // Test that comprehensive config can be parsed
    let parsed_config: Result<uncomment::config::Config, _> = toml::from_str(&content);
    assert!(parsed_config.is_ok());

    let config = parsed_config.unwrap();

    // Should have many languages
    assert!(config.languages.len() > 10);

    // Check some specific languages exist
    assert!(config.languages.contains_key("vue"));
    assert!(config.languages.contains_key("swift"));
    assert!(config.languages.contains_key("kotlin"));
    assert!(config.languages.contains_key("zig"));

    // Check that grammar configs are present
    let vue_config = config.languages.get("vue").unwrap();
    assert!(matches!(
        vue_config.grammar.source,
        uncomment::config::GrammarSource::Git { .. }
    ));
}

/// Test different file extensions are detected correctly
#[test]
fn test_file_extension_detection() {
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("extensions.toml");

    // Create files with various extensions
    fs::write(temp_dir.path().join("script.py"), "# Python").unwrap();
    fs::write(
        temp_dir.path().join("types.d.ts"),
        "// TypeScript definitions",
    )
    .unwrap();
    fs::write(temp_dir.path().join("component.tsx"), "// React TypeScript").unwrap();
    fs::write(temp_dir.path().join("module.mjs"), "// ES Module").unwrap();
    fs::write(temp_dir.path().join("config.yaml"), "# YAML").unwrap();
    fs::write(temp_dir.path().join("main.tf"), "# Terraform").unwrap();

    // Generate smart config directly using the temp directory
    let template = uncomment::config::Config::smart_template(temp_dir.path()).unwrap();
    fs::write(&output_path, template).unwrap();

    let content = fs::read_to_string(&output_path).unwrap();

    // Check detection of various extensions
    assert!(content.contains("py files"));
    assert!(content.contains("ts files"));
    assert!(content.contains("tsx files"));
    assert!(content.contains("mjs files"));
    assert!(content.contains("yaml files"));
    assert!(content.contains("tf files"));
}
