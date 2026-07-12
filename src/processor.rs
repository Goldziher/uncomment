use crate::ast::visitor::{CommentInfo, CommentVisitor};
use crate::config::{ConfigManager, ResolvedConfig};
use crate::languages::registry::LanguageRegistry;
use crate::rules::preservation::PreservationRule;
use anyhow::{Context, Result};
use std::borrow::Cow;
use std::path::Path;
use tree_sitter::Parser;

#[derive(Debug, Clone)]
pub struct ProcessingOptions {
    pub remove_todo: bool,
    pub remove_fixme: bool,
    pub remove_doc: bool,
    pub custom_preserve_patterns: Vec<String>,
    pub use_default_ignores: bool,
    pub dry_run: bool,
    pub show_diff: bool,
    pub respect_gitignore: bool,
    pub traverse_git_repos: bool,
}

pub struct Processor {
    parser: Parser,
    registry: LanguageRegistry,
}

impl Default for Processor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor {
    pub fn new() -> Self {
        Self {
            parser: Parser::new(),
            registry: LanguageRegistry::new(),
        }
    }

    pub fn new_with_config(config_manager: &ConfigManager) -> Self {
        let mut registry = LanguageRegistry::new();

        let all_languages = config_manager.get_all_languages();
        registry.register_configured_languages(&all_languages);

        Self {
            parser: Parser::new(),
            registry,
        }
    }

    pub fn process_file_with_config(
        &mut self,
        path: &Path,
        config_manager: &ConfigManager,
        cli_overrides: Option<&ProcessingOptions>,
    ) -> Result<ProcessedFile> {
        let content =
            std::fs::read_to_string(path).with_context(|| format!("Failed to read file: {}", path.display()))?;

        let language_config = self
            .registry
            .detect_language_arc(path)
            .with_context(|| format!("Unsupported file type: {}", path.display()))?;

        let language_name = if language_config.name.bytes().all(|byte| !byte.is_ascii_uppercase()) {
            Cow::Borrowed(language_config.name.as_str())
        } else {
            Cow::Owned(language_config.name.to_lowercase())
        };

        let mut resolved_config = config_manager.get_config_for_file_with_language(path, &language_name);

        if let Some(overrides) = cli_overrides {
            if overrides.remove_doc {
                resolved_config.remove_docs = true;
            }
            resolved_config.use_default_ignores = overrides.use_default_ignores;
            if overrides.remove_todo {
                resolved_config.remove_todos = true;
            }
            if overrides.remove_fixme {
                resolved_config.remove_fixme = true;
            }
            if !overrides.custom_preserve_patterns.is_empty() {
                resolved_config
                    .preserve_patterns
                    .extend(overrides.custom_preserve_patterns.iter().cloned());
            }
            resolved_config.respect_gitignore = overrides.respect_gitignore;
            resolved_config.traverse_git_repos = overrides.traverse_git_repos;
        }

        let outcome = self.process_content_with_config(&content, language_config.as_ref(), &resolved_config)?;

        Ok(ProcessedFile {
            path: path.to_path_buf(),
            original_content: content,
            processed_content: outcome.content,
            modified: false,
            comments_removed: outcome.removed_comments.len(),
            removed_comments: outcome.removed_comments,
            removed_ranges: outcome.removed_ranges,
            important_removals: outcome.important_removals,
        })
    }

    fn process_content_with_config(
        &mut self,
        content: &str,
        language_config: &crate::languages::config::LanguageConfig,
        resolved_config: &ResolvedConfig,
    ) -> Result<ProcessOutcome> {
        let language = tree_sitter_language_pack::get_language(&language_config.tslp_name).with_context(|| {
            format!(
                "Failed to load grammar for '{}' (tslp name: '{}')",
                language_config.name, language_config.tslp_name
            )
        })?;

        self.parser
            .set_language(&language)
            .context("Failed to set parser language")?;

        let tree = self
            .parser
            .parse(content, None)
            .context("Failed to parse source code")?;

        let preservation_rules = self.create_preservation_rules_from_config(resolved_config);

        let mut visitor = CommentVisitor::new_with_language(
            content,
            &preservation_rules,
            &language_config.comment_types,
            &language_config.doc_comment_types,
            &language_config.name,
        );
        visitor.visit_node(tree.root_node());

        let comments_to_remove = visitor.get_comments_to_remove();

        let removed_comments = comments_to_remove
            .iter()
            .map(|comment| RemovedComment {
                start_row: comment.start_row,
                end_row: comment.end_row,
                is_documentation: comment.is_documentation,
                preview: first_line_preview(comment.content(content)),
            })
            .collect();

        let important_removals = detect_important_removals(&comments_to_remove, content);

        let (output, removed_ranges) = self.remove_comments_from_content(content, &comments_to_remove);

        Ok(ProcessOutcome {
            content: output,
            removed_comments,
            important_removals,
            removed_ranges,
        })
    }

