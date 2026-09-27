//! Bare-filename language detection, and the two languages it unlocks.
//!
//! `BUILD`, `WORKSPACE` and `MODULE.bazel` carry no usable extension, so nothing about them can be
//! expressed as an extension rule. Detection therefore has to be able to claim a whole filename,
//! and that claim has to come from data a config file can also supply — otherwise every new
//! extensionless format needs a new arm in a hardcoded `match`.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;
use uncomment::languages::config::LanguageConfig;
use uncomment::languages::registry::LanguageRegistry;

fn detected(registry: &LanguageRegistry, file_name: &str) -> String {
    registry
        .detect_language(Path::new(file_name))
        .unwrap_or_else(|| panic!("no language detected for `{file_name}`"))
        .name
        .clone()
}

/// A fixture repository. `.git/HEAD` makes the temporary directory a repository root, which is what
/// bounds config discovery.
fn repo(files: &[(&str, &str)]) -> TempDir {
    let temp = TempDir::new().expect("temp dir");
    let entries = [(".git/HEAD", "ref: refs/heads/feat/no-key-here\n")];
    for (relative, content) in entries.iter().chain(files.iter()) {
        let path = temp.path().join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(&path, content).expect("write fixture file");
    }
    temp
}

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_uncomment"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run uncomment")
}

