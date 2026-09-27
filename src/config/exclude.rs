//! Paths a run refuses to collect: the `exclude` globs of every config loaded before collection,
//! plus the `--exclude` flags, compiled once into one set that all four file collectors share.
//!
//! The globs themselves are `super::file`'s dialect — see [`super::file::compile_path_glob`] — so
//! `exclude` and `[patterns."<glob>"]` keys are written the same way.

use super::file::compile_path_glob;
use crate::paths;
use anyhow::{Context, Result};
use globset::GlobMatcher;
use std::path::{Path, PathBuf};

/// `[global] exclude` as it is named in an error, so config validation and the set built from it
/// report a broken glob identically.
pub(super) const CONFIG_KEY: &str = "[global] exclude";

/// The path globs a candidate must not match to be collected.
///
/// # Anchoring
///
/// A glob is matched against the candidate path **relative to the directory the glob was declared
/// in** — the config file's own directory, exactly as `[patterns."<glob>"]` keys are, or the
/// invocation directory for `--exclude`. So `exclude = ["playground/**"]` in a repository root names
/// that repository's `playground` whatever directory the command was run from.
///
/// A candidate is made absolute against the invocation directory and lexically normalized before it
/// is matched, so `playground/a.py`, `./playground/a.py` and the absolute spelling are one path and
/// answer alike. A candidate lying *outside* the anchor directory is matched against its absolute
/// form instead, which is all a user-level config — anchored at the platform config directory — can
/// usefully match, and only with a `**/`-prefixed glob.
///
/// Every layer's globs form a single union: a nested config can add an exclusion, never withdraw one.
/// Naming an excluded path on the command line does not override it either, because `exclude` states
/// which files the project never wants rewritten rather than which ones this invocation skips.
#[derive(Debug, Clone)]
pub struct ExcludeSet {
    /// The directory a relative candidate is resolved against.
    base: PathBuf,
    rules: Vec<Rule>,
}

#[derive(Debug, Clone)]
struct Rule {
    /// The directory this glob is anchored at, already normalized.
    root: PathBuf,
    matcher: GlobMatcher,
    /// The same glob with a trailing `/**` removed. A rule written for a subtree also names the
    /// subtree's own directory, which is what lets a walk skip it outright instead of descending a
    /// vendored tree and discarding every entry.
    directory: Option<GlobMatcher>,
}

impl ExcludeSet {
    /// An empty set whose relative candidates resolve against `base` — the invocation directory.
    pub fn new(base: &Path) -> Self {
        Self {
            base: paths::normalize_lexical(base),
            rules: Vec::new(),
        }
    }

    /// Add `patterns`, anchored at `root`. `key` names the setting they were written in, so a glob
    /// that does not compile is reported against the place it came from.
    pub fn add(&mut self, root: &Path, patterns: &[String], key: &str) -> Result<()> {
        let root = paths::normalize_lexical(root);
        for pattern in patterns {
            let compile =
                |glob: &str| compile_path_glob(glob).with_context(|| format!("Invalid glob in {key}: {pattern:?}"));
            self.rules.push(Rule {
                root: root.clone(),
                matcher: compile(pattern)?,
                directory: pattern.strip_suffix("/**").map(compile).transpose()?,
            });
        }
        Ok(())
    }

    /// Whether `path` is excluded.
    pub fn is_excluded(&self, path: &Path) -> bool {
        self.matches(path, false)
    }

    /// Whether a walk may skip `dir` and everything below it.
    pub fn prunes_dir(&self, dir: &Path) -> bool {
        self.matches(dir, true)
    }

    /// Predicate for [`ignore::WalkBuilder::filter_entry`], which needs an owned `'static` closure.
    /// Pruning is an optimisation only: a collector still tests each entry it is handed, so a
    /// walker that descends anyway yields nothing excluded.
    pub fn walk_filter(&self) -> impl Fn(&ignore::DirEntry) -> bool + Send + Sync + 'static {
        let excludes = self.clone();
        move |entry| {
            if entry.file_type().is_some_and(|kind| kind.is_dir()) {
                !excludes.prunes_dir(entry.path())
            } else {
                !excludes.is_excluded(entry.path())
            }
        }
    }

    fn matches(&self, path: &Path, as_directory: bool) -> bool {
        if self.rules.is_empty() {
            return false;
        }

        let candidate = paths::absolute_normalized(&self.base, path);
        self.rules.iter().any(|rule| {
            let relative = paths::repo_relative(&rule.root, &candidate);
            let target = relative.as_deref().unwrap_or(&candidate);
            rule.matcher.is_match(target)
                || (as_directory && rule.directory.as_ref().is_some_and(|matcher| matcher.is_match(target)))
        })
    }
}