    fn create_preservation_rules_from_config(&self, config: &ResolvedConfig) -> Vec<PreservationRule> {
        let mut rules = Vec::new();

        rules.push(PreservationRule::shebang());

        // Always preserve ~keep
        rules.push(PreservationRule::pattern("~keep"));

        // Preserve TODO/FIXME unless explicitly removed
        if !config.remove_todos {
            rules.push(PreservationRule::pattern("TODO"));
            rules.push(PreservationRule::pattern("todo"));
        }
        if !config.remove_fixme {
            rules.push(PreservationRule::pattern("FIXME"));
            rules.push(PreservationRule::pattern("fixme"));
        }

        if !config.remove_docs {
            rules.push(PreservationRule::documentation());
        }

        for pattern in &config.preserve_patterns {
            rules.push(PreservationRule::pattern_owned(pattern.clone()));
        }

        if config.use_default_ignores {
            let mut comprehensive_rules = PreservationRule::comprehensive_rules();

            // Remove TODO/FIXME rules if they should be removed according to config
            if config.remove_todos {
                comprehensive_rules.retain(|rule| !rule.pattern_matches("TODO") && !rule.pattern_matches("todo"));
            }
            if config.remove_fixme {
                comprehensive_rules.retain(|rule| !rule.pattern_matches("FIXME") && !rule.pattern_matches("fixme"));
            }

            if config.remove_docs {
                comprehensive_rules.retain(|rule| !matches!(rule, PreservationRule::Documentation));
            }

            rules.extend(comprehensive_rules);
        }

        rules
    }

    /// Rewrite `content` with the given comments removed, returning the new source
    /// and the byte ranges (in the *original* `content`) that were deleted.
    fn remove_comments_from_content(
        &self,
        content: &str,
        comments_to_remove: &[&CommentInfo],
    ) -> (String, Vec<(usize, usize)>) {
        if comments_to_remove.is_empty() {
            return (content.to_string(), Vec::new());
        }

        let bytes = content.as_bytes();

        let mut ranges: Vec<(usize, usize)> = Vec::with_capacity(comments_to_remove.len());
        for comment in comments_to_remove {
            ranges.push((comment.start_byte, comment.end_byte));
        }
        ranges.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));

        let mut filtered: Vec<(usize, usize)> = Vec::with_capacity(ranges.len());
        for (start, end) in ranges {
            if let Some(previous) = filtered.last()
                && start >= previous.0
                && end <= previous.1
            {
                continue;
            }
            filtered.push((start, end));
        }

        let mut removal_ranges: Vec<(usize, usize)> = Vec::with_capacity(filtered.len());
        for (start, end) in &filtered {
            if let Some(range) = Self::expand_range(bytes, *start, *end) {
                removal_ranges.push(range);
            }
        }

        let mut output = String::with_capacity(content.len());
        let mut cursor = 0;
        for (start, end) in &removal_ranges {
            let start = cursor.max(*start);
            if cursor < start {
                output.push_str(&content[cursor..start]);
            }
            cursor = *end;
        }
        if cursor < content.len() {
            output.push_str(&content[cursor..]);
        }
        (output, removal_ranges)
    }

    /// Expand a comment byte range `[start, end)` to cover its whole line(s) when
    /// only whitespace surrounds it, so removing a standalone comment also drops
    /// the now-blank line. Returns `None` for degenerate ranges (empty or past the
    /// end of `bytes`); otherwise the expanded range, or the original span when the
    /// comment shares its line with code.
    fn expand_range(bytes: &[u8], start: usize, end: usize) -> Option<(usize, usize)> {
        let end = end.min(bytes.len());
        if start >= end || start >= bytes.len() {
            return None;
        }

        let line_start = match memchr::memrchr(b'\n', &bytes[..start]) {
            Some(pos) => pos + 1,
            None => 0,
        };
        let line_end = match memchr::memchr(b'\n', &bytes[end..]) {
            Some(pos) => end + pos + 1,
            None => bytes.len(),
        };

        let before = &bytes[line_start..start];
        let after = &bytes[end..line_end];
        let before_ws = before.iter().all(|b| b.is_ascii_whitespace());
        let after_ws = after.iter().all(|b| b.is_ascii_whitespace());

        if before_ws && after_ws {
            Some((line_start, line_end))
        } else {
            Some((start, end))
        }
    }

    /// Detect the removable comments in `content` without touching the filesystem
    /// or rewriting the source, returning one [`Removal`] per comment that would be
    /// stripped (with both the comment span and the expanded delete range).
    ///
    /// The language is chosen from `path`'s extension via the built-in registry.
    /// This is a pure in-memory planning API intended for host tools (e.g. editors
    /// or linters) that build their own diagnostics/edits from the ranges rather
    /// than consuming the already-rewritten string. Config discovery is *not*
    /// performed — the caller supplies a fully [`ResolvedConfig`].
    ///
    /// # Errors
    ///
    /// Returns [`UncommentError::LanguageNotSupported`](crate::UncommentError) (via
    /// `anyhow`) when `path`'s extension maps to no known language, and propagates
    /// grammar-load / parse failures.
    pub fn plan_removals(&mut self, content: &str, path: &Path, config: &ResolvedConfig) -> Result<Vec<Removal>> {
        let language_config = self
            .registry
            .detect_language_arc(path)
            .with_context(|| format!("Unsupported file type: {}", path.display()))?;

        let language = tree_sitter_language_pack::get_language(&language_config.tslp_name).with_context(|| {
            format!(
                "Failed to load grammar for '{}' (tslp name: '{}')",
                language_config.name, language_config.tslp_name
            )
        })?;
        self.parser
            .set_language(&language)
            .context("Failed to set parser language")?;
        let tree = self
            .parser
            .parse(content, None)
            .context("Failed to parse source code")?;

        let preservation_rules = self.create_preservation_rules_from_config(config);
        let mut visitor = CommentVisitor::new_with_language(
            content,
            &preservation_rules,
            &language_config.comment_types,
            &language_config.doc_comment_types,
            &language_config.name,
        );
        visitor.visit_node(tree.root_node());

        let bytes = content.as_bytes();
        let removals = visitor
            .get_comments_to_remove()
            .into_iter()
            .filter_map(|comment| {
                let (remove_start, remove_end) = Self::expand_range(bytes, comment.start_byte, comment.end_byte)?;
                let preview = first_line_preview(comment.content(content));
                Some(Removal {
                    comment_start: comment.start_byte,
                    comment_end: comment.end_byte,
                    remove_start,
                    remove_end,
                    start_row: comment.start_row,
                    is_documentation: comment.is_documentation,
                    preview,
                })
            })
            .collect();
        Ok(removals)
    }
}

