use std::fs;
use tempfile::TempDir;
use uncomment::cli::Cli;

/// Test basic init command functionality
#[test]
fn test_init_command_basic() {
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join(".uncommentrc.toml");

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp_dir.path()).unwrap();

    let result = Cli::handle_init_command(&output_path, false, false, false);

    std::env::set_current_dir(original_dir).unwrap();

    assert!(result.is_ok());
    assert!(output_path.exists());

    let content = fs::read_to_string(&output_path).unwrap();
    assert!(content.contains("[global]"));
    assert!(content.contains("remove_todos = false"));
    assert!(content.contains("preserve_patterns = [\"HACK\", \"WORKAROUND\", \"NOTE\"]"));
}

/// Test init command with force flag
#[test]
fn test_init_command_force() {
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join(".uncommentrc.toml");

    fs::write(&output_path, "existing content").unwrap();

    let result = Cli::handle_init_command(&output_path, false, false, false);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already exists"));

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

    assert!(content.contains("[languages.vue]"));
    assert!(content.contains("[languages.swift]"));
    assert!(content.contains("[languages.kotlin]"));
    assert!(content.contains("[languages.zig]"));
    assert!(content.contains("[languages.elixir]"));
    assert!(content.contains("[languages.julia]"));

    assert!(content.contains("source = { type = \"git\""));
    assert!(content.contains("tree-sitter-vue"));
    assert!(!content.contains("tree-sitter-swift"));
    assert!(!content.contains("[languages.swift.grammar]"));
    assert!(!content.contains("[languages.kotlin.grammar]"));

    assert!(!content.contains("# Web Development Languages"));
    assert!(!content.contains("# Mobile Development"));
}

/// Test smart template generation based on detected files
#[test]
fn test_init_command_smart_detection() {
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("smart.toml");

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

    let template = uncomment::config::Config::smart_template(temp_dir.path()).unwrap();
    fs::write(&output_path, template).unwrap();

    assert!(output_path.exists());
    let content = fs::read_to_string(&output_path).unwrap();

    assert!(content.contains("# Smart Uncomment Configuration"));
    assert!(content.contains("# Detected languages in your project:"));
    assert!(content.contains("py files"));
    assert!(content.contains("js files"));
    assert!(content.contains("rs files"));
    assert!(content.contains("go files"));
    assert!(content.contains("vue files"));
    assert!(content.contains("dockerfile files"));

    assert!(content.contains("[languages.python]"));
    assert!(content.contains("[languages.javascript]"));
    assert!(content.contains("[languages.rust]"));
    assert!(content.contains("[languages.go]"));
    assert!(content.contains("[languages.vue]"));
    assert!(content.contains("[languages.dockerfile]"));

    assert!(content.contains("tree-sitter-vue"));
    assert!(content.contains("tree-sitter-dockerfile"));
}

/// Test smart template with no files
#[test]
fn test_init_command_smart_no_files() {
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("empty.toml");

    let template = uncomment::config::Config::smart_template(temp_dir.path()).unwrap();
    fs::write(&output_path, template).unwrap();

    assert!(output_path.exists());
    let content = fs::read_to_string(&output_path).unwrap();

    assert!(content.contains("# Uncomment Configuration File"));
    assert!(!content.contains("# Smart Uncomment Configuration"));
}

/// Test smart template with builtin languages only
#[test]
fn test_init_command_smart_builtin_only() {
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("builtin.toml");

    fs::write(temp_dir.path().join("main.py"), "print('hello')").unwrap();
    fs::write(temp_dir.path().join("app.ts"), "console.log('hello')").unwrap();
    fs::write(temp_dir.path().join("lib.rs"), "fn main() {}").unwrap();

    let template = uncomment::config::Config::smart_template(temp_dir.path()).unwrap();
    fs::write(&output_path, template).unwrap();

    assert!(output_path.exists());
    let content = fs::read_to_string(&output_path).unwrap();

    assert!(content.contains("# Smart Uncomment Configuration"));
    assert!(content.contains("[languages.python]"));
    assert!(content.contains("[languages.typescript]"));
    assert!(content.contains("[languages.rust]"));

    assert!(!content.contains("tree-sitter-python"));
    assert!(!content.contains("tree-sitter-typescript"));
    assert!(!content.contains("tree-sitter-rust"));
}

/// Test error handling for invalid output paths
#[test]
fn test_init_command_invalid_path() {
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

    let parsed_config: Result<uncomment::config::Config, _> = toml::from_str(&content);
    assert!(parsed_config.is_ok());

    let config = parsed_config.unwrap();

    assert!(!config.global.remove_todos);
    assert!(!config.global.remove_fixme);
    assert!(!config.global.remove_docs);
    assert!(config.global.use_default_ignores);
    assert!(config.global.respect_gitignore);
    assert!(!config.global.traverse_git_repos);

    assert!(!config.global.preserve_patterns.is_empty());

    assert!(config.languages.contains_key("python"));
    assert!(config.languages.contains_key("javascript"));
    assert!(config.languages.contains_key("typescript"));

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

    let parsed_config: Result<uncomment::config::Config, _> = toml::from_str(&content);
    assert!(parsed_config.is_ok());

    let config = parsed_config.unwrap();

    assert!(config.languages.len() > 10);

    assert!(config.languages.contains_key("vue"));
    assert!(config.languages.contains_key("swift"));
    assert!(config.languages.contains_key("kotlin"));
    assert!(config.languages.contains_key("zig"));

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

    let template = uncomment::config::Config::smart_template(temp_dir.path()).unwrap();
    fs::write(&output_path, template).unwrap();

    let content = fs::read_to_string(&output_path).unwrap();

    assert!(content.contains("py files"));
    assert!(content.contains("ts files"));
    assert!(content.contains("tsx files"));
    assert!(content.contains("mjs files"));
    assert!(content.contains("yaml files"));
    assert!(content.contains("tf files"));
}
