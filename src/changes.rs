//! Narrowing a run to what a branch changed — the machinery behind `--changed-only` and `--base`.
//!
//! `lint` and the removal command's `--check` both gate on it, and both need the same answer to
//! "which files did this branch touch", so the flags, the default base ref and the `git diff` call
//! live here rather than with either command.
//!
//! Unlike [`crate::git`], which reads `.git` directly, this has to shell out: what changed between two
//! commits is only knowable by diffing trees, and reimplementing that is not worth the dependency.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::paths::absolute_normalized;

/// Fallback base ref for `--changed-only` when the checkout does not record `origin/HEAD`.
const FALLBACK_BASE_REF: &str = "main";

/// The flags that narrow a run to what changed, shared by every command that offers them.
#[derive(clap::Args, Debug, Clone, Default)]
pub struct ChangeScopeArgs {
    /// Only files changed against the base ref
    #[arg(
        long = "changed-only",
        help = "Only files changed against --base",
        help_heading = "Change scope"
    )]
    pub changed_only: bool,

    /// Base ref for --changed-only; defaults to the remote's default branch, else `main`
    #[arg(
        long,
        value_name = "REF",
        help = "Base ref for --changed-only (default: origin's default branch)",
        help_heading = "Change scope"
    )]
    pub base: Option<String>,
}

impl ChangeScopeArgs {
    /// Whether any flag asks for the run to be narrowed.
    pub fn is_active(&self) -> bool {
        self.changed_only
    }
}

/// The resolved answer to "what changed": the files, and a description of what they were diffed
/// against for the run's notes.
#[derive(Debug, Clone)]
pub struct ChangeScope {
    files: HashSet<PathBuf>,
    description: String,
}

impl ChangeScope {
    /// Resolve `args` against the repository at `repo_root`, or `None` when no flag narrows the run.
    ///
    /// # Errors
    ///
    /// Fails when narrowing was asked for outside a git repository, or when `git diff` fails — an
    /// unknown base ref, most often. Either way the run cannot tell what changed, and checking
    /// nothing would pass a gate that should have failed.
    pub fn resolve(args: &ChangeScopeArgs, repo_root: Option<&Path>) -> Result<Option<Self>> {
        if !args.is_active() {
            return Ok(None);
        }
        let root = repo_root.context("--changed-only needs a git repository, and none encloses the base directory")?;
        let base_ref = match &args.base {
            Some(given) => given.clone(),
            None => default_base_ref(root),
        };
        Ok(Some(Self {
            files: changed_files(root, &base_ref)?,
            description: format!("--changed-only against {base_ref}"),
        }))
    }

    /// Whether `file`, absolute and normalized, is one the diff touched.
    pub fn contains_file(&self, file: &Path) -> bool {
        self.files.contains(file)
    }

    /// The note a run prints once it has narrowed `total` candidate files down to `kept`.
    pub fn note(&self, kept: usize, total: usize) -> String {
        format!("{}: {kept} of {total} file(s) changed", self.description)
    }
}

/// `origin`'s default branch as recorded in the checkout, else [`FALLBACK_BASE_REF`].
///
/// Read out of `.git` rather than asked of `git remote show`, which goes to the network. Never
/// `master`: a repository that does not record a default branch is far likelier to use `main`.
fn default_base_ref(repo_root: &Path) -> String {
    let Some(common) = git_common_dir(repo_root) else {
        return FALLBACK_BASE_REF.to_string();
    };

    fs::read_to_string(common.join("refs/remotes/origin/HEAD"))
        .ok()
        .and_then(|content| {
            let reference = content.trim().strip_prefix("ref:")?.trim();
            let branch = reference.strip_prefix("refs/remotes/origin/")?;
            (!branch.is_empty()).then(|| branch.to_string())
        })
        .unwrap_or_else(|| FALLBACK_BASE_REF.to_string())
}

