//! Narrowing a run to what a branch changed — the machinery behind `--changed-only`,
//! `--changed-lines`, `--staged` and `--base`.
//!
//! `lint` and the removal command's `--check` both gate on it, and both need the same answer to
//! "what did this branch touch", so the flags, the default base ref and the `git diff` calls live
//! here rather than with either command.
//!
//! File scope is what a clean codebase needs. Line scope is what makes a policy adoptable on one
//! that is not: a legacy file with fifty existing comments fails a file-scoped gate the first time
//! anyone edits it, while a line-scoped gate asks only about the lines the edit touched.
//!
//! Unlike [`crate::git`], which reads `.git` directly, this has to shell out: what changed between two
//! commits is only knowable by diffing trees, and reimplementing that is not worth the dependency.

use std::collections::HashMap;
use std::fs;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::paths::absolute_normalized;

/// Fallback base ref for `--changed-only` when the checkout does not record `origin/HEAD`.
const FALLBACK_BASE_REF: &str = "main";

/// The prefix git puts on the new side of a diff header, forced so a `diff.noprefix` or
/// `diff.mnemonicPrefix` setting cannot change what is parsed.
const NEW_SIDE_PREFIX: &str = "b/";

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

    /// Only comments on lines changed against the base ref
    #[arg(
        long = "changed-lines",
        help = "Only report comments on lines changed against --base (implies --changed-only)",
        help_heading = "Change scope"
    )]
    pub changed_lines: bool,

    /// Diff the index against `HEAD` instead of the base ref against `HEAD`
    #[arg(
        long,
        conflicts_with = "base",
        help = "Compare staged changes against HEAD instead of --base (for pre-commit hooks)",
        help_heading = "Change scope"
    )]
    pub staged: bool,

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
        self.changed_only || self.changed_lines || self.staged
    }

    /// The flag to name in a message about the scope as a whole.
    fn flag(&self) -> &'static str {
        if self.changed_lines {
            "--changed-lines"
        } else if self.changed_only || !self.staged {
            "--changed-only"
        } else {
            "--staged"
        }
    }
}

/// What a diff is taken between.
#[derive(Debug, Clone, PartialEq, Eq)]
enum DiffSource {
    /// The merge base of the ref and `HEAD`, against the working tree — staged, unstaged and
    /// untracked (but not ignored) edits included: what a branch changed, uncommitted edits too.
    ///
    /// Diffing `base...HEAD` — as this used to — compares two *commits*, so anything not yet
    /// committed on the branch is invisible: a hook or a CI step run before the final commit sees no
    /// violations in the very lines it exists to catch. Comparing the merge-base to the working tree
    /// instead is exactly what `base...HEAD`'s notation promises but does not do.
    Range(String),
    /// `HEAD` against the index: what the next commit changes.
    Staged,
}

impl DiffSource {
    /// Resolve to git's own single-argument way of saying what to diff against: the range's merge
    /// base with `HEAD` (a single ref diffs against the working tree, not another commit), or
    /// `--cached` for the index.
    fn diff_arg(&self, repo_root: &Path) -> Result<String> {
        match self {
            Self::Range(base) => {
                let merge_base = run_git(
                    repo_root,
                    &["merge-base", base, "HEAD"],
                    &format!("merge-base {base} HEAD"),
                )?;
                Ok(merge_base.trim().to_string())
            }
            Self::Staged => Ok("--cached".to_string()),
        }
    }

    /// Whether untracked (but not ignored) files are in scope: yes against the working tree, never
    /// for the index, which cannot hold a file that was never `git add`ed.
    fn includes_untracked(&self) -> bool {
        matches!(self, Self::Range(_))
    }

    fn describe(&self) -> String {
        match self {
            Self::Range(base) => base.clone(),
            Self::Staged => "the index".to_string(),
        }
    }
}

