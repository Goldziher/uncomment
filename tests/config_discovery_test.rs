use std::fs;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use uncomment::config::ConfigManager;

const DECOY_DIR_COUNT: usize = 2000;

fn write(path: &std::path::Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

/// A subtree the root config cannot possibly be affected by must never be walked.
/// The decoy config registers a custom language, which is observable through
/// `get_all_languages()` if — and only if — the subtree was descended into.
#[test]
fn construction_does_not_descend_below_the_root_directory() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();

    write(
        &root.join(".uncommentrc.toml"),
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
        &decoy_root.join("branch0").join("leaf0").join(".uncommentrc.toml"),
        r#"
[languages.decoylang]
name = "DecoyLang"
extensions = [".decoy"]
comment_nodes = ["comment"]
"#,
    );

    let start = Instant::now();
    let manager = ConfigManager::new(root).unwrap();
    let elapsed = start.elapsed();
    println!("ConfigManager::new over a {DECOY_DIR_COUNT}-directory decoy tree took {elapsed:?}");

    assert!(
        !manager.get_all_languages().contains_key("decoylang"),
        "a config below the root must not be discovered at construction time"
    );
    assert!(
        elapsed < Duration::from_millis(200),
        "construction should not scale with tree size, took {elapsed:?}"
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
        &repo.join(".uncommentrc.toml"),
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
        &outer.join(".uncommentrc.toml"),
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
        &root.join(".uncommentrc.toml"),
        r#"
[global]
remove_todos = false
"#,
    );
    write(
        &root.join("sub").join(".uncommentrc.toml"),
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
        &root.join(".uncommentrc.toml"),
        r#"
[global]
remove_todos = false
"#,
    );
    write(
        &root.join("sub").join(".uncommentrc.toml"),
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