/// A single comment that [`Processor::plan_removals`] determined is removable,
/// expressed as byte offsets into the analysed source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removal {
    /// Start byte of the comment token itself (diagnostic location).
    pub comment_start: usize,
    /// End byte of the comment token itself.
    pub comment_end: usize,
    /// Start byte of the range a deleting edit should remove. Equals
    /// `comment_start` unless the comment stands alone on its line(s), in which
    /// case the range is expanded to swallow the surrounding whitespace/newline.
    pub remove_start: usize,
    /// End byte of the range a deleting edit should remove.
    pub remove_end: usize,
    /// 0-based line of the comment's first byte.
    pub start_row: usize,
    /// Whether the comment was classified as documentation.
    pub is_documentation: bool,
    /// Trimmed, length-capped first line of the comment, for a human message.
    pub preview: String,
}

/// Cap on [`Removal::preview`] length so a huge block comment can't bloat a
/// diagnostic message.
const PREVIEW_MAX_CHARS: usize = 80;

/// Internal result of rewriting one source string.
struct ProcessOutcome {
    content: String,
    removed_comments: Vec<RemovedComment>,
    important_removals: Vec<ImportantRemoval>,
    removed_ranges: Vec<(usize, usize)>,
}

#[derive(Debug)]
pub struct ProcessedFile {
    pub path: std::path::PathBuf,
    pub original_content: String,
    pub processed_content: String,
    pub modified: bool,
    pub comments_removed: usize,
    /// One entry per removed comment, in source order, for location reporting.
    pub removed_comments: Vec<RemovedComment>,
    /// Byte ranges deleted from `original_content`, used to render the diff.
    pub removed_ranges: Vec<(usize, usize)>,
    pub important_removals: Vec<ImportantRemoval>,
}

/// A single removed comment, expressed by line for human-facing location output.
#[derive(Debug, Clone)]
pub struct RemovedComment {
    /// 0-based first line of the comment.
    pub start_row: usize,
    /// 0-based last line of the comment.
    pub end_row: usize,
    /// Whether the comment was classified as documentation.
    pub is_documentation: bool,
    /// Trimmed, length-capped first line of the comment, for `--verbose` output.
    pub preview: String,
}

#[derive(Debug, Clone)]
pub struct ImportantRemoval {
    pub line: usize,
    pub reason: Cow<'static, str>,
    pub preview: String,
}

/// Trimmed, length-capped first line of a comment, for human-facing messages.
fn first_line_preview(content: &str) -> String {
    content
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .chars()
        .take(PREVIEW_MAX_CHARS)
        .collect()
}

pub struct OutputWriter {
    dry_run: bool,
    verbose: bool,
    show_diff: bool,
    quiet: bool,
}

impl OutputWriter {
    pub fn new(dry_run: bool, verbose: bool, show_diff: bool, quiet: bool) -> Self {
        Self {
            dry_run,
            verbose,
            show_diff,
            quiet,
        }
    }

    /// Persist the rewritten file (on a real run) and report what changed.
    ///
    /// The write happens before the `quiet` gate so `--quiet` silences reporting
    /// without ever suppressing the actual edit.
    pub fn write_file(&self, processed_file: &ProcessedFile) -> Result<()> {
        use crate::ui;

        let modified = processed_file.original_content != processed_file.processed_content;

        if modified && !self.dry_run {
            std::fs::write(&processed_file.path, &processed_file.processed_content)
                .with_context(|| format!("Failed to write file: {}", processed_file.path.display()))?;
        }

        if self.quiet {
            return Ok(());
        }

        if !modified {
            if self.verbose {
                anstream::println!(
                    "{} {} {}",
                    ui::success(ui::CHECK),
                    ui::dim("No changes needed:"),
                    ui::path(&processed_file.path)
                );
            }
            return Ok(());
        }

        let count = processed_file.comments_removed;
        let ranges = ui::format_line_ranges(
            processed_file
                .removed_comments
                .iter()
                .map(|comment| (comment.start_row, comment.end_row)),
            self.verbose,
        );

        if self.dry_run {
            anstream::println!(
                "{} {} {} {}",
                ui::accent("[DRY RUN]"),
                ui::dim("Would modify:"),
                ui::path(&processed_file.path),
                ui::dim(format!("— would remove {count} ({ranges})")),
            );
        } else {
            anstream::println!(
                "{} {} {}",
                ui::success("Modified:"),
                ui::path(&processed_file.path),
                ui::dim(format!("— removed {count} ({ranges})")),
            );
        }

        if self.verbose {
            for comment in &processed_file.removed_comments {
                anstream::println!(
                    "  {}  {}",
                    ui::accent(ui::line_span(comment.start_row, comment.end_row)),
                    ui::dim(&comment.preview),
                );
            }
        }

        if self.show_diff {
            self.show_diff(processed_file);
        }

        Ok(())
    }

