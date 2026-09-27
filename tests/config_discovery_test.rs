use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;
use uncomment::config::ConfigManager;

const DECOY_DIR_COUNT: usize = 2000;

fn write(path: &std::path::Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

/// Run the real binary so exit status and stderr are observable.
fn run_uncomment(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_uncomment"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}

/// A subtree the root config cannot possibly be affected by must never be walked.
/// The decoy config registers a custom language, which is observable through
/// `get_all_languages()` if — and only if — the subtree was descended into; it also
/// carries an unknown key, which is fatal when read, so a successful construction is
/// proof the file was never opened.
#[test]
fn construction_does_not_descend_below_the_root_directory() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    write(
        &root.join(".uncomment.toml"),
        r#"
[global]
remove_todos = false
"#,
    );

    let decoy_root = root.join("decoy");
    for index in 0..DECOY_DIR_COUNT / 20 {
        let branch = decoy_root.join(format!("branch{index}"));
        for leaf in 0..20 {
            fs::create_dir_all(branch.join(format!("leaf{leaf}"))).unwrap();
        }
    }
    write(
        &decoy_root.join("branch0").join("leaf0").join(".uncomment.toml"),
        r#"
[global]
remove_todoz = true

[languages.decoylang]
name = "DecoyLang"
extensions = [".decoy"]
comment_nodes = ["comment"]
"#,
    );

    let manager = ConfigManager::new(root).expect("a config below the root must not be read at construction time");

    assert!(
        !manager.get_all_languages().contains_key("decoylang"),
        "a config below the root must not be discovered at construction time"
    );
}

/// The root directory's own ancestors up to the git root are configuration for it,
/// so invoking from a subdirectory must not lose the repository's config.
#[test]
fn ancestor_config_up_to_the_git_root_is_loaded() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path().join("repo");
    fs::create_dir_all(repo.join(".git")).unwrap();

    write(
        &repo.join(".uncomment.toml"),
        r#"
[global]
remove_todos = true
"#,
    );

    let sub = repo.join("sub");
    let file = sub.join("thing.py");
    write(&file, "# TODO: tracked work\n");

    let manager = ConfigManager::new(&sub).unwrap();
    assert!(
        manager.get_config_for_file(&file).remove_todos,
        "the repository root config must apply when invoked from a subdirectory"
    );
}

#[test]
fn config_above_the_git_root_is_not_picked_up() {
    let temp = TempDir::new().unwrap();
    let outer = temp.path().join("outer");
    let repo = outer.join("repo");
    // A bare `.git` file is what worktrees and submodules get.
    write(&repo.join(".git"), "gitdir: /elsewhere\n");

    write(
        &outer.join(".uncomment.toml"),
        r#"
[global]
remove_todos = true
"#,
    );

    let file = repo.join("thing.py");
    write(&file, "# TODO: tracked work\n");

    let manager = ConfigManager::new(&repo).unwrap();
    assert!(
        !manager.get_config_for_file(&file).remove_todos,
        "the upward walk must stop at the git root"
    );
}

#[test]
fn nested_config_below_the_root_is_discovered_lazily() {
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
        &root.join("sub").join(".uncomment.toml"),
        r#"
[global]
remove_todos = true
"#,
    );

    let root_file = root.join("thing.py");
    let sub_file = root.join("sub").join("thing.py");
    write(&root_file, "# TODO: tracked work\n");
    write(&sub_file, "# TODO: tracked work\n");

    let manager = ConfigManager::new(root).unwrap();
    assert!(!manager.get_config_for_file(&root_file).remove_todos);
    assert!(
        manager.get_config_for_file(&sub_file).remove_todos,
        "a config below the root still applies to files under it"
    );
}

/// Custom language registration has to be decided up front, so a config only
/// reached lazily must not retroactively add a language to the registry.
#[test]
fn lazily_discovered_config_does_not_register_languages() {
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
        &root.join("sub").join(".uncomment.toml"),
        r#"
[global]
remove_todos = true

[languages.latelang]
name = "LateLang"
extensions = [".late"]
comment_nodes = ["comment"]
"#,
    );

    let sub_file = root.join("sub").join("thing.py");
    write(&sub_file, "# TODO: tracked work\n");

    let manager = ConfigManager::new(root).unwrap();
    assert!(manager.get_config_for_file(&sub_file).remove_todos);
    assert!(
        !manager.get_all_languages().contains_key("latelang"),
        "a lazily discovered config must not add a language after the fact"
    );
    assert!(manager.get_language_config("latelang").is_none());
}