/// The resolved answer to "what changed": the files, their changed lines when the scope is per line,
/// and a description of what they were diffed against for the run's notes.
#[derive(Debug, Clone)]
pub struct ChangeScope {
    /// Absolute, normalized paths. The value is `None` in file scope, where lines are not asked.
    files: HashMap<PathBuf, Option<Vec<RangeInclusive<usize>>>>,
    per_line: bool,
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
        let flag = args.flag();
        let root = repo_root
            .with_context(|| format!("{flag} needs a git repository, and none encloses the base directory"))?;
        let source = if args.staged {
            DiffSource::Staged
        } else {
            DiffSource::Range(args.base.clone().unwrap_or_else(|| default_base_ref(root)))
        };
        let files = if args.changed_lines {
            changed_lines(root, &source)?
                .into_iter()
                .map(|(path, lines)| (path, Some(lines)))
                .collect()
        } else {
            changed_files(root, &source)?
                .into_iter()
                .map(|path| (path, None))
                .collect()
        };
        Ok(Some(Self {
            files,
            per_line: args.changed_lines,
            description: format!("{flag} against {}", source.describe()),
        }))
    }

    /// Whether `file`, absolute and normalized, is one the diff touched.
    pub fn contains_file(&self, file: &Path) -> bool {
        self.files.contains_key(file)
    }

    /// Whether anything on lines `first..=last` (1-based) of `file` is in scope: in file scope, any
    /// line of a changed file; in line scope, a line the diff added or rewrote.
    pub fn touches(&self, file: &Path, first: usize, last: usize) -> bool {
        match self.files.get(file) {
            None => false,
            Some(None) => true,
            Some(Some(ranges)) => ranges
                .iter()
                .any(|range| *range.start() <= last && first <= *range.end()),
        }
    }

    /// Whether the scope is narrower than whole files.
    pub fn is_per_line(&self) -> bool {
        self.per_line
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

/// Absolute paths of files not tracked anywhere but not ignored either — what a diff against a
/// single ref never reports (there is no earlier version to diff), yet the working tree still
/// counts as changed.
fn untracked_files(repo_root: &Path) -> Result<Vec<PathBuf>> {
    let output = run_git(
        repo_root,
        &["ls-files", "--others", "--exclude-standard", "-z"],
        "ls-files --others --exclude-standard -z",
    )?;
    Ok(output
        .split('\0')
        .filter(|name| !name.is_empty())
        .map(|name| absolute_normalized(repo_root, Path::new(name)))
        .collect())
}

/// The number of lines in a file on disk: newline-terminated lines, plus one more when the last line
/// has no trailing newline. `0` for a missing or empty file.
fn line_count(path: &Path) -> usize {
    let Ok(content) = fs::read(path) else {
        return 0;
    };
    if content.is_empty() {
        return 0;
    }
    let newlines = content.iter().filter(|&&byte| byte == b'\n').count();
    if content.ends_with(b"\n") { newlines } else { newlines + 1 }
}

/// Run `git diff` in `repo_root` with `args` after the source's own, returning its stdout.
fn git_diff(repo_root: &Path, source: &DiffSource, args: &[&str]) -> Result<Vec<u8>> {
    let diff_arg = source.diff_arg(repo_root)?;
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(repo_root)
        .args(["-c", "core.quotePath=false", "diff", "--no-color", "--no-ext-diff"])
        .args(args)
        .arg(&diff_arg);
    let rendered = format!("git diff {} {diff_arg}", args.join(" "));
    let output = command.output().with_context(|| format!("failed to run `{rendered}`"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("`{rendered}` failed: {}", stderr.trim());
    }
    Ok(output.stdout)
}

/// Absolute paths of the files the diff touches, plus untracked files when the source diffs against
/// the working tree.
fn changed_files(repo_root: &Path, source: &DiffSource) -> Result<Vec<PathBuf>> {
    let stdout = git_diff(repo_root, source, &["--name-only", "-z"])?;
    let mut files: Vec<PathBuf> = stdout
        .split(|&byte| byte == 0)
        .filter(|name| !name.is_empty())
        .map(|name| absolute_normalized(repo_root, Path::new(&*String::from_utf8_lossy(name))))
        .collect();
    if source.includes_untracked() {
        files.extend(untracked_files(repo_root)?);
    }
    Ok(files)
}

/// Absolute paths of the files the diff touches, each with the new-side lines it added or rewrote.
/// An untracked file — in scope only when the source diffs against the working tree — is present
/// with every one of its lines: none of it has ever been reviewed, so all of it counts as changed.
fn changed_lines(repo_root: &Path, source: &DiffSource) -> Result<HashMap<PathBuf, Vec<RangeInclusive<usize>>>> {
    let stdout = git_diff(
        repo_root,
        source,
        &["--unified=0", "--src-prefix=a/", "--dst-prefix=b/"],
    )?;
    let mut files: HashMap<PathBuf, Vec<RangeInclusive<usize>>> = parse_unified_zero(&String::from_utf8_lossy(&stdout))
        .into_iter()
        .map(|(path, lines)| (absolute_normalized(repo_root, Path::new(&path)), lines))
        .collect();

    if source.includes_untracked() {
        for path in untracked_files(repo_root)? {
            let ranges = match line_count(&path) {
                0 => Vec::new(),
                count => vec![1..=count],
            };
            files.insert(path, ranges);
        }
    }
    Ok(files)
}

/// The new-side line ranges of a `git diff --unified=0`, keyed by repo-relative path.
///
/// A file whose diff adds no line — a pure deletion inside it — is present with no ranges: it
/// changed, but nothing in it is new. A deleted file (`+++ /dev/null`) is absent.
fn parse_unified_zero(diff: &str) -> HashMap<String, Vec<RangeInclusive<usize>>> {
    let mut files: HashMap<String, Vec<RangeInclusive<usize>>> = HashMap::new();
    let mut current: Option<String> = None;

    for line in diff.lines() {
        if let Some(target) = line.strip_prefix("+++ ") {
            current = unquote(target).strip_prefix(NEW_SIDE_PREFIX).map(str::to_string);
            if let Some(path) = &current {
                files.entry(path.clone()).or_default();
            }
        } else if line.starts_with("@@")
            && let (Some(path), Some(range)) = (&current, parse_hunk_new_side(line))
        {
            files.entry(path.clone()).or_default().push(range);
        }
    }
    files
}

/// The new-side lines of a hunk header, `@@ -a[,b] +c[,d] @@`, or `None` when it adds none.
fn parse_hunk_new_side(header: &str) -> Option<RangeInclusive<usize>> {
    let new_side = header.split_whitespace().nth(2)?.strip_prefix('+')?;
    let (start, count) = match new_side.split_once(',') {
        Some((start, count)) => (start.parse::<usize>().ok()?, count.parse::<usize>().ok()?),
        None => (new_side.parse::<usize>().ok()?, 1),
    };
    (count > 0).then(|| start..=start + count - 1)
}

/// A diff header path, with git's C-style quoting undone when it is quoted.
///
/// `core.quotePath=false` leaves non-ASCII names alone, but a name holding a quote, a backslash or a
/// control character is still quoted.
fn unquote(raw: &str) -> String {
    let Some(inner) = raw.strip_prefix('"').and_then(|rest| rest.strip_suffix('"')) else {
        return raw.to_string();
    };

    let mut bytes = Vec::with_capacity(inner.len());
    let mut chars = inner.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            let mut buffer = [0u8; 4];
            bytes.extend_from_slice(ch.encode_utf8(&mut buffer).as_bytes());
            continue;
        }
        match chars.next() {
            Some('n') => bytes.push(b'\n'),
            Some('t') => bytes.push(b'\t'),
            Some('r') => bytes.push(b'\r'),
            Some(digit @ '0'..='7') => {
                let mut value = digit.to_digit(8).unwrap_or_default();
                for _ in 0..2 {
                    if let Some(next) = chars.peek().and_then(|c| c.to_digit(8)) {
                        value = value * 8 + next;
                        chars.next();
                    }
                }
                bytes.push(u8::try_from(value).unwrap_or(u8::MAX));
            }
            Some(other) => {
                let mut buffer = [0u8; 4];
                bytes.extend_from_slice(other.encode_utf8(&mut buffer).as_bytes());
            }
            None => bytes.push(b'\\'),
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ranges(pairs: &[(usize, usize)]) -> Vec<RangeInclusive<usize>> {
        pairs.iter().map(|&(start, end)| start..=end).collect()
    }

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
            ..ChangeScopeArgs::default()
        };
        let error = ChangeScope::resolve(&args, None).expect_err("no repository to diff");
        assert!(error.to_string().contains("needs a git repository"), "{error}");
    }

    #[test]
    fn a_hunk_header_yields_its_new_side_lines() {
        assert_eq!(parse_hunk_new_side("@@ -3,2 +5,4 @@ fn main() {"), Some(5..=8));
        assert_eq!(
            parse_hunk_new_side("@@ -3 +7 @@"),
            Some(7..=7),
            "an omitted count is one line"
        );
        assert_eq!(
            parse_hunk_new_side("@@ -3,2 +2,0 @@"),
            None,
            "a pure deletion adds no line"
        );
        assert_eq!(parse_hunk_new_side("@@ garbage @@"), None);
    }

    #[test]
    fn a_unified_zero_diff_is_keyed_by_new_path() {
        let diff = "\
diff --git a/src/edited.rs b/src/edited.rs
index 1111111..2222222 100644
--- a/src/edited.rs
+++ b/src/edited.rs
@@ -2,0 +3,2 @@ fn a() {
+    // one
+    // two
@@ -10 +12 @@ fn b() {
-old
+new
diff --git a/src/shrunk.rs b/src/shrunk.rs
--- a/src/shrunk.rs
+++ b/src/shrunk.rs
@@ -4,2 +3,0 @@
-gone
-gone
diff --git a/src/removed.rs b/src/removed.rs
deleted file mode 100644
--- a/src/removed.rs
+++ /dev/null
@@ -1 +0,0 @@
-bye
diff --git a/src/new file.rs b/src/new file.rs
new file mode 100644
--- /dev/null
+++ \"b/src/new\\tfile.rs\"
@@ -0,0 +1,3 @@
+a
+b
+c
";
        let parsed = parse_unified_zero(diff);

        assert_eq!(parsed.get("src/edited.rs"), Some(&ranges(&[(3, 4), (12, 12)])));
        assert_eq!(
            parsed.get("src/shrunk.rs"),
            Some(&Vec::new()),
            "changed, but nothing new"
        );
        assert!(!parsed.contains_key("src/removed.rs"), "a deleted file has no new side");
        assert_eq!(parsed.get("src/new\tfile.rs"), Some(&ranges(&[(1, 3)])), "{parsed:?}");
    }

    #[test]
    fn a_quoted_path_is_unquoted() {
        assert_eq!(unquote("b/plain.rs"), "b/plain.rs");
        assert_eq!(unquote(r#""b/say \"hi\".rs""#), "b/say \"hi\".rs");
        assert_eq!(unquote(r#""b/caf\303\251.rs""#), "b/café.rs");
        assert_eq!(unquote(r#""b/back\\slash.rs""#), "b/back\\slash.rs");
    }

    #[test]
    fn line_scope_intersects_a_comment_span_with_the_changed_lines() {
        let file = PathBuf::from("/repo/a.rs");
        let scope = ChangeScope {
            files: HashMap::from([(file.clone(), Some(ranges(&[(5, 7)])))]),
            per_line: true,
            description: String::new(),
        };

        assert!(scope.touches(&file, 7, 7));
        assert!(scope.touches(&file, 1, 5), "a block comment ending on a changed line");
        assert!(scope.touches(&file, 6, 20));
        assert!(!scope.touches(&file, 8, 9));
        assert!(!scope.touches(&file, 1, 4));
        assert!(!scope.touches(Path::new("/repo/other.rs"), 5, 5));
        assert!(scope.is_per_line());
    }

    #[test]
    fn file_scope_touches_every_line_of_a_changed_file() {
        let file = PathBuf::from("/repo/a.rs");
        let scope = ChangeScope {
            files: HashMap::from([(file.clone(), None)]),
            per_line: false,
            description: String::new(),
        };

        assert!(scope.touches(&file, 1, 1));
        assert!(scope.touches(&file, 1000, 1000));
        assert!(!scope.is_per_line());
    }
}