    /// Render a unified-style diff of the removed comments.
    ///
    /// Because `uncomment` only ever deletes, each original line's post-state is
    /// that line with its overlapping deleted byte ranges cut out — so the diff is
    /// derived exactly from [`ProcessedFile::removed_ranges`] with no guessing about
    /// line alignment (the failure mode of a naive index-by-index compare).
    fn show_diff(&self, processed_file: &ProcessedFile) {
        use crate::ui;
        const CONTEXT: usize = 2;

        let content = &processed_file.original_content;
        let merged = merge_ranges(&processed_file.removed_ranges);

        let mut records: Vec<DiffLine> = Vec::new();
        let mut offset = 0usize;
        for raw in content.split_inclusive('\n') {
            let line_start = offset;
            let line_full_end = offset + raw.len();
            offset = line_full_end;
            let text_end = line_full_end - usize::from(raw.ends_with('\n'));
            let text = &content[line_start..text_end];
            let remaining = cut_ranges(content, line_start, text_end, &merged);

            let kind = if remaining == text {
                DiffKind::Context
            } else if remaining.trim().is_empty() {
                DiffKind::Removed
            } else {
                DiffKind::Changed { remaining }
            };
            records.push(DiffLine {
                text: text.to_string(),
                kind,
            });
        }

        let total = records.len();
        let mut show = vec![false; total];
        for (index, record) in records.iter().enumerate() {
            if !matches!(record.kind, DiffKind::Context) {
                let lo = index.saturating_sub(CONTEXT);
                let hi = (index + CONTEXT).min(total.saturating_sub(1));
                show.iter_mut().take(hi + 1).skip(lo).for_each(|flag| *flag = true);
            }
        }

        anstream::println!();
        anstream::println!("{}", ui::dim(format!("--- {}", processed_file.path.display())));
        anstream::println!(
            "{}",
            ui::dim(format!("+++ {} (processed)", processed_file.path.display()))
        );

        let width = format!("{total}").len();
        let mut printed_any = false;
        for (index, record) in records.iter().enumerate() {
            if !show[index] {
                continue;
            }
            if printed_any && index > 0 && !show[index - 1] {
                anstream::println!("{}", ui::dim("  ⋯"));
            }
            printed_any = true;

            let number = format!("{:>width$}", index + 1, width = width);
            match &record.kind {
                DiffKind::Context => {
                    anstream::println!("{} {}", ui::dim(&number), ui::dim(format!(" {}", record.text)));
                }
                DiffKind::Removed => {
                    anstream::println!("{} {}", ui::dim(&number), ui::danger(format!("-{}", record.text)));
                }
                DiffKind::Changed { remaining } => {
                    anstream::println!("{} {}", ui::dim(&number), ui::danger(format!("-{}", record.text)));
                    anstream::println!("{} {}", ui::dim(&number), ui::success(format!("+{remaining}")));
                }
            }
        }
    }

    pub fn print_summary(&self, total_files: usize, modified_files: usize, comments_removed: usize) {
        crate::ui::print_summary(total_files, modified_files, comments_removed, self.dry_run);
    }
}

/// A classified original line, used only by [`OutputWriter::show_diff`].
struct DiffLine {
    text: String,
    kind: DiffKind,
}

enum DiffKind {
    /// Untouched by any removal.
    Context,
    /// The whole line vanished (standalone comment).
    Removed,
    /// Part of the line was cut (e.g. a trailing comment); `remaining` is the result.
    Changed { remaining: String },
}

