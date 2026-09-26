//! Reading the current branch, and the issue key embedded in it, straight from `.git`.
//!
//! `lint` needs the issue key of the branch being worked on, because a `TODO(KEY)` naming the
//! current branch's own issue is a bug: that issue closes when the pull request merges, so the TODO
//! ends up pointing at a closed ticket and the work it describes becomes invisible.
//!
//! `HEAD` is parsed directly instead of shelling out to `git` — no `PATH` dependency, no process
//! spawn per invocation, and fixtures are a couple of files in a temp directory. Every failure is a
//! `None`: not being in a repository, or being on a detached `HEAD`, is normal and must not abort a
//! run that has real work to report.

use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;

use crate::paths::absolute_normalized;

/// Current branch name, or `None` when detached, or not in a git repository.
pub fn current_branch(repo_root: &Path) -> Option<String> {
    let head = head_path(repo_root)?;
    let content = fs::read_to_string(head).ok()?;
    parse_head(&content)
}

/// The first capture of `pattern` applied to the current branch name.
///
/// Falls back to the whole match for a pattern with no capture group, so a bare `[A-Z]+-\d+` works
/// as well as the parenthesised form.
pub fn current_issue_key(repo_root: &Path, pattern: &Regex) -> Option<String> {
    let branch = current_branch(repo_root)?;
    let captures = pattern.captures(&branch)?;
    captures
        .get(1)
        .or_else(|| captures.get(0))
        .map(|m| m.as_str().to_string())
}

/// Locate `HEAD` for the repository rooted at `repo_root`.
///
/// In a linked worktree `.git` is a file holding `gitdir: <path>` pointing at
/// `…/.git/worktrees/<name>`, which has its own `HEAD`. The pointer may be relative, in which case
/// it resolves against the directory holding the `.git` file.
fn head_path(repo_root: &Path) -> Option<PathBuf> {
    let dot_git = repo_root.join(".git");
    let metadata = fs::metadata(&dot_git).ok()?;

    if metadata.is_dir() {
        return Some(dot_git.join("HEAD"));
    }

    let pointer = fs::read_to_string(&dot_git).ok()?;
    let pointer = pointer.trim().strip_prefix("gitdir:")?.trim();
    if pointer.is_empty() {
        return None;
    }

    Some(absolute_normalized(repo_root, Path::new(pointer)).join("HEAD"))
}

