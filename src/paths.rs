//! Path helpers shared by config resolution and the comment-inventory commands.
//!
//! Everything here is lexical unless the name says otherwise: no `canonicalize`, no symlink
//! resolution, no filesystem access. That matters because `Path::starts_with` and `Path::parent`
//! are themselves lexical, so a path carrying a literal `..` silently defeats any containment
//! check written against it.

use std::path::{Component, Path, PathBuf};

/// Resolve `.` and `..` textually, without touching the filesystem.
///
/// A leading `..` on a relative path is kept (there is nothing to pop), and `..` directly below a
/// root is dropped, matching the kernel's treatment of `/..` as `/`. Unlike `canonicalize` this
/// never fails and never follows symlinks, so `a/symlink/../b` normalizes to `a/b` even when that
/// is not where the kernel would land — acceptable for containment checks, not for opening files.
pub fn normalize_lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => out.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => match out.components().next_back() {
                Some(Component::Normal(_)) => {
                    out.pop();
                }
                Some(Component::RootDir | Component::Prefix(_)) => {}
                _ => out.push(".."),
            },
            Component::Normal(part) => out.push(part),
        }
    }

    out
}

/// Make `path` absolute against `base` and normalize it in one step.
pub fn absolute_normalized(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        normalize_lexical(path)
    } else {
        normalize_lexical(&base.join(path))
    }
}

/// Whether `candidate` is `ancestor` or sits underneath it.
///
/// Both sides are normalized first. If the lexical answer is `false` the check is retried against
/// canonicalized forms, because a user-supplied `/tmp/x` and a `current_dir()` of `/private/tmp/x`
/// name the same directory on macOS and must not be treated as unrelated. The retry is best-effort:
/// if either side cannot be canonicalized the lexical answer stands.
pub fn is_ancestor_of(ancestor: &Path, candidate: &Path) -> bool {
    let ancestor_norm = normalize_lexical(ancestor);
    let candidate_norm = normalize_lexical(candidate);

    if candidate_norm.starts_with(&ancestor_norm) {
        return true;
    }

    match (ancestor_norm.canonicalize(), candidate_norm.canonicalize()) {
        (Ok(a), Ok(c)) => c.starts_with(a),
        _ => false,
    }
}

/// Nearest ancestor of `start` (inclusive) containing a `.git` entry.
///
/// `.git` is tested with `exists()` rather than `is_dir()` so that a linked worktree, where `.git`
/// is a file holding a `gitdir:` pointer, counts as a repository root.
pub fn find_repo_root(start: &Path) -> Option<PathBuf> {
    let start = normalize_lexical(start);
    start
        .ancestors()
        .find(|dir| dir.join(".git").exists())
        .map(Path::to_path_buf)
}

/// `path` expressed relative to `root`, or `None` when it lies outside.
pub fn repo_relative(root: &Path, path: &Path) -> Option<PathBuf> {
    let root = normalize_lexical(root);
    let path = normalize_lexical(path);
    path.strip_prefix(&root).ok().map(Path::to_path_buf)
}

/// Render a path with `/` separators so ids and glob matches are identical across platforms.
pub fn to_slash(path: &Path) -> String {
    let rendered = path.to_string_lossy();
    if std::path::MAIN_SEPARATOR == '/' {
        rendered.into_owned()
    } else {
        rendered.replace(std::path::MAIN_SEPARATOR, "/")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_resolves_dot_and_parent() {
        let cases = [
            ("/repo/sub/../other/x.py", "/repo/other/x.py"),
            ("/repo/./src/main.rs", "/repo/src/main.rs"),
            ("/repo/a/b/../../c", "/repo/c"),
            ("/repo/..", "/"),
            ("/..", "/"),
            ("/../../etc", "/etc"),
            ("a/b/../c", "a/c"),
            ("./a", "a"),
        ];

        for (input, expected) in cases {
            assert_eq!(
                normalize_lexical(Path::new(input)),
                PathBuf::from(expected),
                "normalizing {input}"
            );
        }
    }

    #[test]
    fn normalize_keeps_leading_parent_on_relative_paths() {
        // Nothing to pop, so dropping these would change which directory the path names.
        assert_eq!(normalize_lexical(Path::new("../sibling")), PathBuf::from("../sibling"));
        assert_eq!(normalize_lexical(Path::new("../../x")), PathBuf::from("../../x"));
        assert_eq!(normalize_lexical(Path::new("a/../../x")), PathBuf::from("../x"));
    }

    #[test]
    fn containment_is_not_fooled_by_a_parent_component() {
        // The whole point: lexically `/repo/sub/../other` starts with `/repo/sub`, which would let
        // a sibling directory's config apply to files that are not under it.
        assert!(!is_ancestor_of(
            Path::new("/repo/sub"),
            Path::new("/repo/sub/../other/x.py")
        ));
        assert!(is_ancestor_of(Path::new("/repo"), Path::new("/repo/sub/../other/x.py")));
        assert!(is_ancestor_of(Path::new("/repo"), Path::new("/repo")));
        assert!(!is_ancestor_of(Path::new("/repo"), Path::new("/repository/x")));
    }

    #[test]
    fn absolute_normalized_joins_then_normalizes() {
        assert_eq!(
            absolute_normalized(Path::new("/repo/sub"), Path::new("../other/x.py")),
            PathBuf::from("/repo/other/x.py")
        );
        assert_eq!(
            absolute_normalized(Path::new("/repo"), Path::new("/abs/./y.py")),
            PathBuf::from("/abs/y.py")
        );
    }

    #[test]
    fn repo_relative_strips_the_root_and_rejects_outsiders() {
        assert_eq!(
            repo_relative(Path::new("/repo"), Path::new("/repo/./src/main.rs")),
            Some(PathBuf::from("src/main.rs"))
        );
        assert_eq!(repo_relative(Path::new("/repo"), Path::new("/elsewhere/x")), None);
    }

    #[test]
    fn find_repo_root_accepts_a_git_file_as_well_as_a_directory() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        let nested = root.join("a/b");
        std::fs::create_dir_all(&nested).unwrap();

        assert_eq!(find_repo_root(&nested), None);

        // A linked worktree has `.git` as a file, not a directory.
        std::fs::write(root.join(".git"), "gitdir: /elsewhere/.git/worktrees/wt\n").unwrap();
        assert_eq!(
            find_repo_root(&nested).as_deref(),
            Some(normalize_lexical(root).as_path())
        );
    }

    #[test]
    fn to_slash_is_identity_on_unix_separators() {
        assert_eq!(to_slash(Path::new("src/commands/scan.rs")), "src/commands/scan.rs");
    }
}
