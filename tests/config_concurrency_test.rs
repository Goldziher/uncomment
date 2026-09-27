//! `ConfigManager` hands one `&self` to every rayon worker and fills its caches behind
//! `RwLock`s while they read. Nothing else in the suite exercises that, so these tests
//! resolve a few hundred files across differing configs from the real parallel path.

use rayon::prelude::*;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;
use uncomment::config::ConfigManager;
use uncomment::processor::Processor;

const GROUPS: usize = 10;
const FILES_PER_DIR: usize = 15;

struct Expectation {
    path: PathBuf,
    remove_todos: bool,
    remove_docs: bool,
    preserve_patterns: Vec<String>,
}

fn write(path: &std::path::Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

/// Groups alternate between overriding the globals and saying nothing about them, so a
/// leaked value from a neighbouring group is visible in either direction.
fn build_tree(root: &std::path::Path) -> Vec<Expectation> {
    write(
        &root.join(".uncomment.toml"),
        r#"
[global]
remove_todos = false
remove_docs = false
preserve_patterns = ["ROOT"]
"#,
    );

    let mut expectations = Vec::new();
    for group in 0..GROUPS {
        let group_dir = root.join(format!("group{group}"));
        let overrides_globals = group % 2 == 0;

        if overrides_globals {
            write(
                &group_dir.join(".uncomment.toml"),
                &format!(
                    r#"
[global]
remove_todos = true
preserve_patterns = ["G{group}"]
"#
                ),
            );
        } else {
            write(
                &group_dir.join(".uncomment.toml"),
                r#"
[patterns."deep/*.py"]
remove_docs = true
preserve_patterns = []
"#,
            );
        }

        for sub in ["deep", "flat"] {
            for index in 0..FILES_PER_DIR {
                let path = group_dir.join(sub).join(format!("file{index}.py"));
                write(
                    &path,
                    "# TODO: tracked work\ndef f():\n    \"\"\"Doc.\"\"\"\n    pass\n",
                );

                let deep = sub == "deep";
                expectations.push(Expectation {
                    path,
                    remove_todos: overrides_globals,
                    remove_docs: !overrides_globals && deep,
                    preserve_patterns: match (overrides_globals, deep) {
                        (true, _) => vec![format!("G{group}"), "ROOT".to_string()],
                        (false, true) => Vec::new(),
                        (false, false) => vec!["ROOT".to_string()],
                    },
                });
            }
        }
    }

    expectations
}

#[test]
fn parallel_resolution_agrees_with_sequential_resolution() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let expectations = build_tree(root);
    assert_eq!(expectations.len(), GROUPS * FILES_PER_DIR * 2);

    // A fresh manager per iteration means the caches start cold and the lazily discovered
    // group configs are raced for by every worker.
    for iteration in 0..4 {
        let manager = ConfigManager::new(root).unwrap();

        expectations.par_iter().for_each(|expected| {
            let resolved = manager.get_config_for_file(&expected.path);
            let display = expected.path.strip_prefix(root).unwrap().display();

            assert_eq!(
                resolved.remove_todos, expected.remove_todos,
                "iteration {iteration}: remove_todos for {display}"
            );
            assert_eq!(
                resolved.remove_docs, expected.remove_docs,
                "iteration {iteration}: remove_docs for {display}"
            );
            assert_eq!(
                resolved.preserve_patterns, expected.preserve_patterns,
                "iteration {iteration}: preserve_patterns for {display}"
            );
        });

        assert!(manager.deferred_config_error().is_none());
    }
}

/// The same tree through `Processor`, which is how the binary reaches the config: one
/// processor per file, all sharing the manager.
#[test]
fn parallel_processing_applies_the_config_of_each_directory() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let expectations = build_tree(root);
    let manager = ConfigManager::new(root).unwrap();

    expectations.par_iter().take(60).for_each(|expected| {
        let mut processor = Processor::new_with_config(&manager);
        let processed = processor
            .process_file_with_config(&expected.path, &manager, None)
            .unwrap_or_else(|e| panic!("processing {} failed: {e:#}", expected.path.display()));

        assert_eq!(
            !processed.processed_content.contains("TODO: tracked work"),
            expected.remove_todos,
            "TODO handling for {}",
            expected.path.strip_prefix(root).unwrap().display()
        );
    });
}
