use std::fs;
use tempfile::TempDir;
use uncomment::config::Config;

fn load_error(contents: &str) -> String {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("uncomment.toml");
    fs::write(&path, contents).unwrap();
    let error = Config::from_file(&path).expect_err("unknown key must be rejected");
    format!("{error:#}")
}

#[test]
fn unknown_global_key_is_rejected_by_name() {
    let rendered = load_error(
        r#"
[global]
remove_todos = false
remove_todoz = true
"#,
    );
    assert!(
        rendered.contains("remove_todoz"),
        "error should name the offending key, got: {rendered}"
    );
}

#[test]
fn unknown_top_level_key_is_rejected_by_name() {
    let rendered = load_error(
        r#"
[globl]
remove_todos = false
"#,
    );
    assert!(
        rendered.contains("globl"),
        "error should name the offending table, got: {rendered}"
    );
}

#[test]
fn unknown_pattern_key_is_rejected_by_name() {
    let rendered = load_error(
        r#"
[patterns."tests/**/*"]
remove_todoz = true
"#,
    );
    assert!(
        rendered.contains("remove_todoz"),
        "error should name the offending key, got: {rendered}"
    );
}

#[test]
fn unknown_language_key_is_rejected_by_name() {
    let rendered = load_error(
        r#"
[languages.python]
name = "Python"
extensions = [".py"]
comment_nodes = ["comment"]
remove_dox = true
"#,
    );
    assert!(
        rendered.contains("remove_dox"),
        "error should name the offending key, got: {rendered}"
    );
}

#[test]
fn every_shipped_template_still_parses() {
    for (label, template) in [
        ("template", Config::template()),
        ("template_clean", Config::template_clean()),
        ("comprehensive_template", Config::comprehensive_template()),
        ("comprehensive_template_clean", Config::comprehensive_template_clean()),
    ] {
        let parsed: Result<Config, _> = toml::from_str(&template);
        assert!(parsed.is_ok(), "{label} failed to parse: {:?}", parsed.err());
        parsed.unwrap().validate().unwrap_or_else(|e| panic!("{label}: {e:#}"));
    }
}
