use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
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

/// Go through `Config::from_file`, the entry point every real config takes, rather than
/// `toml::from_str` plus a hand-rolled `validate` call.
fn assert_loads(label: &str, template: &str) {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join(".uncomment.toml");
    fs::write(&path, template).unwrap();

    Config::from_file(&path).unwrap_or_else(|e| panic!("{label} failed to load: {e:#}"));
}

#[test]
fn every_shipped_template_still_parses() {
    let project = TempDir::new().unwrap();
    fs::write(project.path().join("sample.py"), "# comment\nx = 1\n").unwrap();
    fs::write(project.path().join("sample.ts"), "// comment\nconst x = 1;\n").unwrap();

    let (smart_with_info, _) = Config::smart_template_with_info(project.path()).unwrap();

    for (label, template) in [
        ("template", Config::template()),
        ("template_clean", Config::template_clean()),
        ("comprehensive_template", Config::comprehensive_template()),
        ("comprehensive_template_clean", Config::comprehensive_template_clean()),
        ("smart_template", Config::smart_template(project.path()).unwrap()),
        ("smart_template_with_info", smart_with_info),
    ] {
        assert_loads(label, &template);
    }
}

/// `smart_template*` falls back to a static template when it detects no source files, so
/// that branch needs its own fixture.
#[test]
fn smart_templates_parse_for_a_project_with_no_source_files() {
    let empty = TempDir::new().unwrap();

    let (with_info, info) = Config::smart_template_with_info(empty.path()).unwrap();
    assert_eq!(info.configured_languages, 0);

    assert_loads(
        "smart_template (no files)",
        &Config::smart_template(empty.path()).unwrap(),
    );
    assert_loads("smart_template_with_info (no files)", &with_info);
}

/// `interactive_template_clean` is what `init --interactive` writes; it reads stdin, so it
/// is only reachable through the binary with the prompts answered.
#[test]
fn interactive_template_parses() {
    let temp = TempDir::new().unwrap();
    let output_path = temp.path().join(".uncomment.toml");

    let mut child = Command::new(env!("CARGO_BIN_EXE_uncomment"))
        .current_dir(temp.path())
        .args(["init", "--interactive", "--output"])
        .arg(&output_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"y\nn\ny\nall\n")
        .expect("answer the prompts");
    let status = child.wait().unwrap();
    assert!(status.success(), "init --interactive failed");

    let written = fs::read_to_string(&output_path).unwrap();
    assert!(written.contains("[languages.vue]"), "expected the selected languages");
    assert_loads("interactive_template_clean", &written);
}
