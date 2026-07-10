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
