//! `uncomment --check` — the removal run as a gate.
//!
//! The run is the same one `--dry-run` performs: same file collection, same configuration, same
//! verdict for every comment, and nothing written. What differs is the result: each comment the run
//! would remove is a violation, reported as a grep-friendly `path:line:col: text` line, and the exit
//! code says whether any exist — which is what a pre-commit hook or a CI job needs to fail on.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::changes::{ChangeScope, ChangeScopeArgs};
use crate::languages::registry::LanguageRegistry;
use crate::lint::OutputFormat;
use crate::paths::{absolute_normalized, repo_relative, to_slash};
use crate::processor::ProcessedFile;
use crate::ui;

/// Nothing would be removed.
pub const EXIT_CLEAN: i32 = 0;
/// At least one comment would be removed.
pub const EXIT_REMOVABLE: i32 = 1;
/// The check could not be completed — a usage error, a rejected config, a file that could not be
/// inspected — so its verdict cannot be trusted either way. Matches clap's own usage-error code.
pub const EXIT_ERROR: i32 = 2;

/// Marker for a failed check, the counterpart of [`ui::CHECK`].
const CROSS: &str = "✗";

/// The flags that turn the removal run into a gate. They exist only on the top-level command.
#[derive(clap::Args, Debug, Clone, Default)]
pub struct CheckArgs {
    /// Report what would be removed and exit 1 if anything would; write nothing
    #[arg(
        long,
        conflicts_with = "diff",
        help = "Fail (exit 1) if any comment would be removed; write nothing",
        long_help = "Run exactly as --dry-run would, write nothing, and print each comment that would be \
                     removed as `path:line:col: text`. Exits 0 when nothing would be removed, 1 when \
                     something would, and 2 when the check could not be completed (bad arguments, a \
                     rejected config, a file that could not be processed, a failed `git diff`).",
        help_heading = "Check"
    )]
    pub check: bool,

    #[arg(
        long,
        value_enum,
        default_value = "text",
        value_name = "FORMAT",
        requires = "check",
        help = "Output format for --check",
        help_heading = "Check"
    )]
    pub format: OutputFormat,

    #[command(flatten)]
    pub scope: ChangeScopeArgs,
}

/// One comment the run would remove, resolved to a 1-based line and column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// As printed: relative to the invocation directory where possible.
    pub path: PathBuf,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    /// The comment's first line, trimmed and length-capped.
    pub excerpt: String,
}

/// Everything a check concluded, before any of it is printed.
#[derive(Debug, Default)]
pub struct Outcome {
    pub violations: Vec<Violation>,
    /// Anything worth saying that is not a violation: the change scope, a skipped file.
    pub notes: Vec<String>,
    pub files_checked: usize,
    pub files_with_violations: usize,
    /// Files that matched but could not be processed, so nothing about them is known.
    pub uninspectable: usize,
    /// What to put inside the backticks of the "keep a comment" tip, in the first violating file's
    /// own comment syntax. `None` when nothing violated, so no tip is due.
    pub keep_marker_line_hint: Option<String>,
}

