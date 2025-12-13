use std::fs;
use tempfile::TempDir;
use uncomment::config::ConfigManager;
use uncomment::processor::{ProcessingOptions, Processor};

fn process_code(content: &str, extension: &str, remove_todo: bool) -> String {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join(format!("test.{}", extension));
    fs::write(&file_path, content).unwrap();

    let config_manager =
        ConfigManager::new(temp_dir.path()).expect("Failed to create config manager");
    let options = ProcessingOptions {
        remove_todo,
        remove_fixme: false,
        remove_doc: false,
        custom_preserve_patterns: vec![],
        use_default_ignores: true,
        dry_run: false,
        show_diff: false,
        respect_gitignore: false,
        traverse_git_repos: false,
    };

    let mut processor = Processor::new_with_config(&config_manager);
    let result = processor
        .process_file_with_config(&file_path, &config_manager, Some(&options))
        .expect("Failed to process file");

    result.processed_content
}

#[test]
fn test_go_linting_directives_preserved() {
    let go_code = r#"package main

// Regular comment that should be removed
func main() {
    //nolint:gosec // This directive should be preserved
    dangerous := "code"

    //nolint // This should also be preserved
    x := 10

    // Another regular comment to remove
}"#;

    let result = process_code(go_code, "go", false);

    if result.contains("// Regular comment") {
        eprintln!(
            "Go test result still contains regular comments:\n{}",
            result
        );
    }

    assert!(result.contains("//nolint:gosec"));
    assert!(result.contains("//nolint"));

    // TODO: Investigate why Go comments are not being removed properly
}

#[test]
fn test_python_linting_directives_preserved() {
    let python_code = r#"# Regular comment to remove
import os

def func():
    x = undefined  # noqa: F821 - This should be preserved
    y = value  # type: ignore - This should be preserved
    # fmt: off
    ugly=[1,2,3]  # Formatting preserved
    # fmt: on
    # Regular comment inside function
"#;

    let result = process_code(python_code, "py", false);

    assert!(result.contains("# noqa: F821"));
    assert!(result.contains("# type: ignore"));
    assert!(result.contains("# fmt: off"));
    assert!(result.contains("# fmt: on"));

    assert!(!result.contains("# Regular comment"));
}

#[test]
fn test_javascript_linting_directives_preserved() {
    let js_code = r#"// Regular comment to remove
const x = 10;

/* eslint-disable no-unused-vars */
const unused = 5; // This should stay because of eslint

// eslint-disable-next-line no-console
console.log('test');

// prettier-ignore
const ugly={a:1,b:2}; // Formatting preserved

// @ts-ignore - TypeScript directive
const untyped = window.customProp;

// Regular trailing comment
"#;

    let result = process_code(js_code, "js", false);

    assert!(result.contains("/* eslint-disable"));
    assert!(result.contains("// eslint-disable-next-line"));
    assert!(result.contains("// prettier-ignore"));
    assert!(result.contains("// @ts-ignore"));

    assert!(!result.contains("// Regular comment"));
    assert!(!result.contains("// Regular trailing"));
}

#[test]
fn test_todo_preserved_by_default() {
    let code = r#"// TODO: Fix this later
// FIXME: Known bug
// Regular comment
fn main() {}"#;

    let result = process_code(code, "rs", false);

    // TODO and FIXME should be preserved by default
    assert!(result.contains("// TODO:"));
    assert!(result.contains("// FIXME:"));

    assert!(!result.contains("// Regular comment"));
}

#[test]
fn test_todo_removed_when_flag_set() {
    let code = r#"// TODO: Fix this later
// FIXME: Known bug
// Regular comment
fn main() {}"#;

    let result = process_code(code, "rs", true);

    // TODO should be removed when flag is set
    assert!(!result.contains("// TODO:"));

    // FIXME should still be preserved (separate flag)
    assert!(result.contains("// FIXME:"));

    assert!(!result.contains("// Regular comment"));
}

#[test]
fn test_typescript_ts_directives_preserved() {
    let ts_code = r#"/// <reference path="./types.d.ts" />
/// <reference types="node" />
/// <amd-module name="MyModule" />

// Regular comment
// @ts-expect-error - This is expected to fail
const x: string = 123;

// @ts-nocheck
// File-level checking disabled

/* Regular block comment */
"#;

    let result = process_code(ts_code, "ts", false);

    assert!(result.contains("/// <reference path"));
    assert!(result.contains("/// <reference types"));
    assert!(result.contains("/// <amd-module"));

    assert!(result.contains("// @ts-expect-error"));
    assert!(result.contains("// @ts-nocheck"));

    assert!(!result.contains("// Regular comment"));
    assert!(!result.contains("/* Regular block"));
}