/// A nested config that says nothing about `[global]` must not reset the globals the
/// outer config set: `bool` + a serde default cannot tell "absent" from "false".
#[test]
fn nested_config_without_a_global_section_inherits_the_outer_globals() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path().join("repo");
    fs::create_dir_all(repo.join(".git")).unwrap();

    write(
        &repo.join(".uncomment.toml"),
        r#"
[global]
remove_docs = true
respect_gitignore = false
"#,
    );
    write(
        &repo.join("sub").join(".uncomment.toml"),
        r#"
[patterns."*.py"]
remove_todos = true
"#,
    );

    let file = repo.join("sub").join("x.py");
    write(&file, "# TODO: tracked work\n");

    let manager = ConfigManager::new(repo.join("sub")).unwrap();
    let resolved = manager.get_config_for_file(&file);

    assert!(resolved.remove_docs, "the outer config's remove_docs must survive");
    assert!(
        !resolved.respect_gitignore,
        "the outer config's respect_gitignore must survive"
    );
    assert!(resolved.remove_todos, "the nested pattern section must still apply");
}

/// `.uncomment.toml` is the preferred name and needs no company to be found.
#[test]
fn the_preferred_config_name_alone_is_discovered() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    write(
        &root.join(".uncomment.toml"),
        r#"
[global]
remove_todos = true
"#,
    );

    let file = root.join("thing.py");
    write(&file, "# TODO: tracked work\n");

    let resolved = ConfigManager::new(root).unwrap().get_config_for_file(&file);
    assert!(resolved.remove_todos, ".uncomment.toml must be discovered and applied");
}

/// The preferred name outranks the deprecated dotfile, and the loser contributes nothing:
/// the highest-precedence name is used outright rather than merged.
#[test]
fn the_preferred_name_beats_the_legacy_dotfile_in_the_same_directory() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    write(
        &root.join(".uncomment.toml"),
        r#"
[global]
remove_todos = true
"#,
    );
    write(
        &root.join(".uncommentrc.toml"),
        r#"
[global]
remove_todos = false
remove_fixme = true
"#,
    );

    let file = root.join("thing.py");
    write(&file, "# TODO: tracked work\n");

    let resolved = ConfigManager::new(root).unwrap().get_config_for_file(&file);
    assert!(resolved.remove_todos, ".uncomment.toml must win");
    assert!(
        !resolved.remove_fixme,
        ".uncommentrc.toml must be ignored entirely when .uncomment.toml exists"
    );
}

/// Reading a config under a deprecated name says so once, naming both the file and the
/// name to move to — and never renames anything or fails the run.
#[test]
fn a_legacy_config_name_warns_once_naming_the_preferred_name() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    write(&root.join(".uncommentrc.toml"), "[global]\nremove_todos = false\n");
    write(
        &root.join("sub").join("uncomment.toml"),
        "[global]\nremove_todos = false\n",
    );
    write(&root.join("x.py"), "# a comment\nx = 1\n");
    write(&root.join("sub").join("y.py"), "# a comment\ny = 1\n");

    let output = run_uncomment(root, &[".", "--dry-run"]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "a deprecated name is a notice, not a failure, stderr: {stderr}"
    );
    assert!(
        stderr.contains(".uncommentrc.toml") && stderr.contains("rename it to .uncomment.toml"),
        "the notice must name the offending file and the preferred name, got: {stderr}"
    );
    assert_eq!(
        stderr.matches("deprecated").count(),
        1,
        "the notice must be emitted once per run, not once per config file, got: {stderr}"
    );
    assert!(
        root.join(".uncommentrc.toml").is_file() && root.join("sub").join("uncomment.toml").is_file(),
        "no config file may be renamed or removed"
    );
}

/// The preferred name is not deprecated, so it must warn about nothing.
#[test]
fn the_preferred_config_name_does_not_warn() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    write(&root.join(".uncomment.toml"), "[global]\nremove_todos = false\n");
    write(&root.join("x.py"), "# a comment\nx = 1\n");

    let output = run_uncomment(root, &[".", "--dry-run"]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success(), "stderr: {stderr}");
    assert!(
        !stderr.contains("deprecated"),
        "the preferred config name must not warn, got: {stderr}"
    );
}

/// Both deprecated names are still honoured, in their established order: a directory holding
/// `.uncommentrc.toml` and `uncomment.toml` — and no `.uncomment.toml` — uses the dotfile.
#[test]
fn legacy_dotfile_config_beats_uncomment_toml_in_the_same_directory() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    write(
        &root.join(".uncommentrc.toml"),
        r#"
[global]
remove_todos = true
"#,
    );
    write(
        &root.join("uncomment.toml"),
        r#"
[global]
remove_todos = false
remove_fixme = true
"#,
    );

    let file = root.join("thing.py");
    write(&file, "# TODO: tracked work\n");

    let resolved = ConfigManager::new(root).unwrap().get_config_for_file(&file);
    assert!(resolved.remove_todos, ".uncommentrc.toml must win");
    assert!(
        !resolved.remove_fixme,
        "uncomment.toml must be ignored entirely when the dotfile exists"
    );
}

