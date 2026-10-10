//! Guards the published config schema, `schema/uncomment.schema.json`.
//!
//! The schema is what an editor validates `.uncomment.toml` against, so it has to accept every
//! config uncomment itself accepts — above all the ones `uncomment init` writes. A template that
//! grows a key the schema does not know, or a schema that drops a key a template uses, would make an
//! editor reject a file the tool is happy with; these tests fail first.

use std::fs;
use std::path::Path;

use serde_json::Value;
use tempfile::TempDir;
use uncomment::config::Config;

/// The published schema, parsed.
fn schema() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("schema/uncomment.schema.json");
    let raw = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&raw).expect("schema is valid JSON")
}

/// A TOML document as the JSON instance the schema validates.
fn instance(toml_src: &str) -> Value {
    let parsed: toml::Value = toml::from_str(toml_src).expect("document is valid TOML");
    serde_json::to_value(parsed).expect("TOML serializes to JSON")
}

fn validator() -> jsonschema::Validator {
    jsonschema::validator_for(&schema()).expect("schema compiles")
}

fn assert_valid(label: &str, toml_src: &str) {
    let errors: Vec<String> = validator()
        .iter_errors(&instance(toml_src))
        .map(|error| error.to_string())
        .collect();
    assert!(
        errors.is_empty(),
        "{label} is not valid against the schema:\n{}",
        errors.join("\n")
    );
}

/// A project with source files, so `smart_template` takes its language-detecting branch rather than
/// falling back to the static template.
fn auto_project() -> TempDir {
    let temp = TempDir::new().unwrap();
    fs::write(temp.path().join("main.py"), "print('hi')\n").unwrap();
    fs::write(temp.path().join("app.ts"), "export const x = 1;\n").unwrap();
    temp
}

#[test]
fn every_shipped_template_is_valid_against_the_schema() {
    let project = auto_project();
    let (smart_with_info, _) = Config::smart_template_with_info(project.path()).unwrap();

    for (label, template) in [
        ("template", Config::template()),
        ("template_clean", Config::template_clean()),
        ("comprehensive_template", Config::comprehensive_template()),
        ("comprehensive_template_clean", Config::comprehensive_template_clean()),
        ("smart_template", Config::smart_template(project.path()).unwrap()),
        ("smart_template_with_info", smart_with_info),
    ] {
        assert_valid(label, &template);
    }
}

/// Every key of every table, which none of the templates spells out — `[lint]` in particular.
#[test]
fn a_document_naming_every_key_is_valid_against_the_schema() {
    assert_valid(
        "kitchen sink",
        r#"
[global]
remove_todos = true
remove_fixme = true
remove_docs = true
preserve_patterns = ["NOTE"]
exclude = ["vendor/**"]
use_default_ignores = false
respect_gitignore = false
traverse_git_repos = true

[languages.python]
name = "Python"
extensions = ["py"]
filenames = ["SConstruct"]
comment_nodes = ["comment"]
doc_comment_nodes = ["string"]
preserve_patterns = ["noqa:"]
remove_todos = true
remove_fixme = false
remove_docs = true
use_default_ignores = false

[patterns."tests/**/*.py"]
remove_todos = false
remove_fixme = false
remove_docs = false
preserve_patterns = []
use_default_ignores = true

[lint]
enabled = true
tags = ["TODO", "NOTE"]
canonical_tag = "TODO"
case_sensitive_tags = true
key_pattern = '^\s*(?:TODO|NOTE)\((?<key>[A-Z][A-Z0-9]+-\d+)\)\s*:'
current_issue_from_branch = '([A-Z][A-Z0-9]+-\d+)'
include_doc_comments = true

[lint.rules]
tag-not-canonical = "warn"
todo-missing-key = "error"
todo-key-not-upper-case = "off"
tag-form-not-canonical = "warn"
todo-self-reference = "off"
"#,
    );
}

#[test]
fn an_unknown_key_is_rejected_by_the_schema() {
    assert!(!validator().is_valid(&instance("[global]\nremove_todoz = true\n")));
}

#[test]
fn a_wrongly_typed_value_is_rejected_by_the_schema() {
    assert!(!validator().is_valid(&instance("[global]\nremove_todos = \"yes\"\n")));
    assert!(!validator().is_valid(&instance("[lint.rules]\ntodo-missing-key = \"loud\"\n")));
}

#[test]
fn a_language_with_no_comment_nodes_is_rejected_by_the_schema() {
    assert!(!validator().is_valid(&instance(
        "[languages.python]\nname = \"Python\"\nextensions = [\"py\"]\n"
    )));
}