/// Reject any glob in `patterns` that does not compile, naming `key`.
///
/// Config validation calls this so a broken `exclude` stops the run when the file is read, rather
/// than when the first path happens to be tested against it.
pub(super) fn validate(patterns: &[String], key: &str) -> Result<()> {
    let mut set = ExcludeSet::new(Path::new(""));
    set.add(Path::new(""), patterns, key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(root: &str, patterns: &[&str]) -> ExcludeSet {
        let patterns: Vec<String> = patterns.iter().map(|pattern| (*pattern).to_string()).collect();
        let mut set = ExcludeSet::new(Path::new("/repo"));
        set.add(Path::new(root), &patterns, CONFIG_KEY).expect("compile globs");
        set
    }

    /// The whole reason candidates are normalized first: a caller that writes `./playground/a.py`
    /// must not get a different answer from one that writes `playground/a.py`.
    #[test]
    fn every_spelling_of_one_path_answers_alike() {
        let set = set("/repo", &["playground/**"]);

        for spelling in [
            "playground/a.py",
            "./playground/a.py",
            "/repo/playground/a.py",
            "/repo/./playground/a.py",
            "src/../playground/a.py",
        ] {
            assert!(set.is_excluded(Path::new(spelling)), "{spelling} should be excluded");
        }

        assert!(!set.is_excluded(Path::new("src/a.py")));
        assert!(!set.is_excluded(Path::new("/elsewhere/playground/a.py")));
    }

    #[test]
    fn a_subtree_rule_covers_every_depth_and_prunes_its_own_directory() {
        let set = set("/repo", &["playground/**"]);

        assert!(set.is_excluded(Path::new("playground/a.py")));
        assert!(set.is_excluded(Path::new("playground/deep/inner/a.py")));

        // `playground` itself does not match `playground/**`, so pruning needs the stripped form.
        assert!(!set.is_excluded(Path::new("playground")));
        assert!(set.prunes_dir(Path::new("playground")));
        assert!(!set.prunes_dir(Path::new("src")));
    }

    /// A glob is anchored at the directory it was written in, not at the invocation directory, so a
    /// repository-root config means that repository's `playground` and nothing else.
    #[test]
    fn a_glob_is_anchored_at_the_directory_it_was_declared_in() {
        let set = set("/repo/pkg", &["generated/**"]);

        assert!(set.is_excluded(Path::new("/repo/pkg/generated/a.py")));
        assert!(!set.is_excluded(Path::new("/repo/generated/a.py")));
    }

    /// A user-level config sits outside the tree being processed, so its anchor can never contain a
    /// candidate. Matching the absolute path instead is what keeps a `**/`-prefixed glob usable.
    #[test]
    fn a_candidate_outside_the_anchor_is_matched_as_an_absolute_path() {
        let set = set("/home/user/.config/uncomment", &["**/node_modules/**"]);

        assert!(set.is_excluded(Path::new("/repo/pkg/node_modules/left-pad/index.js")));
        assert!(!set.is_excluded(Path::new("/repo/pkg/src/index.js")));
    }

    #[test]
    fn a_glob_that_does_not_compile_names_the_setting_and_the_glob() {
        let mut set = ExcludeSet::new(Path::new("/repo"));
        let error = set
            .add(Path::new("/repo"), &["playground/[".to_string()], CONFIG_KEY)
            .expect_err("an unclosed character class is not a glob");

        let message = format!("{error:#}");
        assert!(message.contains(CONFIG_KEY), "{message}");
        assert!(message.contains("playground/["), "{message}");
    }

    #[test]
    fn an_empty_set_excludes_nothing() {
        let set = ExcludeSet::new(Path::new("/repo"));
        assert!(!set.is_excluded(Path::new("playground/a.py")));
        assert!(!set.prunes_dir(Path::new("playground")));
    }
}