fn run_ok(dir: &Path, args: &[&str]) -> Output {
    let output = run(dir, args);
    assert!(
        output.status.success(),
        "uncomment {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[test]
fn every_bazel_file_shape_detects_as_starlark() {
    let registry = LanguageRegistry::new();

    for file_name in [
        "BUILD",
        "BUILD.bazel",
        "WORKSPACE",
        "WORKSPACE.bazel",
        "WORKSPACE.bzlmod",
        "MODULE.bazel",
        "defs.bzl",
        "macros.bazel",
        "rules.star",
    ] {
        assert_eq!(detected(&registry, file_name), "starlark", "detecting `{file_name}`");
    }
}

#[test]
fn a_bazel_filename_is_detected_below_a_directory_too() {
    let registry = LanguageRegistry::new();

    assert_eq!(
        registry
            .detect_language(Path::new("armis/services/mtms/BUILD"))
            .map(|config| config.name.as_str()),
        Some("starlark")
    );
}

/// `BUILD.bazel` has a `.bazel` extension *and* is a name starlark claims outright. The claim on the
/// whole name has to be consulted first, or an extension rule for `.bazel` decides the language of a
/// file whose name was spoken for.
#[test]
fn a_claimed_filename_wins_over_an_extension_rule() {
    let mut registry = LanguageRegistry::new();
    registry.register_language(
        LanguageConfig::new("generated-python", vec![], vec!["comment"], vec![], "python")
            .with_filenames(vec!["setup.py"]),
    );

    assert_eq!(detected(&registry, "setup.py"), "generated-python");
    assert_eq!(
        detected(&registry, "other.py"),
        "python",
        "unclaimed names still go by extension"
    );
}

#[test]
fn the_make_and_dockerfile_filenames_still_detect() {
    let registry = LanguageRegistry::new();

    for (file_name, expected) in [
        ("Makefile", "make"),
        ("makefile", "make"),
        ("GNUmakefile", "make"),
        ("rules.mk", "make"),
        ("Dockerfile", "dockerfile"),
        ("dockerfile", "dockerfile"),
        ("Dockerfile.prod", "dockerfile"),
        ("dockerfile.dev", "dockerfile"),
        (".bashrc", "shell"),
        ("zshenv", "shell"),
        ("index.d.ts", "typescript"),
    ] {
        assert_eq!(detected(&registry, file_name), expected, "detecting `{file_name}`");
    }
}

#[test]
fn a_properties_file_detects_as_properties() {
    let registry = LanguageRegistry::new();
    let config = registry
        .get_language("properties")
        .expect("properties is a built-in language");

    assert_eq!(detected(&registry, "log4j.properties"), "properties");
    assert!(
        config.is_comment_type("comment"),
        "the grammar's comment node is `comment`"
    );
}

/// Both grammars are `#`-commented, which is what `keep` needs to write a marker line.
#[test]
fn the_new_languages_carry_a_hash_line_token() {
    let registry = LanguageRegistry::new();

    for name in ["starlark", "properties"] {
        let config = registry
            .get_language(name)
            .unwrap_or_else(|| panic!("`{name}` is a built-in language"));
        assert_eq!(config.line_comment_token(), Some("#"), "line token for `{name}`");
    }
}

/// Starlark has docstrings, not doc comments, so `string` is its doc-comment type — the same pair
/// Python declares. The kind is only safe because `PythonHandler` classifies it: a string outside
/// docstring position is rejected rather than collected as a comment, which is what keeps every
/// string literal in a `BUILD` file out of the doc-comment machinery. The two halves must stay
/// together; see `tests/starlark_test.rs`.
#[test]
fn starlark_declares_string_as_its_doc_comment_type() {
    let registry = LanguageRegistry::new();
    let config = registry.get_language("starlark").expect("starlark is a built-in");

    assert_eq!(config.get_doc_comment_types(), ["string"]);
}

/// The only thing that makes a `[languages]` section useful is that a file it claims is collected,
/// and collection happens before any comment is looked at. A `filenames` key no collector consults
/// registers nothing and does nothing.
#[test]
fn a_filename_declared_in_a_config_is_collected_and_processed() {
    let config = r#"
[languages.python]
name = "python"
extensions = []
filenames = ["Zorkfile"]
comment_nodes = ["comment"]
"#;
    let temp = repo(&[(".uncomment.toml", config), ("Zorkfile", "# a plain comment\nx = 1\n")]);

    run_ok(temp.path(), &["."]);

    let processed = fs::read_to_string(temp.path().join("Zorkfile")).expect("read fixture");
    assert!(
        !processed.contains("# a plain comment"),
        "the declared filename must be collected and processed, got: {processed:?}"
    );
    assert!(processed.contains("x = 1"), "the code must survive, got: {processed:?}");
}

#[test]
fn a_build_file_loses_a_plain_comment_and_keeps_a_marked_one() {
    let build = "# AUTOGENERATED by gazelle — do not edit\n\
                 load(\"@rules_python//python:defs.bzl\", \"py_library\")\n\
                 \n\
                 # a plain explanatory comment\n\
                 py_library(\n\
                 \x20   name = \"thing\",  # ~keep the target name is load-bearing\n\
                 \x20   srcs = [\"thing.py\"],\n\
                 )\n";
    let temp = repo(&[("BUILD", build)]);

    run_ok(temp.path(), &["."]);

    let processed = fs::read_to_string(temp.path().join("BUILD")).expect("read fixture");
    assert!(
        !processed.contains("a plain explanatory comment"),
        "the plain comment must go, got: {processed:?}"
    );
    assert!(
        processed.contains("AUTOGENERATED by gazelle"),
        "a generated-file header is preserved by default, got: {processed:?}"
    );
    assert!(
        processed.contains("~keep the target name is load-bearing"),
        "a ~keep marker must survive, got: {processed:?}"
    );
    assert!(
        processed.contains("name = \"thing\"") && processed.contains("srcs = [\"thing.py\"]"),
        "the rule itself must survive, got: {processed:?}"
    );
}

#[test]
fn a_properties_file_loses_a_plain_comment() {
    let temp = repo(&[(
        "log4j.properties",
        "# a plain comment\n! also a comment\nlog4j.rootLogger=INFO\n",
    )]);

    run_ok(temp.path(), &["."]);

    let processed = fs::read_to_string(temp.path().join("log4j.properties")).expect("read fixture");
    assert!(
        !processed.contains("a plain comment") && !processed.contains("also a comment"),
        "both `#` and `!` comment forms must go, got: {processed:?}"
    );
    assert!(
        processed.contains("log4j.rootLogger=INFO"),
        "the property must survive, got: {processed:?}"
    );
}