/// `cd repo/sub && uncomment ../other` must not hand `other/` the config that governs
/// `sub/`: `Path::starts_with` is lexical, so an unnormalized `..` slips past it.
#[test]
fn a_parent_component_does_not_apply_the_config_of_a_sibling_directory() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path().join("repo");
    fs::create_dir_all(repo.join(".git")).unwrap();

    write(
        &repo.join("sub").join(".uncomment.toml"),
        r#"
[global]
remove_fixme = true

[patterns."**/*.py"]
remove_todos = true
"#,
    );

    let sibling_file = repo.join("other").join("x.py");
    write(&sibling_file, "# FIXME: tracked work\n");

    let manager = ConfigManager::new(repo.join("sub")).unwrap();
    let resolved = manager.get_config_for_file(repo.join("sub").join("..").join("other").join("x.py"));

    assert!(
        !resolved.remove_fixme,
        "sub/'s globals must not reach a sibling directory"
    );
    assert!(
        !resolved.remove_todos,
        "sub/'s pattern globs must not match a path outside sub/"
    );
}

/// The same arithmetic with a shallower target reaches above the git root, where the
/// upward walk is not allowed to look at all.
#[test]
fn a_parent_component_cannot_escape_the_git_root() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path().join("repo");
    fs::create_dir_all(repo.join(".git")).unwrap();

    write(
        &temp.path().join(".uncomment.toml"),
        r#"
[global]
remove_fixme = true
"#,
    );

    let outside_file = temp.path().join("x.py");
    write(&outside_file, "# FIXME: tracked work\n");

    let manager = ConfigManager::new(&repo).unwrap();
    let resolved = manager.get_config_for_file(repo.join("..").join("x.py"));

    assert!(
        !resolved.remove_fixme,
        "a config above the git root must stay out of reach even via `..`"
    );
}

/// `uncomment ./src` produces file paths carrying a `.` component; they must resolve
/// exactly as the clean spelling does.
#[test]
fn a_dot_component_resolves_to_the_same_config() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    write(
        &root.join(".uncomment.toml"),
        r#"
[global]
remove_fixme = true

[patterns."src/*.py"]
remove_todos = true
"#,
    );

    let clean = root.join("src").join("x.py");
    write(&clean, "# TODO: tracked work\n");
    let dotted = root.join(".").join("src").join("x.py");

    let manager = ConfigManager::new(root).unwrap();
    let clean_resolved = manager.get_config_for_file(&clean);
    let dotted_resolved = manager.get_config_for_file(&dotted);

    assert!(clean_resolved.remove_fixme && clean_resolved.remove_todos);
    assert_eq!(
        (dotted_resolved.remove_fixme, dotted_resolved.remove_todos),
        (clean_resolved.remove_fixme, clean_resolved.remove_todos),
        "a `.` component must not change the outcome"
    );
}

/// `uncomment ../other` makes the file's directory `repo/sub/../other`, whose lexical
/// parent is `repo/sub/..` — a cache miss against the ancestor config registered under
/// `repo`, which used to re-load that same file and then claim its `[languages]` were
/// ignored. Every `init` template has a `[languages]` section, so this was the common
/// case. (`./src` does not trigger it: `Path::parent` already elides `.`.)
#[test]
fn a_parent_component_argument_does_not_warn_that_languages_are_ignored() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path().join("repo");
    fs::create_dir_all(repo.join(".git")).unwrap();
    fs::create_dir_all(repo.join("sub")).unwrap();

    write(
        &repo.join(".uncomment.toml"),
        r#"
[global]
remove_todos = false

[languages.python]
name = "Python"
extensions = [".py"]
comment_nodes = ["comment"]
"#,
    );
    write(&repo.join("other").join("x.py"), "# a comment\nx = 1\n");

    let output = run_uncomment(&repo.join("sub"), &["../other", "--dry-run"]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !stderr.contains("is ignored"),
        "the repository config's languages are registered, so nothing should warn: {stderr}"
    );
}