/// The shared git directory: `.git`, following a worktree's `gitdir:` pointer and then its
/// `commondir`, because a linked worktree keeps no `refs/` of its own.
fn git_common_dir(repo_root: &Path) -> Option<PathBuf> {
    let dot_git = repo_root.join(".git");
    let metadata = fs::metadata(&dot_git).ok()?;

    let git_dir = if metadata.is_dir() {
        dot_git
    } else {
        let pointer = fs::read_to_string(&dot_git).ok()?;
        let pointer = pointer.trim().strip_prefix("gitdir:")?.trim();
        if pointer.is_empty() {
            return None;
        }
        absolute_normalized(repo_root, Path::new(pointer))
    };

    match fs::read_to_string(git_dir.join("commondir")) {
        Ok(common) => Some(absolute_normalized(&git_dir, Path::new(common.trim()))),
        Err(_) => Some(git_dir),
    }
}

/// Absolute paths of the files changed between `base`'s merge-base with `HEAD` and the working
/// tree — staged, unstaged and untracked (but not ignored) edits included.
///
/// Diffing `base...HEAD` — as this used to — compares two *commits*, so anything not yet committed
/// on the branch is invisible to `--changed-only`: a hook or a CI step run before the final commit
/// sees no violations in the very lines it exists to catch. Comparing the merge-base to the working
/// tree instead is exactly what `git diff base...HEAD`'s notation promises but does not do; `git
/// diff <merge-base>` (one ref, no `HEAD`) is git's own way of saying "against what's on disk".
fn changed_files(repo_root: &Path, base: &str) -> Result<HashSet<PathBuf>> {
    let merge_base = run_git(
        repo_root,
        &["merge-base", base, "HEAD"],
        &format!("merge-base {base} HEAD"),
    )?;
    let merge_base = merge_base.trim();

    let diffed = run_git(
        repo_root,
        &["diff", "--name-only", merge_base],
        &format!("diff --name-only {merge_base}"),
    )?;
    let untracked = run_git(
        repo_root,
        &["ls-files", "--others", "--exclude-standard"],
        "ls-files --others --exclude-standard",
    )?;

    Ok(diffed
        .lines()
        .chain(untracked.lines())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| absolute_normalized(repo_root, Path::new(line)))
        .collect())
}

/// Run a `git` subcommand in `repo_root` and return its stdout, or fail naming `description`.
fn run_git(repo_root: &Path, args: &[&str], description: &str) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .output()
        .with_context(|| format!("failed to run `git {description}`"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("`git {description}` failed: {}", stderr.trim());
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_base_ref_is_read_from_origin_head_and_never_master() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let root = temp.path();
        let refs = root.join(".git/refs/remotes/origin");
        fs::create_dir_all(&refs).expect("create refs");

        assert_eq!(default_base_ref(root), "main", "no record of origin's default branch");

        fs::write(refs.join("HEAD"), "ref: refs/remotes/origin/trunk\n").expect("write HEAD");
        assert_eq!(default_base_ref(root), "trunk");

        fs::write(refs.join("HEAD"), "ref: refs/remotes/origin/master\n").expect("write HEAD");
        assert_eq!(
            default_base_ref(root),
            "master",
            "a repository that really uses master is still honoured"
        );
    }

    #[test]
    fn a_worktree_resolves_refs_through_commondir() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let main_git = temp.path().join("main/.git");
        fs::create_dir_all(main_git.join("refs/remotes/origin")).expect("create refs");
        fs::create_dir_all(main_git.join("worktrees/wt")).expect("create worktree dir");
        fs::write(
            main_git.join("refs/remotes/origin/HEAD"),
            "ref: refs/remotes/origin/main\n",
        )
        .expect("write HEAD");
        fs::write(main_git.join("worktrees/wt/commondir"), "../..\n").expect("write commondir");

        let worktree = temp.path().join("wt");
        fs::create_dir_all(&worktree).expect("create worktree");
        fs::write(
            worktree.join(".git"),
            format!("gitdir: {}\n", main_git.join("worktrees/wt").display()),
        )
        .expect("write .git");

        assert_eq!(default_base_ref(&worktree), "main");
    }

    #[test]
    fn an_inactive_scope_resolves_to_none_even_outside_a_repository() {
        let scope = ChangeScope::resolve(&ChangeScopeArgs::default(), None).expect("resolve");
        assert!(scope.is_none(), "no flag set must never touch git");
    }

    #[test]
    fn narrowing_outside_a_repository_is_an_error() {
        let args = ChangeScopeArgs {
            changed_only: true,
            base: None,
        };
        let error = ChangeScope::resolve(&args, None).expect_err("no repository to diff");
        assert!(error.to_string().contains("needs a git repository"), "{error}");
    }
}