/// Merge sorted/unsorted, possibly overlapping byte ranges into disjoint ranges.
fn merge_ranges(ranges: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut sorted = ranges.to_vec();
    sorted.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::with_capacity(sorted.len());
    for (start, end) in sorted {
        match merged.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    merged
}

/// Return `content[from..to]` with any bytes covered by `merged` ranges removed.
fn cut_ranges(content: &str, from: usize, to: usize, merged: &[(usize, usize)]) -> String {
    let mut out = String::new();
    let mut cursor = from;
    for &(start, end) in merged {
        if end <= from || start >= to {
            continue;
        }
        let start = start.max(from);
        let end = end.min(to);
        if cursor < start {
            out.push_str(&content[cursor..start]);
        }
        cursor = cursor.max(end);
    }
    if cursor < to {
        out.push_str(&content[cursor..to]);
    }
    out
}

fn detect_important_removals(comments_to_remove: &[&CommentInfo], source: &str) -> Vec<ImportantRemoval> {
    comments_to_remove
        .iter()
        .copied()
        .filter_map(|comment| {
            let trimmed = comment.content(source).trim_start();
            let reason = if trimmed.starts_with("#!") {
                Some(Cow::Borrowed("shebang"))
            } else if trimmed.starts_with("//go:")
                || trimmed.starts_with("/*go:")
                || trimmed.starts_with("//+build")
                || trimmed.starts_with("// +build")
                || trimmed.starts_with("//line ")
                || trimmed.starts_with("/*line ")
            {
                Some(Cow::Borrowed("go directive"))
            } else if trimmed.contains("shellcheck") {
                Some(Cow::Borrowed("shellcheck directive"))
            } else if trimmed.contains("eslint-")
                || trimmed.contains("prettier-")
                || trimmed.contains("@ts-")
                || trimmed.contains("biome-")
                || trimmed.contains("deno-")
                || trimmed.contains("nolint")
            {
                Some(Cow::Borrowed("linter/formatter directive"))
            } else if trimmed.starts_with("#pragma") || trimmed.contains("NOLINT") || trimmed.contains("clang-format") {
                Some(Cow::Borrowed("compiler/formatter directive"))
            } else if trimmed.starts_with("# frozen_string_literal:")
                || trimmed.starts_with("# encoding:")
                || trimmed.starts_with("# coding:")
                || trimmed.starts_with("# typed:")
            {
                Some(Cow::Borrowed("language magic comment"))
            } else {
                None
            }?;

            let normalized_preview = if trimmed.contains('\n') {
                Cow::Owned(trimmed.replace('\n', " "))
            } else {
                Cow::Borrowed(trimmed)
            };

            let mut preview = normalized_preview.into_owned();
            const MAX: usize = 120;
            if preview.len() > MAX {
                let mut cut = MAX;
                while cut > 0 && !preview.is_char_boundary(cut) {
                    cut -= 1;
                }
                preview.truncate(cut);
                preview.push('…');
            }

            Some(ImportantRemoval {
                line: comment.start_row + 1,
                reason,
                preview,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, ConfigManager, ResolvedConfig};
    use crate::languages::config::LanguageConfig;
    use tempfile::tempdir;

    fn default_resolved_config() -> ResolvedConfig {
        ResolvedConfig {
            remove_todos: false,
            remove_fixme: false,
            remove_docs: false,
            preserve_patterns: Vec::new(),
            use_default_ignores: true,
            respect_gitignore: true,
            traverse_git_repos: false,
            language_config: None,
        }
    }

    fn process_rust(source: &str) -> String {
        let mut processor = Processor::new();
        let language_config = LanguageConfig::rust();
        let resolved_config = default_resolved_config();
        let ProcessOutcome { content: output, .. } = processor
            .process_content_with_config(source, &language_config, &resolved_config)
            .expect("processing rust source");
        output
    }

    #[test]
    fn plan_removals_reports_removable_comments_with_ranges() {
        let source = "// remove me\nfn main() {\n    let x = 1; // trailing\n    // TODO: keep\n    // ~keep\n}\n";
        let mut processor = Processor::new();
        let removals = processor
            .plan_removals(source, std::path::Path::new("sample.rs"), &default_resolved_config())
            .expect("plan removals");
        // TODO (remove_todos=false) and ~keep are preserved; two comments remain.
        let previews: Vec<&str> = removals.iter().map(|removal| removal.preview.as_str()).collect();
        assert_eq!(previews, vec!["// remove me", "// trailing"]);
        assert_eq!(removals[0].remove_start, 0);
        assert_eq!(
            &source[removals[0].remove_start..removals[0].remove_end],
            "// remove me\n"
        );
        assert_eq!(&source[removals[1].remove_start..removals[1].remove_end], "// trailing");
    }

    #[test]
    fn merge_ranges_combines_touching_and_overlapping() {
        assert_eq!(merge_ranges(&[(0, 5), (5, 10)]), vec![(0, 10)], "touching ranges merge");
        assert_eq!(
            merge_ranges(&[(0, 5), (6, 10)]),
            vec![(0, 5), (6, 10)],
            "disjoint stay split"
        );
        assert_eq!(merge_ranges(&[(0, 7), (3, 10)]), vec![(0, 10)], "overlapping merge");
        assert_eq!(
            merge_ranges(&[(6, 10), (0, 5)]),
            vec![(0, 5), (6, 10)],
            "unsorted input is sorted"
        );
        assert_eq!(merge_ranges(&[]), Vec::<(usize, usize)>::new(), "empty input");
    }

    #[test]
    fn cut_ranges_removes_only_covered_bytes() {
        let content = "abcdefghij";
        assert_eq!(cut_ranges(content, 0, 10, &[(3, 6)]), "abcghij", "range mid-window");
        assert_eq!(
            cut_ranges(content, 2, 8, &[(2, 4)]),
            "efgh",
            "range flush to window start"
        );
        assert_eq!(
            cut_ranges(content, 2, 8, &[(6, 8)]),
            "cdef",
            "range flush to window end"
        );
        assert_eq!(cut_ranges(content, 2, 8, &[(0, 10)]), "", "range covers whole window");
        assert_eq!(
            cut_ranges(content, 2, 5, &[(6, 9)]),
            "cde",
            "range outside window is ignored"
        );
        assert_eq!(
            cut_ranges(content, 0, 5, &[(3, 3)]),
            "abcde",
            "zero-length range is a no-op"
        );
    }

    #[test]
    fn records_removed_comment_locations_and_previews() {
        let source = "// standalone\nfn main() {\n    let x = 1; // trailing\n    /* block\n       two */\n}\n";
        let mut processor = Processor::new();
        let language_config = LanguageConfig::rust();
        let outcome = processor
            .process_content_with_config(source, &language_config, &default_resolved_config())
            .expect("processing rust source");

        let spans: Vec<(usize, usize)> = outcome
            .removed_comments
            .iter()
            .map(|comment| (comment.start_row, comment.end_row))
            .collect();
        assert_eq!(spans, vec![(0, 0), (2, 2), (3, 4)]);

        let previews: Vec<&str> = outcome
            .removed_comments
            .iter()
            .map(|comment| comment.preview.as_str())
            .collect();
        assert_eq!(previews, vec!["// standalone", "// trailing", "/* block"]);

        assert!(outcome.removed_comments.iter().all(|comment| !comment.is_documentation));
        assert_eq!(outcome.removed_comments.len(), 3);
    }

    #[test]
    fn plan_removals_preserves_python_docstrings_by_default() {
        let source = "def f():\n    \"\"\"docstring\"\"\"\n    # remove me\n    return 1\n";
        let mut processor = Processor::new();
        let removals = processor
            .plan_removals(source, std::path::Path::new("module.py"), &default_resolved_config())
            .expect("plan removals");
        let previews: Vec<&str> = removals.iter().map(|removal| removal.preview.as_str()).collect();
        assert_eq!(previews, vec!["# remove me"]);
    }

    #[test]
    fn plan_removals_unsupported_extension_errors() {
        let mut processor = Processor::new();
        let result = processor.plan_removals(
            "noop",
            std::path::Path::new("file.unknownext"),
            &default_resolved_config(),
        );
        assert!(result.is_err());
    }

    fn process_go(source: &str, use_default_ignores: bool, remove_docs: bool) -> String {
        let mut processor = Processor::new();
        let language_config = LanguageConfig::go();
        let mut resolved_config = default_resolved_config();
        resolved_config.use_default_ignores = use_default_ignores;
        resolved_config.remove_docs = remove_docs;
        let ProcessOutcome { content: output, .. } = processor
            .process_content_with_config(source, &language_config, &resolved_config)
            .expect("processing go source");
        output
    }

    fn process_language(source: &str, language_config: LanguageConfig) -> String {
        let mut processor = Processor::new();
        let resolved_config = default_resolved_config();
        let ProcessOutcome { content: output, .. } = processor
            .process_content_with_config(source, &language_config, &resolved_config)
            .expect("processing source");
        output
    }

    fn process_language_with_default_ignores(
        source: &str,
        language_config: LanguageConfig,
        use_default_ignores: bool,
    ) -> String {
        let mut processor = Processor::new();
        let mut resolved_config = default_resolved_config();
        resolved_config.use_default_ignores = use_default_ignores;
        let ProcessOutcome { content: output, .. } = processor
            .process_content_with_config(source, &language_config, &resolved_config)
            .expect("processing source");
        output
    }

    #[test]
    fn preserves_strings_matching_comment_text() {
        let source = r#"fn main() {
    let pattern = "// comment";
    println!("{}", pattern); // comment
}
"#;

        let processed = process_rust(source);

        assert!(processed.contains("\"// comment\""));
        assert!(!processed.contains("; // comment"));
    }

    #[test]
    fn preserves_macro_invocations_with_comment_like_strings() {
        let source = r#"macro_rules! announce {
    ($msg:expr) => {{
        println!("{}", $msg); // keep
    }};
}

fn main() {
    announce!("// keep");
}
"#;

        let processed = process_rust(source);

        assert!(processed.contains("announce!(\"// keep\");"));
        assert!(!processed.contains("// keep\n"));
    }

    #[test]
    fn preserves_attributes_when_removing_doc_comments() {
        let source = r#"#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Create smart config
    #[command(about = "Create a template configuration file")]
    Init,
}
"#;

        let mut processor = Processor::new();
        let language_config = LanguageConfig::rust();
        let mut config = default_resolved_config();
        config.remove_docs = true;

        let ProcessOutcome { content: processed, .. } = processor
            .process_content_with_config(source, &language_config, &config)
            .expect("process doc comments");

        assert!(processed.contains("#[command(about = \"Create a template configuration file\")]"));
        assert!(!processed.contains("Create smart config"));
    }

    #[test]
    fn respects_no_default_ignores_override() {
        let dir = tempdir().expect("create temp dir");
        let file_path = dir.path().join("sample.rs");
        let source = r#"/// #![feature(never_type)]
// NOTE: this would normally be preserved
fn main() {}
"#;

        std::fs::write(&file_path, source).expect("write test file");

        let config_manager = ConfigManager::from_single_config(dir.path(), Config::default()).expect("config manager");

        let mut processor = Processor::new();

        let overrides_with_defaults = ProcessingOptions {
            remove_todo: true,
            remove_fixme: true,
            remove_doc: true,
            custom_preserve_patterns: Vec::new(),
            use_default_ignores: true,
            dry_run: true,
            show_diff: false,
            respect_gitignore: true,
            traverse_git_repos: false,
        };

        let with_defaults = processor
            .process_file_with_config(&file_path, &config_manager, Some(&overrides_with_defaults))
            .expect("process with defaults");
        assert!(with_defaults.processed_content.contains("NOTE"));
        assert!(with_defaults.processed_content.contains("#![feature"));

        let overrides_without_defaults = ProcessingOptions {
            use_default_ignores: false,
            ..overrides_with_defaults
        };

        let without_defaults = processor
            .process_file_with_config(&file_path, &config_manager, Some(&overrides_without_defaults))
            .expect("process without defaults");
        assert!(!without_defaults.processed_content.contains("NOTE"));
        assert!(!without_defaults.processed_content.contains("#![feature"));
        assert!(without_defaults.processed_content.contains("fn main()"));
    }

    #[test]
    fn preserves_go_embed_directives_even_without_default_ignores() {
        let source = r#"package main

//go:embed hello.txt
var embedded string

func main() { /* regular comment should be removed */ }
"#;

        let processed = process_go(source, false, true);
        assert!(processed.contains("//go:embed hello.txt"));
        assert!(!processed.contains("regular comment should be removed"));
    }

    #[test]
    fn preserves_go_cgo_preamble_comments() {
        let source = r#"package htmltomarkdown

// #cgo LDFLAGS: -lhtml_to_markdown_ffi
// #include <stdlib.h>
// extern const char* html_to_markdown_version();
import "C"

func Version() string { return C.GoString(C.html_to_markdown_version()) /* regular comment should be removed */ }
"#;

        for use_default_ignores in [true, false] {
            let processed = process_go(source, use_default_ignores, true);
            assert!(
                processed.contains("// #cgo LDFLAGS: -lhtml_to_markdown_ffi"),
                "expected to preserve cgo preamble with use_default_ignores={use_default_ignores}"
            );
            assert!(
                processed.contains("// #include <stdlib.h>"),
                "expected to preserve cgo preamble with use_default_ignores={use_default_ignores}"
            );
            assert!(
                processed.contains("// extern const char* html_to_markdown_version();"),
                "expected to preserve cgo preamble with use_default_ignores={use_default_ignores}"
            );
            assert!(processed.contains("import \"C\""));
            assert!(!processed.contains("regular comment should be removed"));
        }
    }

    #[test]
    fn removes_ruby_comments_without_touching_strings() {
        let source = r#"# remove me
puts "Hello # not a comment"
"#;

        let processed = process_language(source, LanguageConfig::ruby());
        assert!(!processed.contains("# remove me"));
        assert!(processed.contains("Hello # not a comment"));
    }

    #[test]
    fn preserves_ruby_frozen_string_literal_magic_comment() {
        let source = r#"# frozen_string_literal: true
# remove me
puts "ok"
"#;

        let processed = process_language(source, LanguageConfig::ruby());
        assert!(processed.contains("# frozen_string_literal: true"));
        assert!(!processed.contains("# remove me"));
    }

    #[test]
    fn preserves_shebangs_even_without_default_ignores() {
        let source = r#"#!/usr/bin/env bash
# remove me
echo "ok"
"#;

        let processed = process_language_with_default_ignores(source, LanguageConfig::shell(), false);

        assert!(processed.starts_with("#!/usr/bin/env bash\n"));
        assert!(!processed.contains("# remove me"));
        assert!(processed.contains("echo \"ok\""));
    }

    #[test]
    fn preserves_ruby_yard_doc_comments_by_default() {
        let source = r#"# @param x [Integer]
def foo(x)
  x + 1
end
"#;

        let processed = process_language(source, LanguageConfig::ruby());
        assert!(processed.contains("# @param x [Integer]"));
    }

    #[test]
    fn removes_php_comments_without_touching_strings() {
        let source = r#"<?php
// remove me
$s = "// not a comment";
echo $s;
"#;

        let processed = process_language(source, LanguageConfig::php());
        assert!(!processed.contains("// remove me"));
        assert!(processed.contains("\"// not a comment\""));
    }

    #[test]
    fn preserves_c_header_guard_trailing_comments() {
        let source = r#"#ifndef HTML_TO_MARKDOWN_H
#define HTML_TO_MARKDOWN_H

// remove me
int x;

#endif  /* HTML_TO_MARKDOWN_H */
"#;

        let processed = process_language(source, LanguageConfig::c());
        assert!(processed.contains("#endif  /* HTML_TO_MARKDOWN_H */"));
        assert!(!processed.contains("remove me"));
        assert!(processed.contains("int x;"));
    }

    #[test]
    fn removes_elixir_comments_without_touching_strings() {
        let source = r##"# remove me
IO.puts("# not a comment")
"##;

        let processed = process_language(source, LanguageConfig::elixir());
        assert!(!processed.contains("# remove me"));
        assert!(processed.contains("\"# not a comment\""));
    }

    #[test]
    fn removes_toml_comments_without_touching_strings() {
        let source = r##"# remove me
key = "# not a comment"
"##;

        let processed = process_language(source, LanguageConfig::toml());
        assert!(!processed.contains("# remove me"));
        assert!(processed.contains("\"# not a comment\""));
    }

    #[test]
    fn removes_csharp_comments_without_touching_strings() {
        let source = r#"// remove me
class C { void M() { var s = "// not a comment"; } }
"#;

        let processed = process_language(source, LanguageConfig::csharp());
        assert!(!processed.contains("// remove me"));
        assert!(processed.contains("\"// not a comment\""));
    }

    #[test]
    fn removes_haskell_comments_without_touching_strings() {
        let source = r#"-- remove me
main = putStrLn "-- not a comment"
"#;

        let processed = process_language(source, LanguageConfig::haskell());
        assert!(!processed.contains("-- remove me"));
        assert!(processed.contains("\"-- not a comment\""));
    }

    #[test]
    fn removes_html_comments_without_touching_content() {
        let source = r#"<!-- remove me -->
<div>Hello</div>
"#;

        let processed = process_language(source, LanguageConfig::html());
        assert!(!processed.contains("remove me"));
        assert!(processed.contains("<div>Hello</div>"));
    }

    #[test]
    fn removes_css_comments_without_touching_strings() {
        let source = r#"/* remove me */
.a::before { content: "/* not a comment */"; }
"#;

        let processed = process_language(source, LanguageConfig::css());
        assert!(!processed.contains("remove me"));
        assert!(processed.contains("\"/* not a comment */\""));
    }

    #[test]
    fn removes_xml_comments_without_touching_text() {
        let source = r#"<!-- remove me -->
<root>hello</root>
"#;

        let processed = process_language(source, LanguageConfig::xml());
        assert!(!processed.contains("remove me"));
        assert!(processed.contains("<root>hello</root>"));
    }

    #[test]
    fn removes_sql_comments_without_touching_strings() {
        let source = r#"-- remove me
SELECT '-- not a comment' as val;
"#;

        let processed = process_language(source, LanguageConfig::sql());
        assert!(!processed.contains("-- remove me"));
        assert!(processed.contains("'-- not a comment'"));
    }

    #[test]
    fn removes_kotlin_comments_without_touching_strings() {
        let source = r#"// remove me
fun main() { val s = "// not a comment" }
"#;

        let processed = process_language(source, LanguageConfig::kotlin());
        assert!(!processed.contains("// remove me"));
        assert!(processed.contains("\"// not a comment\""));
    }

    #[test]
    fn removes_swift_comments_without_touching_strings() {
        let source = r#"// remove me
let s = "// not a comment"
"#;

        let processed = process_language(source, LanguageConfig::swift());
        assert!(!processed.contains("// remove me"));
        assert!(processed.contains("\"// not a comment\""));
    }

    #[test]
    fn removes_lua_comments_without_touching_strings() {
        let source = r#"-- remove me
local s = "-- not a comment"
"#;

        let processed = process_language(source, LanguageConfig::lua());
        assert!(!processed.contains("-- remove me"));
        assert!(processed.contains("\"-- not a comment\""));
    }

    #[test]
    fn removes_nix_comments_without_touching_strings() {
        let source = r##"# remove me
let s = "# not a comment"; in s
"##;

        let processed = process_language(source, LanguageConfig::nix());
        assert!(!processed.contains("# remove me"));
        assert!(processed.contains("\"# not a comment\""));
    }

    #[test]
    fn removes_powershell_comments_without_touching_strings() {
        let source = r##"# remove me
$s = "# not a comment"
Write-Output $s
"##;

        let processed = process_language(source, LanguageConfig::powershell());
        assert!(!processed.contains("# remove me"));
        assert!(processed.contains("\"# not a comment\""));
    }

    #[test]
    fn removes_proto_comments_without_touching_strings() {
        let source = r#"// remove me
syntax = "proto3";
message A { string s = 1 [default = "// not a comment"]; }
"#;

        let processed = process_language(source, LanguageConfig::proto());
        assert!(!processed.contains("// remove me"));
        assert!(processed.contains("\"// not a comment\""));
    }

    #[test]
    fn removes_ini_comments_without_touching_values() {
        let source = r#"; remove me
[section]
key = # not a comment
"#;

        let processed = process_language(source, LanguageConfig::ini());
        assert!(!processed.contains("; remove me"));
        assert!(processed.contains("key = # not a comment"));
    }

    #[test]
    fn removes_python_docstrings_when_remove_docs_enabled() {
        let source = r#""""This is a docstring"""
# TODO: regular todo
# mypy: ignore
def hello(): pass"#;

        let mut processor = Processor::new();
        let language_config = LanguageConfig::python();
        let mut resolved_config = default_resolved_config();
        resolved_config.remove_docs = true;

        let ProcessOutcome { content: output, .. } = processor
            .process_content_with_config(source, &language_config, &resolved_config)
            .expect("processing python source");

        assert!(
            !output.contains("This is a docstring"),
            "Python docstring should be removed when remove_docs=true"
        );
        assert!(output.contains("TODO: regular todo"), "TODO should be preserved");
        assert!(output.contains("mypy: ignore"), "mypy should be preserved");
    }

    #[test]
    fn handles_utf8_multibyte_in_comments() {
        let source = "// Comment with emoji 🎉\nfn main() {}\n";

        let processed = process_rust(source);
        assert!(!processed.contains("🎉"));
        assert!(processed.contains("fn main()"));
    }

    #[test]
    fn handles_file_with_only_comments() {
        let source = "// Only comments\n// Nothing else\n";

        let processed = process_rust(source);
        assert!(processed.trim().is_empty());
    }

    #[test]
    fn handles_empty_file() {
        let source = "";

        let mut processor = Processor::new();
        let language_config = LanguageConfig::rust();
        let resolved_config = default_resolved_config();
        let outcome = processor
            .process_content_with_config(source, &language_config, &resolved_config)
            .expect("processing empty source");
        assert_eq!(outcome.content, "");
        assert_eq!(outcome.removed_comments.len(), 0);
    }

    #[test]
    fn handles_comment_at_end_of_file_no_trailing_newline() {
        let source = "fn main() {} // trailing";

        let processed = process_rust(source);
        assert!(!processed.contains("// trailing"));
        assert!(processed.contains("fn main()"));
    }
}