impl Outcome {
    /// Collect the violations out of the processed files, sorted by path, line and column.
    ///
    /// `base` is the directory printed paths are made relative to. A per-line `scope` drops every
    /// comment on a line the diff did not touch, and says how many it dropped. `registry` names the
    /// first violating file's own comment syntax for the "keep a comment" tip, the same as the
    /// removal command's.
    pub fn from_results(
        base: &Path,
        results: &[ProcessedFile],
        scope: Option<&ChangeScope>,
        registry: &LanguageRegistry,
    ) -> Self {
        let mut outcome = Self {
            files_checked: results.len(),
            ..Self::default()
        };
        let mut out_of_scope = 0usize;
        for processed in results {
            let absolute = absolute_normalized(base, &processed.path);
            let path = display_path(base, &processed.path);
            let before = outcome.violations.len();
            for comment in &processed.removed_comments {
                let (line, end_line) = (comment.start_row + 1, comment.end_row + 1);
                if scope.is_some_and(|scope| !scope.touches(&absolute, line, end_line)) {
                    out_of_scope += 1;
                    continue;
                }
                outcome.violations.push(Violation {
                    path: path.clone(),
                    line,
                    column: comment.start_column + 1,
                    end_line,
                    excerpt: comment.preview.clone(),
                });
            }
            if outcome.violations.len() > before {
                outcome.files_with_violations += 1;
            }
        }
        if out_of_scope > 0 {
            outcome.notes.push(format!(
                "{out_of_scope} removable comment(s) outside the changed lines not reported"
            ));
        }
        outcome.violations.sort_by(|a, b| {
            a.path
                .cmp(&b.path)
                .then(a.line.cmp(&b.line))
                .then(a.column.cmp(&b.column))
        });
        outcome.keep_marker_line_hint = (!outcome.violations.is_empty()).then(|| {
            results
                .iter()
                .find(|processed| !processed.removed_comments.is_empty())
                .and_then(|processed| registry.detect_language(&processed.path))
                .map_or_else(|| "comment".to_string(), |language| language.keep_marker_line_hint())
        });
        outcome
    }

    /// [`EXIT_ERROR`] when a file could not be inspected, else [`EXIT_REMOVABLE`] when anything
    /// would be removed, else [`EXIT_CLEAN`].
    pub fn exit_code(&self) -> i32 {
        if self.uninspectable > 0 {
            EXIT_ERROR
        } else if self.violations.is_empty() {
            EXIT_CLEAN
        } else {
            EXIT_REMOVABLE
        }
    }
}

/// `file` relative to `base` when it lies underneath, else absolute.
pub fn display_path(base: &Path, file: &Path) -> PathBuf {
    let absolute = absolute_normalized(base, file);
    repo_relative(base, &absolute).unwrap_or(absolute)
}

/// Print `outcome`: notes to stderr, violations and the summary to stdout.
///
/// `quiet` drops the per-comment lines and keeps the summary, in the text format only — a JSON
/// consumer asked for the data.
///
/// # Errors
///
/// Fails only if the JSON document cannot be rendered.
pub fn report(outcome: &Outcome, format: OutputFormat, quiet: bool) -> Result<()> {
    match format {
        OutputFormat::Text => {
            report_text(outcome, quiet);
            Ok(())
        }
        OutputFormat::Json => report_json(outcome),
    }
}

fn report_text(outcome: &Outcome, quiet: bool) {
    for note in &outcome.notes {
        anstream::eprintln!("{} {}", ui::dim(ui::BULLET), ui::dim(note));
    }

    if !quiet {
        for violation in &outcome.violations {
            anstream::println!(
                "{}:{}:{}: {}",
                ui::accent(to_slash(&violation.path)),
                violation.line,
                violation.column,
                violation.excerpt
            );
        }
    }

    let mut detail = vec![format!("{} file(s) checked", outcome.files_checked)];
    if outcome.uninspectable > 0 {
        detail.push(format!("{} file(s) could not be inspected", outcome.uninspectable));
    }
    let detail = detail.join(", ");

    if outcome.violations.is_empty() && outcome.uninspectable == 0 {
        anstream::println!(
            "{} {} ({})",
            ui::success(ui::CHECK),
            ui::success("no removable comments"),
            ui::dim(detail)
        );
        return;
    }

    anstream::println!(
        "{} {} ({})",
        ui::danger(CROSS),
        ui::danger(format!(
            "{} removable comment(s) in {} file(s)",
            outcome.violations.len(),
            outcome.files_with_violations
        )),
        ui::dim(detail)
    );
    if !outcome.violations.is_empty() && !quiet {
        let marker_line = outcome.keep_marker_line_hint.as_deref().unwrap_or("//");
        anstream::eprintln!(
            "{}",
            ui::dim(format!(
                "Tip: remove these comments, or keep one by adding `~keep` to it or to a `{marker_line}` line just \
                 above it."
            ))
        );
    }
}