/// A branch name from `HEAD` contents, or `None` for a detached or unrecognised `HEAD`.
///
/// Only `refs/heads/` is stripped; the remainder is the branch name verbatim, slashes included, so
/// `feat/foo/bar` survives intact.
fn parse_head(content: &str) -> Option<String> {
    let reference = content.trim().strip_prefix("ref:")?.trim();
    let branch = reference.strip_prefix("refs/heads/")?;
    if branch.is_empty() {
        None
    } else {
        Some(branch.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn repo_with_head(contents: &str) -> TempDir {
        let temp = TempDir::new().unwrap();
        let git_dir = temp.path().join(".git");
        fs::create_dir_all(&git_dir).unwrap();
        fs::write(git_dir.join("HEAD"), contents).unwrap();
        temp
    }

    #[test]
    fn reads_the_branch_name_from_a_plain_repository() {
        let repo = repo_with_head("ref: refs/heads/master\n");
        assert_eq!(current_branch(repo.path()).as_deref(), Some("master"));
    }

    #[test]
    fn a_branch_name_keeps_its_slashes() {
        let repo = repo_with_head("ref: refs/heads/feat/foo/bar\n");
        assert_eq!(current_branch(repo.path()).as_deref(), Some("feat/foo/bar"));
    }

    #[test]
    fn trailing_whitespace_is_trimmed() {
        let repo = repo_with_head("ref: refs/heads/topic  \n\n");
        assert_eq!(current_branch(repo.path()).as_deref(), Some("topic"));
    }

    #[test]
    fn a_detached_head_has_no_branch() {
        let repo = repo_with_head("1077b28f9a4c5d6e7f8091a2b3c4d5e6f7089a1b\n");
        assert_eq!(current_branch(repo.path()), None);
    }

    #[test]
    fn a_non_branch_reference_has_no_branch() {
        let repo = repo_with_head("ref: refs/tags/v3.7.0\n");
        assert_eq!(current_branch(repo.path()), None);
    }

    #[test]
    fn unrecognised_head_contents_yield_none() {
        for contents in ["", "   \n", "garbage", "ref:\n", "ref: refs/heads/\n"] {
            let repo = repo_with_head(contents);
            assert_eq!(current_branch(repo.path()), None, "contents {contents:?}");
        }
    }

    #[test]
    fn a_missing_or_unreadable_git_entry_yields_none() {
        let temp = TempDir::new().unwrap();
        assert_eq!(current_branch(temp.path()), None);

        // `.git` exists as a directory but holds no HEAD.
        fs::create_dir_all(temp.path().join(".git")).unwrap();
        assert_eq!(current_branch(temp.path()), None);
    }

    #[test]
    fn a_worktree_git_file_with_an_absolute_pointer_is_followed() {
        let temp = TempDir::new().unwrap();
        let worktree_git_dir = temp.path().join("main/.git/worktrees/wt");
        fs::create_dir_all(&worktree_git_dir).unwrap();
        fs::write(worktree_git_dir.join("HEAD"), "ref: refs/heads/feat/linked\n").unwrap();

        let worktree = temp.path().join("wt");
        fs::create_dir_all(&worktree).unwrap();
        fs::write(
            worktree.join(".git"),
            format!("gitdir: {}\n", worktree_git_dir.display()),
        )
        .unwrap();

        assert_eq!(current_branch(&worktree).as_deref(), Some("feat/linked"));
    }

    #[test]
    fn a_worktree_git_file_with_a_relative_pointer_resolves_against_the_worktree() {
        let temp = TempDir::new().unwrap();
        let worktree_git_dir = temp.path().join("main/.git/worktrees/wt");
        fs::create_dir_all(&worktree_git_dir).unwrap();
        fs::write(worktree_git_dir.join("HEAD"), "ref: refs/heads/relative/pointer\n").unwrap();

        let worktree = temp.path().join("wt");
        fs::create_dir_all(&worktree).unwrap();
        fs::write(worktree.join(".git"), "gitdir: ../main/.git/worktrees/wt\n").unwrap();

        assert_eq!(current_branch(&worktree).as_deref(), Some("relative/pointer"));
    }

    #[test]
    fn a_dangling_or_malformed_git_file_yields_none() {
        let temp = TempDir::new().unwrap();
        let worktree = temp.path().join("wt");
        fs::create_dir_all(&worktree).unwrap();

        for contents in ["gitdir: /nowhere/at/all\n", "gitdir:\n", "not a pointer\n"] {
            fs::write(worktree.join(".git"), contents).unwrap();
            assert_eq!(current_branch(&worktree), None, "contents {contents:?}");
        }
    }

    #[test]
    fn the_issue_key_is_the_first_capture_of_the_pattern() {
        let repo = repo_with_head("ref: refs/heads/naaman.AMVP-160815.ai-rulez-migration\n");
        let pattern = Regex::new(r"([A-Z][A-Z0-9]+-\d+)").unwrap();
        assert_eq!(current_issue_key(repo.path(), &pattern).as_deref(), Some("AMVP-160815"));
    }

    #[test]
    fn a_pattern_without_a_capture_group_falls_back_to_the_whole_match() {
        let repo = repo_with_head("ref: refs/heads/feat/PPSC-42-comment-inventory\n");
        let pattern = Regex::new(r"[A-Z][A-Z0-9]+-\d+").unwrap();
        assert_eq!(current_issue_key(repo.path(), &pattern).as_deref(), Some("PPSC-42"));
    }

    #[test]
    fn a_branch_without_an_issue_key_has_no_key() {
        let repo = repo_with_head("ref: refs/heads/master\n");
        let pattern = Regex::new(r"([A-Z][A-Z0-9]+-\d+)").unwrap();
        assert_eq!(current_issue_key(repo.path(), &pattern), None);

        let detached = repo_with_head("1077b28f9a4c5d6e7f8091a2b3c4d5e6f7089a1b\n");
        assert_eq!(current_issue_key(detached.path(), &pattern), None);
    }
}
