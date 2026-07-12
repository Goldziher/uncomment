//! Terminal presentation: the single home for colors, symbols, and structured
//! output so styling is not scattered across the codebase.
//!
//! All output routes through [`anstream`], which strips ANSI escapes
//! automatically when the destination is not a terminal or when `NO_COLOR` is
//! set. Colors are therefore applied unconditionally with [`owo_colors`]; the
//! stream — not each call site — decides whether they survive. This means
//! `uncomment ... | cat` and `NO_COLOR=1 uncomment ...` both produce clean plain
//! text with no manual gating.

use std::fmt::Display;
use std::path::Path;

use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;

/// Success / completed marker.
pub const CHECK: &str = "✓";
/// Neutral informational marker.
pub const BULLET: &str = "•";
/// Show the progress bar only when a run touches at least this many files.
pub const PROGRESS_MIN_FILES: usize = 20;

/// Accent (brand teal → terminal cyan): paths, highlights, primary emphasis.
pub fn accent(value: impl Display) -> String {
    value.cyan().to_string()
}

/// Style a filesystem path in the accent color.
pub fn path(value: &Path) -> String {
    value.display().to_string().cyan().to_string()
}

/// Positive outcome (files written, config created).
pub fn success(value: impl Display) -> String {
    value.green().to_string()
}

/// Cautionary output that is not an error (preserved-comment warnings).
pub fn warn(value: impl Display) -> String {
    value.yellow().to_string()
}

/// Error output.
pub fn danger(value: impl Display) -> String {
    value.red().to_string()
}

/// Secondary / de-emphasized detail (counts, skipped-file breakdowns).
pub fn dim(value: impl Display) -> String {
    value.dimmed().to_string()
}

/// Emphasis without color (headings, totals).
pub fn bold(value: impl Display) -> String {
    value.bold().to_string()
}

/// Cap on the number of line spans shown per file before `--verbose`; keeps the
/// per-file line readable on large removals.
const LINE_RANGE_CAP: usize = 12;

/// Format a comment's line span as `L3` (single line) or `L7–9` (multi-line),
/// converting the 0-based rows to 1-based, inclusive line numbers.
pub fn line_span(start_row: usize, end_row: usize) -> String {
    if start_row == end_row {
        format!("L{}", start_row + 1)
    } else {
        format!("L{}–{}", start_row + 1, end_row + 1)
    }
}

/// Comma-joined line spans for the removed comments, e.g. `L3, L7–9, L15`.
///
/// When `verbose` is false the list is capped at [`LINE_RANGE_CAP`] entries and a
/// `+N more` suffix is appended so a file with hundreds of removals stays readable.
pub fn format_line_ranges(rows: impl Iterator<Item = (usize, usize)>, verbose: bool) -> String {
    let spans: Vec<String> = rows.map(|(start, end)| line_span(start, end)).collect();
    if verbose || spans.len() <= LINE_RANGE_CAP {
        return spans.join(", ");
    }
    let hidden = spans.len() - LINE_RANGE_CAP;
    format!("{}, +{hidden} more", spans[..LINE_RANGE_CAP].join(", "))
}

/// clap help styling that matches the brand: cyan headers/usage, green literals.
pub fn clap_styles() -> clap::builder::Styles {
    use clap::builder::styling::{AnsiColor, Style};

    clap::builder::Styles::styled()
        .header(Style::new().bold().fg_color(Some(AnsiColor::Cyan.into())))
        .usage(Style::new().bold().fg_color(Some(AnsiColor::Cyan.into())))
        .literal(Style::new().bold().fg_color(Some(AnsiColor::Green.into())))
        .placeholder(Style::new().fg_color(Some(AnsiColor::BrightBlack.into())))
        .valid(Style::new().fg_color(Some(AnsiColor::Green.into())))
        .invalid(Style::new().bold().fg_color(Some(AnsiColor::Red.into())))
        .error(Style::new().bold().fg_color(Some(AnsiColor::Red.into())))
}

/// A styled, terminal-aware progress bar for the file-processing phase.
///
/// Returns a hidden bar when stderr is not a terminal, so piped or redirected
/// runs never emit control characters.
pub fn progress_bar(len: u64) -> ProgressBar {
    use std::io::IsTerminal;

    if !std::io::stderr().is_terminal() {
        return ProgressBar::hidden();
    }

    let bar = ProgressBar::new(len);
    let style = ProgressStyle::with_template("  {spinner:.cyan} [{bar:28.cyan/dim}] {pos}/{len} files {msg}")
        .unwrap_or_else(|_| ProgressStyle::default_bar())
        .progress_chars("━━╌");
    bar.set_style(style);
    bar
}

/// Render the end-of-run summary line.
///
/// The wording (`Summary: N files processed, M modified/would be modified`) is a
/// stable, machine-parsed contract consumed by the benchmark tool and the
/// integration tests; only the coloring is cosmetic. Colors are applied around —
/// never inside — those tokens, and `anstream` strips them for non-terminal
/// consumers, so the plain-text tokens are always intact.
pub fn print_summary(total_files: usize, modified_files: usize, comments_removed: usize, dry_run: bool) {
    let prefix = if dry_run { "[DRY RUN] " } else { "" };
    let modified_verb = if dry_run { "would be modified" } else { "modified" };

    anstream::println!();
    anstream::println!(
        "{}{} {} files processed, {} {}, {} comments removed",
        dim(prefix),
        bold("Summary:"),
        accent(total_files),
        accent(modified_files),
        modified_verb,
        accent(comments_removed),
    );

    if total_files > 0 && modified_files == 0 {
        anstream::println!(
            "{}",
            dim("All files were already comment-free or only contained preserved comments.")
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_span_renders_single_and_multi_line() {
        assert_eq!(line_span(0, 0), "L1");
        assert_eq!(line_span(6, 8), "L7–9");
    }

    #[test]
    fn format_line_ranges_joins_spans() {
        let rows = [(0, 0), (2, 2), (4, 5)];
        assert_eq!(format_line_ranges(rows.into_iter(), false), "L1, L3, L5–6");
    }

    #[test]
    fn format_line_ranges_caps_when_not_verbose() {
        let rows: Vec<(usize, usize)> = (0..15).map(|row| (row, row)).collect();
        let capped = format_line_ranges(rows.iter().copied(), false);
        assert!(capped.ends_with("+3 more"), "expected cap suffix, got: {capped}");
        assert_eq!(capped.matches('L').count(), LINE_RANGE_CAP);
    }

    #[test]
    fn format_line_ranges_shows_all_when_verbose() {
        let rows: Vec<(usize, usize)> = (0..15).map(|row| (row, row)).collect();
        let full = format_line_ranges(rows.iter().copied(), true);
        assert!(!full.contains("more"));
        assert_eq!(full.matches('L').count(), 15);
    }
}