/// The lazy path cannot return an error — `get_config_for_file` is infallible by
/// signature — so a config discovered below the root records one for the caller instead.
#[test]
fn a_broken_config_below_the_root_is_recorded_as_a_deferred_error() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    write(&root.join(".uncomment.toml"), "[global]\nremove_todos = false\n");
    write(
        &root.join("sub").join(".uncomment.toml"),
        "[global]\nremove_todoz = true\n",
    );
    let file = root.join("sub").join("x.py");
    write(&file, "# NOTE: keep me\nx = 1\n");

    let manager = ConfigManager::new(root).unwrap();
    assert!(
        manager.deferred_config_error().is_none(),
        "nothing has been resolved yet"
    );

    let _ = manager.get_config_for_file(&file);

    let error = manager
        .deferred_config_error()
        .expect("a config that fails to load must be recorded, not swallowed");
    assert!(
        error.contains(".uncomment.toml") && error.contains("remove_todoz"),
        "the recorded error must name the file and the key, got: {error}"
    );
}

/// This tool rewrites files in place. A discovered config it cannot parse must stop the
/// run, not silently fall back to built-in defaults and keep deleting comments.
#[test]
fn discovered_config_with_an_unknown_key_fails_the_run() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    write(
        &root.join(".uncomment.toml"),
        r#"
[global]
remove_todos = false
remove_todoz = true
"#,
    );
    let file = root.join("x.py");
    write(&file, "# NOTE: keep me\nx = 1\n");

    let output = run_uncomment(root, &["."]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "an unparsable discovered config must fail the run, stderr: {stderr}"
    );
    assert!(
        stderr.contains(".uncomment.toml"),
        "the error must name the offending file, got: {stderr}"
    );
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        "# NOTE: keep me\nx = 1\n",
        "no file may be rewritten when the config was rejected"
    );
}

/// The same guarantee for a config the ancestor walk never sees. This one is only read
/// during the per-file pass, from a call that cannot return an error, so the run has to
/// consult the recorded failure rather than finishing green.
#[test]
fn broken_config_below_the_invocation_directory_fails_the_run() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    write(&root.join(".uncomment.toml"), "[global]\nremove_todos = false\n");
    write(
        &root.join("sub").join(".uncomment.toml"),
        "[global]\nremove_todoz = true\n",
    );
    let above = root.join("x.py");
    let below = root.join("sub").join("y.py");
    write(&above, "# plain\nx = 1\n");
    write(&below, "# plain\ny = 1\n");

    let output = run_uncomment(root, &["."]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "a config discovered below the invocation directory must still fail the run, stderr: {stderr}"
    );
    assert!(
        stderr.contains("remove_todoz"),
        "the error must name the offending key, got: {stderr}"
    );
    assert_eq!(
        fs::read_to_string(&above).unwrap(),
        "# plain\nx = 1\n",
        "no file may be rewritten when a config was rejected, not even one the config never covered"
    );
    assert_eq!(
        fs::read_to_string(&below).unwrap(),
        "# plain\ny = 1\n",
        "the file the rejected config covered must be untouched"
    );
}

/// Which file name wins must not depend on whether the preferred one parses — neither of the
/// deprecated names may be promoted behind it.
#[test]
fn broken_dotfile_config_does_not_promote_a_deprecated_name() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    write(&root.join(".uncomment.toml"), "[global]\nremove_todoz = true\n");
    write(&root.join(".uncommentrc.toml"), "[global]\nremove_todos = true\n");
    write(&root.join("uncomment.toml"), "[global]\nremove_todos = true\n");
    write(&root.join("x.py"), "# TODO: tracked work\nx = 1\n");

    let output = run_uncomment(root, &["."]);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !output.status.success(),
        "a broken .uncomment.toml must fail rather than promote a deprecated name, stderr: {stderr}"
    );
    assert!(
        stderr.contains(".uncomment.toml"),
        "the error must name the file that failed, got: {stderr}"
    );
}

/// The eager ancestor walk is the one resolution path that can still return an error.
#[test]
fn construction_fails_when_an_ancestor_config_is_invalid() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path().join("repo");
    fs::create_dir_all(repo.join(".git")).unwrap();
    write(&repo.join(".uncomment.toml"), "[global]\nremove_todoz = true\n");
    fs::create_dir_all(repo.join("sub")).unwrap();

    let error = ConfigManager::new(repo.join("sub")).expect_err("an invalid ancestor config must be fatal");
    let rendered = format!("{error:#}");
    assert!(
        rendered.contains(".uncomment.toml") && rendered.contains("remove_todoz"),
        "the error must name the file and the offending key, got: {rendered}"
    );
}

/// An absent config is not an error.
#[test]
fn a_directory_without_any_config_is_not_an_error() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("sub").join("x.py");
    write(&file, "x = 1\n");

    let manager = ConfigManager::new(temp.path()).expect("no config is a valid state");
    assert!(!manager.get_config_for_file(&file).remove_todos);
}