fn report_json(outcome: &Outcome) -> Result<()> {
    let document = serde_json::json!({
        "violations": outcome
            .violations
            .iter()
            .map(|violation| serde_json::json!({
                "path": to_slash(&violation.path),
                "line": violation.line,
                "column": violation.column,
                "end_line": violation.end_line,
                "excerpt": violation.excerpt,
            }))
            .collect::<Vec<_>>(),
        "notes": outcome.notes,
        "summary": {
            "files_checked": outcome.files_checked,
            "files_with_violations": outcome.files_with_violations,
            "violations": outcome.violations.len(),
            "uninspectable_files": outcome.uninspectable,
        },
    });

    anstream::println!(
        "{}",
        serde_json::to_string_pretty(&document).context("failed to render the check report")?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::processor::RemovedComment;

    fn processed(path: &str, comments: &[(usize, usize)]) -> ProcessedFile {
        ProcessedFile {
            path: PathBuf::from(path),
            original_content: String::new(),
            processed_content: String::new(),
            modified: !comments.is_empty(),
            comments_removed: comments.len(),
            removed_comments: comments
                .iter()
                .map(|&(row, column)| RemovedComment {
                    start_row: row,
                    start_column: column,
                    end_row: row,
                    is_documentation: false,
                    preview: format!("// at {row}"),
                })
                .collect(),
            removed_ranges: Vec::new(),
            important_removals: Vec::new(),
            redundant_markers: Vec::new(),
        }
    }

    #[test]
    fn violations_are_one_based_and_sorted_by_path_line_and_column() {
        let base = Path::new("/repo");
        let results = [
            processed("/repo/b.rs", &[(4, 0), (0, 8), (0, 2)]),
            processed("/repo/a.rs", &[(9, 0)]),
            processed("/repo/clean.rs", &[]),
        ];

        let outcome = Outcome::from_results(base, &results, None, &LanguageRegistry::new());

        let positions: Vec<(String, usize, usize)> = outcome
            .violations
            .iter()
            .map(|v| (to_slash(&v.path), v.line, v.column))
            .collect();
        assert_eq!(
            positions,
            vec![
                ("a.rs".to_string(), 10, 1),
                ("b.rs".to_string(), 1, 3),
                ("b.rs".to_string(), 1, 9),
                ("b.rs".to_string(), 5, 1),
            ]
        );
        assert_eq!(outcome.files_checked, 3);
        assert_eq!(outcome.files_with_violations, 2);
    }

    #[test]
    fn an_uninspectable_file_outranks_violations_in_the_exit_code() {
        let mut outcome = Outcome::from_results(
            Path::new("/repo"),
            &[processed("/repo/a.rs", &[(0, 0)])],
            None,
            &LanguageRegistry::new(),
        );
        assert_eq!(outcome.exit_code(), EXIT_REMOVABLE);

        outcome.uninspectable = 1;
        assert_eq!(
            outcome.exit_code(),
            EXIT_ERROR,
            "an incomplete check must not read as a verdict"
        );

        let clean = Outcome::from_results(
            Path::new("/repo"),
            &[processed("/repo/a.rs", &[])],
            None,
            &LanguageRegistry::new(),
        );
        assert_eq!(clean.exit_code(), EXIT_CLEAN);
    }

    #[test]
    fn a_path_outside_the_base_stays_absolute() {
        assert_eq!(
            display_path(Path::new("/repo"), Path::new("/elsewhere/a.rs")),
            PathBuf::from("/elsewhere/a.rs")
        );
        assert_eq!(
            display_path(Path::new("/repo"), Path::new("src/a.rs")),
            PathBuf::from("src/a.rs")
        );
    }
}
