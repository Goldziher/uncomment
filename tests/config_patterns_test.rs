use std::fs;
use tempfile::TempDir;
use uncomment::config::{Config, ConfigManager};
use uncomment::processor::Processor;

fn write(path: &std::path::Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

const PY_WITH_TODO: &str = "# TODO: tracked work\ndef hello():\n    pass\n";

#[test]
fn pattern_section_applies_to_matching_files_only() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    write(
        &root.join(".uncomment.toml"),
        r#"
[global]
remove_todos = false

[patterns."tests/**/*"]
remove_todos = true
"#,
    );

    let test_file = root.join("tests").join("test_thing.py");
    let src_file = root.join("src").join("thing.py");
    write(&test_file, PY_WITH_TODO);
    write(&src_file, PY_WITH_TODO);

    let manager = ConfigManager::new(root).unwrap();
    let mut processor = Processor::new();

    let processed_test = processor.process_file_with_config(&test_file, &manager, None).unwrap();
    let processed_src = processor.process_file_with_config(&src_file, &manager, None).unwrap();

    assert!(
        !processed_test.processed_content.contains("TODO: tracked work"),
        "pattern tests/**/* should remove the TODO under tests/, got:\n{}",
        processed_test.processed_content
    );
    assert!(
        processed_src.processed_content.contains("TODO: tracked work"),
        "file outside tests/ should keep its TODO, got:\n{}",
        processed_src.processed_content
    );
}

#[test]
fn overlapping_patterns_resolve_deterministically() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    write(
        &root.join(".uncomment.toml"),
        r#"
[global]
remove_todos = false
remove_docs = false

[patterns."**/*.py"]
remove_todos = true
remove_docs = true

[patterns."src/**/*.py"]
remove_docs = false
"#,
    );

    let file = root.join("src").join("thing.py");
    write(&file, PY_WITH_TODO);

    // A fresh ConfigManager per iteration gives the pattern HashMap a fresh
    // iteration order, so an order-dependent resolver flaps here.
    for iteration in 0..30 {
        let manager = ConfigManager::new(root).unwrap();
        let resolved = manager.get_config_for_file(&file);
        assert!(
            resolved.remove_todos,
            "iteration {iteration}: deeper pattern must not clobber remove_todos = true"
        );
        assert!(
            !resolved.remove_docs,
            "iteration {iteration}: src/**/*.py is more specific and must win remove_docs"
        );
    }
}

#[test]
fn nested_config_patterns_are_relative_to_that_config_directory() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    write(
        &root.join(".uncomment.toml"),
        r#"
[global]
remove_todos = false
"#,
    );
    write(
        &root.join("nested").join(".uncomment.toml"),
        r#"
[patterns."deep/*.py"]
remove_todos = true
"#,
    );

    let nested_deep = root.join("nested").join("deep").join("thing.py");
    let nested_other = root.join("nested").join("other").join("thing.py");
    let root_deep = root.join("deep").join("thing.py");
    for path in [&nested_deep, &nested_other, &root_deep] {
        write(path, PY_WITH_TODO);
    }

    let manager = ConfigManager::new(root).unwrap();

    assert!(
        manager.get_config_for_file(&nested_deep).remove_todos,
        "nested/deep/thing.py matches deep/*.py relative to nested/"
    );
    assert!(
        !manager.get_config_for_file(&nested_other).remove_todos,
        "nested/other/thing.py must not match deep/*.py"
    );
    assert!(
        !manager.get_config_for_file(&root_deep).remove_todos,
        "deep/thing.py at the root must not match a pattern declared in nested/"
    );
}

#[test]
fn invalid_glob_is_a_config_error_naming_the_pattern() {
    let temp = TempDir::new().unwrap();
    let config_path = temp.path().join("uncomment.toml");
    write(
        &config_path,
        r#"
[global]
remove_todos = false

[patterns."src/[unclosed"]
remove_todos = true
"#,
    );

    let error = Config::from_file(&config_path).expect_err("invalid glob must be rejected");
    let rendered = format!("{error:#}");
    assert!(
        rendered.contains("src/[unclosed"),
        "error should name the offending pattern, got: {rendered}"
    );
}

#[test]
fn single_star_does_not_cross_a_path_separator() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    write(
        &root.join(".uncomment.toml"),
        r#"
[global]
remove_todos = false

[patterns."src/*.py"]
remove_todos = true
"#,
    );

    let shallow = root.join("src").join("thing.py");
    let nested = root.join("src").join("inner").join("thing.py");
    write(&shallow, PY_WITH_TODO);
    write(&nested, PY_WITH_TODO);

    let manager = ConfigManager::new(root).unwrap();

    assert!(
        manager.get_config_for_file(&shallow).remove_todos,
        "src/*.py must match src/thing.py"
    );
    assert!(
        !manager.get_config_for_file(&nested).remove_todos,
        "src/*.py must not match src/inner/thing.py"
    );
}

#[test]
fn pattern_overrides_are_applied_before_language_overrides() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    write(
        &root.join(".uncomment.toml"),
        r#"
[global]
remove_todos = false

[languages.python]
name = "Python"
extensions = [".py"]
comment_nodes = ["comment"]
remove_todos = false

[patterns."**/*.py"]
remove_todos = true
"#,
    );

    let file = root.join("thing.py");
    write(&file, PY_WITH_TODO);

    let manager = ConfigManager::new(root).unwrap();
    assert!(
        manager.get_config_for_file(&file).remove_todos,
        "the pattern override alone should switch remove_todos on"
    );
    assert!(
        !manager.get_config_for_file_with_language(&file, "python").remove_todos,
        "an explicit language override is applied after patterns and wins"
    );
}

/// `preserve_patterns = []` is written under "be more aggressive with generated files" in
/// eight shipped templates, so it has to mean "clear the inherited list", not "inherit it".
#[test]
fn empty_preserve_patterns_in_a_pattern_section_clears_the_inherited_list() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    write(
        &root.join(".uncomment.toml"),
        r#"
[global]
preserve_patterns = ["KEEPME"]

[patterns."**/*.generated.*"]
preserve_patterns = []

[patterns."extra/**/*"]
preserve_patterns = ["ALSOKEEP"]
"#,
    );

    let generated = root.join("thing.generated.py");
    let extra = root.join("extra").join("thing.py");
    let plain = root.join("thing.py");
    for path in [&generated, &extra, &plain] {
        write(path, PY_WITH_TODO);
    }

    let manager = ConfigManager::new(root).unwrap();

    assert!(
        manager.get_config_for_file(&generated).preserve_patterns.is_empty(),
        "an explicit empty list must clear the inherited patterns, got: {:?}",
        manager.get_config_for_file(&generated).preserve_patterns
    );
    assert_eq!(
        manager.get_config_for_file(&plain).preserve_patterns,
        vec!["KEEPME".to_string()],
        "a file matching no pattern keeps the global list"
    );
    assert_eq!(
        manager.get_config_for_file(&extra).preserve_patterns,
        vec!["ALSOKEEP".to_string(), "KEEPME".to_string()],
        "a non-empty list still extends the inherited one"
    );
}

/// An omitted `preserve_patterns` key inherits; only an explicit `[]` clears.
#[test]
fn omitted_preserve_patterns_in_a_pattern_section_inherits() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    write(
        &root.join(".uncomment.toml"),
        r#"
[global]
preserve_patterns = ["KEEPME"]

[patterns."**/*.py"]
remove_todos = true
"#,
    );

    let file = root.join("thing.py");
    write(&file, PY_WITH_TODO);

    let resolved = ConfigManager::new(root).unwrap().get_config_for_file(&file);
    assert!(resolved.remove_todos);
    assert_eq!(resolved.preserve_patterns, vec!["KEEPME".to_string()]);
}

/// A forced `--config` anchors its globs at the invocation directory, not at the
/// directory holding the config file.
#[test]
fn forced_config_globs_are_anchored_at_the_invocation_directory() {
    let temp = TempDir::new().unwrap();
    let shared = temp.path().join("shared");
    let project = temp.path().join("project");

    write(
        &shared.join("uncomment.toml"),
        r#"
[global]
remove_todos = false

[patterns."src/**/*.py"]
remove_todos = true
"#,
    );

    let inside = project.join("src").join("thing.py");
    let outside = project.join("other").join("thing.py");
    write(&inside, PY_WITH_TODO);
    write(&outside, PY_WITH_TODO);

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_uncomment"))
        .current_dir(&project)
        .args(["--config", "../shared/uncomment.toml", "src", "other"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        !fs::read_to_string(&inside).unwrap().contains("TODO: tracked work"),
        "src/**/*.py is matched relative to the invocation directory"
    );
    assert!(
        fs::read_to_string(&outside).unwrap().contains("TODO: tracked work"),
        "other/thing.py matches nothing, so the global remove_todos = false stands"
    );
}
