use crate::ast::visitor::{CommentInfo, CommentVisitor, VisitDecision};
use crate::config::{ConfigManager, ResolvedConfig};
use crate::languages::config::CommentSyntaxResolution;
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
            // Apply flags one-directionally so an unset flag never clobbers config-file
            // values: only --no-default-ignores forces this off (see issue #106).
            if !overrides.use_default_ignores {
                resolved_config.use_default_ignores = false;
            }
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
            if !overrides.respect_gitignore {
                resolved_config.respect_gitignore = false;
            }
            if overrides.traverse_git_repos {
                resolved_config.traverse_git_repos = true;
            }
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
            redundant_markers: outcome.redundant_markers,
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
        visitor.extend_keep_blocks();
        visitor.extend_keep_above();

        let marker_ranges = visitor.redundant_keep_markers(resolved_config.remove_docs);
        let redundant_markers = marker_ranges
            .iter()
            .map(|&(start, _)| RedundantMarker {
                line: content[..start].bytes().filter(|&byte| byte == b'\n').count(),
                preview: first_line_preview(line_containing(content, start)),
            })
            .collect();

        let comments_to_remove = dedupe_nested(visitor.get_comments_to_remove());

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

        let (output, removed_ranges) = self.remove_comments_from_content(content, &comments_to_remove, &marker_ranges);

        Ok(ProcessOutcome {
            content: output,
            removed_comments,
            important_removals,
            removed_ranges,
            redundant_markers,
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
    /// Cut `comments_to_remove` from `content`, and additionally excise
    /// `marker_ranges` — spans of redundant `~keep` text inside comments that are
    /// otherwise preserved. Returns the new content and the byte ranges dropped
    /// from the original, which the diff renderer replays.
    fn remove_comments_from_content(
        &self,
        content: &str,
        comments_to_remove: &[&CommentInfo],
        marker_ranges: &[(usize, usize)],
    ) -> (String, Vec<(usize, usize)>) {
        if comments_to_remove.is_empty() && marker_ranges.is_empty() {
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

        let mut removal_ranges: Vec<(usize, usize)> = Vec::with_capacity(filtered.len() + marker_ranges.len());
        for (start, end) in &filtered {
            if let Some(range) = Self::expand_range(bytes, *start, *end) {
                removal_ranges.push(range);
            }
        }

        // Marker spans are excised verbatim — never widened to the whole line,
        // because the comment around them is being kept.
        removal_ranges.extend_from_slice(marker_ranges);
        removal_ranges.sort_unstable();
        removal_ranges.dedup();

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
    /// A filter over [`Self::inspect`], which reports preserved comments too.
    ///
    /// # Errors
    ///
    /// Returns [`UncommentError::LanguageNotSupported`](crate::UncommentError) (via
    /// `anyhow`) when `path`'s extension maps to no known language, and propagates
    /// grammar-load / parse failures.
    pub fn plan_removals(&mut self, content: &str, path: &Path, config: &ResolvedConfig) -> Result<Vec<Removal>> {
        let removals = self
            .inspect(content, path, config)?
            .into_iter()
            .filter_map(|comment| match comment.verdict {
                Verdict::Remove {
                    expanded_start,
                    expanded_end,
                } => Some(Removal {
                    comment_start: comment.start_byte,
                    comment_end: comment.end_byte,
                    remove_start: expanded_start,
                    remove_end: expanded_end,
                    start_row: comment.start_row,
                    is_documentation: comment.is_documentation,
                    preview: first_line_preview(&comment.text),
                }),
                Verdict::Preserve => None,
            })
            .collect();
        Ok(removals)
    }

    /// Every comment in `content`, kept and removed alike, with why — the read-only
    /// inventory [`Self::plan_removals`] filters down to its removals.
    ///
    /// Like `plan_removals` this touches no filesystem, rewrites nothing, picks the
    /// language from `path`'s extension via the built-in registry, and performs no
    /// config discovery. Entries are ordered by `start_byte` ascending.
    ///
    /// Comments the grammar records as several nested nodes (a Rust `///` line arrives
    /// as both an outer `line_comment` and an inner doc node) are collapsed to the
    /// outermost, so one comment is reported once.
    ///
    /// # Errors
    ///
    /// Returns an error when `path`'s extension maps to no known language, and
    /// propagates grammar-load / parse failures.
    pub fn inspect(&mut self, content: &str, path: &Path, config: &ResolvedConfig) -> Result<Vec<InspectedComment>> {
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
        visitor.extend_keep_blocks();
        visitor.extend_keep_above();

        let comments = visitor.comments();
        let mut selected = dedupe_nested_indices(comments, |comment| !comment.should_preserve);
        selected.extend(dedupe_nested_indices(comments, |comment| comment.should_preserve));
        // Each side is already ascending, so a stable sort merges them without
        // disturbing the removable side's order — that order is `plan_removals`' output.
        selected.sort_by(|&a, &b| comments[a].start_byte.cmp(&comments[b].start_byte));

        let bytes = content.as_bytes();
        let syntax = language_config.resolve_comment_syntax();
        let mut inspected = Vec::with_capacity(selected.len());
        for index in selected {
            let comment = &comments[index];
            let text = comment.content(content);
            let verdict = if comment.should_preserve {
                Verdict::Preserve
            } else {
                match Self::expand_range(bytes, comment.start_byte, comment.end_byte) {
                    Some((expanded_start, expanded_end)) => Verdict::Remove {
                        expanded_start,
                        expanded_end,
                    },
                    // A degenerate node (empty, or past the end of the source) describes
                    // no comment and no edit.
                    None => continue,
                }
            };
            let reason = if comment.should_preserve {
                preserve_reason(visitor.decision(index), visitor.is_extended(index))
            } else {
                None
            };
            inspected.push(InspectedComment {
                start_byte: comment.start_byte,
                end_byte: comment.end_byte,
                start_row: comment.start_row,
                end_row: comment.end_row,
                node_type: comment.node_type.clone(),
                kind: classify_kind(comment, text, syntax),
                verdict,
                reason,
                text: text.to_string(),
                is_documentation: comment.is_documentation,
            });
        }
        Ok(inspected)
    }
}

/// The shape a comment is written in, independent of whether it survives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentKind {
    Line,
    Block,
    /// Documentation in comment syntax: `///`, `//!`, `/** */`, `##`, or a comment the
    /// grammar handler recognised as documentation from its position.
    Doc,
    /// Documentation the grammar records as a string literal rather than a comment —
    /// a Python docstring and its equivalents.
    Docstring,
}

/// Why a comment survived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreserveReason {
    /// A preservation pattern matched the comment's text; carries that pattern.
    Pattern(String),
    Documentation,
    FileHeader,
    Shebang,
    /// The comment carries its own `~keep`.
    KeepMarker,
    /// The comment carries no marker and survives only because an adjacent comment's
    /// `~keep` extends over it.
    ExtendedByNeighbourMarker,
    /// The language handler forced preservation from surrounding syntax rather than
    /// from the comment's own text — a Go build/embed directive, a cgo preamble, a
    /// trailing preprocessor comment, a Ruby magic comment.
    LanguageDirective,
}

/// What [`Processor::inspect`] decided to do with a comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The comment will be stripped. The expanded range is what a deleting edit should
    /// remove: the comment span, widened to the whole line(s) when nothing but
    /// whitespace surrounds it.
    Remove {
        expanded_start: usize,
        expanded_end: usize,
    },
    Preserve,
}

/// One comment found in a source file, with the verdict and the reason behind it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectedComment {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_row: usize,
    pub end_row: usize,
    pub node_type: String,
    pub kind: CommentKind,
    pub verdict: Verdict,
    /// `Some` for every preserved comment, `None` for every removed one.
    pub reason: Option<PreserveReason>,
    pub text: String,
    /// The grammar handler's own documentation classification, reported verbatim by
    /// [`Removal::is_documentation`]. Deliberately distinct from `kind`: Rust records
    /// `///` as a plain `line_comment` the handler never flags, while a Go `// Foo does
    /// …` above a declaration is documentation written in line-comment syntax.
    pub is_documentation: bool,
}

/// Name why a preserved comment survived, or `None` for one that is being removed or
/// whose survival has no recorded cause.
///
/// A rule matching the comment's own text wins over the neighbouring-marker fallback,
/// so a `// TODO` swept into a `~keep` block still reports as the pattern match it is,
/// and only a comment with no reason of its own is attributed to its neighbour.
fn preserve_reason(decision: VisitDecision<'_>, extended: bool) -> Option<PreserveReason> {
    if let Some(rule) = decision.matched_rule {
        return Some(match rule {
            PreservationRule::Pattern(pattern) if pattern.as_ref() == KEEP_MARKER => PreserveReason::KeepMarker,
            PreservationRule::Pattern(pattern) => PreserveReason::Pattern(pattern.as_ref().to_string()),
            PreservationRule::Documentation => PreserveReason::Documentation,
            PreservationRule::FileHeader => PreserveReason::FileHeader,
            PreservationRule::Shebang => PreserveReason::Shebang,
        });
    }
    if decision.forced_by_grammar {
        return Some(PreserveReason::LanguageDirective);
    }
    extended.then_some(PreserveReason::ExtendedByNeighbourMarker)
}

/// Classify a comment by shape, preferring the language's own delimiters over row
/// arithmetic: a Rust `///` node spans two rows because it swallows its newline, and a
/// `/* one liner */` spans one, so row count alone gets both backwards.
fn classify_kind(comment: &CommentInfo, text: &str, syntax: CommentSyntaxResolution) -> CommentKind {
    if comment.node_type.contains("string") {
        return CommentKind::Docstring;
    }
    if CommentVisitor::is_doc_comment(comment, text) {
        return CommentKind::Doc;
    }
    if let CommentSyntaxResolution::Resolved(syntax) = syntax {
        let trimmed = text.trim_start();
        if let Some((open, _)) = syntax.block
            && trimmed.starts_with(open)
        {
            return CommentKind::Block;
        }
        if let Some(line) = syntax.line
            && trimmed.starts_with(line)
        {
            return CommentKind::Line;
        }
    }
    if comment.start_row == comment.end_row {
        CommentKind::Line
    } else {
        CommentKind::Block
    }
}

/// Indices of the comments [`dedupe_nested`] would keep, considering only those
/// `include` accepts, ordered by `start_byte` ascending.
///
/// Nesting is resolved within one side of the verdict rather than across the whole
/// list: a removable comment nested inside a preserved one is still its own removal,
/// and collapsing the two would change what gets stripped.
fn dedupe_nested_indices(comments: &[CommentInfo], include: impl Fn(&CommentInfo) -> bool) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..comments.len()).filter(|&index| include(&comments[index])).collect();
    indices.sort_by(|&a, &b| {
        comments[a]
            .start_byte
            .cmp(&comments[b].start_byte)
            .then(comments[b].end_byte.cmp(&comments[a].end_byte))
    });

    let mut kept: Vec<usize> = Vec::with_capacity(indices.len());
    for index in indices {
        let nested = kept.last().is_some_and(|&outer| {
            comments[index].start_byte >= comments[outer].start_byte
                && comments[index].end_byte <= comments[outer].end_byte
        });
        if !nested {
            kept.push(index);
        }
    }
    kept
}

/// The marker token that protects a comment from removal.
const KEEP_MARKER: &str = "~keep";

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
    redundant_markers: Vec<RedundantMarker>,
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
    /// Redundant `~keep` markers stripped from preserved doc comments.
    pub redundant_markers: Vec<RedundantMarker>,
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

/// A `~keep` marker found inside a doc comment that keeps the comment alive on
/// its own, where the token does nothing but travel into rendered documentation.
#[derive(Debug, Clone)]
pub struct RedundantMarker {
    /// 0-based line of the doc comment carrying the marker.
    pub line: usize,
    /// Trimmed, length-capped text of that line, for human-facing messages.
    pub preview: String,
}

#[derive(Debug, Clone)]
pub struct ImportantRemoval {
    pub line: usize,
    pub reason: Cow<'static, str>,
    pub preview: String,
}

/// Drop comments wholly contained in another, keeping the outermost.
///
/// Grammars routinely record one comment as several nested nodes — a Rust `///`
/// line arrives twice, once as the outer `line_comment` and once as the inner
/// doc node. Removal already collapses the overlap, but the reported count and
/// line list are built from this list, so without deduplication a file with two
/// doc comments reports four removals across two duplicated ranges.
fn dedupe_nested(comments: Vec<&CommentInfo>) -> Vec<&CommentInfo> {
    let mut comments = comments;
    comments.sort_by(|a, b| a.start_byte.cmp(&b.start_byte).then(b.end_byte.cmp(&a.end_byte)));

    let mut kept: Vec<&CommentInfo> = Vec::with_capacity(comments.len());
    for comment in comments {
        let nested = kept
            .last()
            .is_some_and(|outer| comment.start_byte >= outer.start_byte && comment.end_byte <= outer.end_byte);
        if !nested {
            kept.push(comment);
        }
    }
    kept
}

/// The whole source line that `offset` falls on.
fn line_containing(content: &str, offset: usize) -> &str {
    let start = content[..offset].rfind('\n').map_or(0, |pos| pos + 1);
    let end = content[offset..].find('\n').map_or(content.len(), |pos| offset + pos);
    &content[start..end]
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

        // A file can change without a single comment being removed, when the only
        // edit was stripping redundant `~keep` markers — so the summary is built
        // from whichever parts actually apply rather than always leading with a
        // removal count.
        let mut parts: Vec<String> = Vec::new();
        if count > 0 {
            let verb = if self.dry_run { "would remove" } else { "removed" };
            parts.push(format!("{verb} {count} ({ranges})"));
        }
        let markers = processed_file.redundant_markers.len();
        if markers > 0 {
            let verb = if self.dry_run { "would strip" } else { "stripped" };
            let marker_lines = ui::format_line_ranges(
                processed_file
                    .redundant_markers
                    .iter()
                    .map(|marker| (marker.line, marker.line)),
                self.verbose,
            );
            let plural = if markers == 1 { "marker" } else { "markers" };
            parts.push(format!("{verb} {markers} redundant ~keep {plural} ({marker_lines})"));
        }
        let detail = parts.join(", ");

        if self.dry_run {
            anstream::println!(
                "{} {} {} {}",
                ui::accent("[DRY RUN]"),
                ui::dim("Would modify:"),
                ui::path(&processed_file.path),
                ui::dim(format!("— {detail}")),
            );
        } else {
            anstream::println!(
                "{} {} {}",
                ui::success("Modified:"),
                ui::path(&processed_file.path),
                ui::dim(format!("— {detail}")),
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
            for marker in &processed_file.redundant_markers {
                anstream::println!(
                    "  {}  {} {}",
                    ui::accent(ui::line_span(marker.line, marker.line)),
                    ui::dim("~keep stripped:"),
                    ui::dim(&marker.preview),
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

    fn process_rust_with(source: &str, remove_docs: bool) -> ProcessOutcome {
        let mut processor = Processor::new();
        let language_config = LanguageConfig::rust();
        let resolved_config = ResolvedConfig {
            remove_docs,
            ..default_resolved_config()
        };
        processor
            .process_content_with_config(source, &language_config, &resolved_config)
            .expect("processing rust source")
    }

    #[test]
    fn above_line_marker_preserves_doc_comment_under_remove_doc() {
        let source = "// ~keep\n/// Parent element ID.\npub fn a() {}\n";
        let outcome = process_rust_with(source, true);
        assert!(
            outcome.content.contains("/// Parent element ID."),
            "above-line marker protects the doc comment: {}",
            outcome.content
        );
        assert!(outcome.content.contains("// ~keep"), "the marker itself survives");
    }

    #[test]
    fn above_line_marker_does_not_reach_past_a_blank_line() {
        let source = "// ~keep\n\n/// Unrelated doc.\npub fn a() {}\n";
        let outcome = process_rust_with(source, true);
        assert!(
            !outcome.content.contains("/// Unrelated doc."),
            "a blank line ends the run: {}",
            outcome.content
        );
    }

    #[test]
    fn above_line_marker_does_not_reach_past_code() {
        // The `///` node swallows its trailing newline, so row adjacency alone
        // would make the comment after `pub fn a()` look like a neighbour.
        let source = "// ~keep\n/// Protected doc.\npub fn a() {}\n// unrelated removable\npub fn b() {}\n";
        let outcome = process_rust_with(source, true);
        assert!(outcome.content.contains("/// Protected doc."), "target kept");
        assert!(
            !outcome.content.contains("// unrelated removable"),
            "code between comments ends the run: {}",
            outcome.content
        );
    }

    #[test]
    fn redundant_keep_marker_is_stripped_from_doc_comment() {
        let source = "/// Parent element ID. ~keep Resolves elsewhere.\npub fn a() {}\n";
        let outcome = process_rust_with(source, false);
        assert!(
            outcome.content.contains("/// Parent element ID. Resolves elsewhere."),
            "marker stripped and spacing collapsed: {}",
            outcome.content
        );
        assert_eq!(outcome.redundant_markers.len(), 1, "one marker reported");
        assert_eq!(outcome.redundant_markers[0].line, 0);
    }

    #[test]
    fn trailing_redundant_marker_takes_the_space_before_it() {
        let source = "/// Parent element ID. ~keep\npub fn a() {}\n";
        let outcome = process_rust_with(source, false);
        assert!(
            outcome.content.contains("/// Parent element ID.\n"),
            "no trailing space left behind: {}",
            outcome.content
        );
    }

    #[test]
    fn load_bearing_keep_marker_survives_under_remove_doc() {
        let source = "/// Parent element ID. ~keep\npub fn a() {}\n";
        let outcome = process_rust_with(source, true);
        assert!(
            outcome.content.contains("~keep"),
            "the marker is the only thing preserving this doc comment: {}",
            outcome.content
        );
        assert!(outcome.redundant_markers.is_empty(), "nothing reported as redundant");
    }

    #[test]
    fn keep_marker_on_a_line_comment_is_never_stripped() {
        let source = "// rationale ~keep\npub fn a() {}\n";
        let outcome = process_rust_with(source, false);
        assert!(
            outcome.content.contains("// rationale ~keep"),
            "line comments do not render into docs, so the marker stays: {}",
            outcome.content
        );
        assert!(outcome.redundant_markers.is_empty());
    }

    #[test]
    fn nested_comment_nodes_are_counted_once() {
        // Rust records each `///` line twice: the outer `line_comment` and the
        // inner doc node. Both describe one comment.
        let source = "/// Doc one.\npub fn a() {}\n\n/// Doc two.\npub fn b() {}\n";
        let outcome = process_rust_with(source, true);
        assert_eq!(outcome.removed_comments.len(), 2, "two doc comments, not four nodes");
        let spans: Vec<(usize, usize)> = outcome
            .removed_comments
            .iter()
            .map(|comment| (comment.start_row, comment.end_row))
            .collect();
        assert_eq!(spans, vec![(0, 1), (3, 4)], "no duplicated ranges");
    }

    #[test]
    fn prose_about_the_marker_is_left_alone() {
        let cases = [
            "/// Comments containing `~keep` are preserved.\npub fn a() {}\n",
            "/// Write `/// ~keep Parent element ID.` to protect it.\npub fn a() {}\n",
            "/// ```text\n/// // ~keep\n/// ```\npub fn a() {}\n",
            "/// A ~keepsake is not a marker.\npub fn a() {}\n",
        ];
        for source in cases {
            let outcome = process_rust_with(source, false);
            assert_eq!(
                outcome.content, source,
                "documentation discussing the marker must survive intact: {source}"
            );
            assert!(outcome.redundant_markers.is_empty(), "nothing reported for: {source}");
        }
    }

    #[test]
    fn keep_marker_preserves_whole_contiguous_line_comment_block() {
        let source = "fn f() {\n    // line one\n    // line two\n    // line three ~keep\n    let x = 1;\n}\n";
        let output = process_rust(source);
        assert!(output.contains("// line one"), "first block line kept: {output}");
        assert!(output.contains("// line two"), "middle block line kept: {output}");
        assert!(output.contains("// line three ~keep"), "marked line kept: {output}");
    }

    #[test]
    fn keep_marker_on_first_line_preserves_block() {
        let source = "fn f() {\n    // one ~keep\n    // two\n    // three\n    let x = 1;\n}\n";
        let output = process_rust(source);
        assert!(output.contains("// one ~keep"), "marked line kept: {output}");
        assert!(output.contains("// two"), "following line kept: {output}");
        assert!(output.contains("// three"), "following line kept: {output}");
    }

    #[test]
    fn blank_line_breaks_keep_block() {
        let source = "fn f() {\n    // kept ~keep\n    // kept two\n\n    // dropped one\n    // dropped two\n    let x = 1;\n}\n";
        let output = process_rust(source);
        assert!(output.contains("// kept ~keep"), "marked line kept: {output}");
        assert!(output.contains("// kept two"), "same-block line kept: {output}");
        assert!(
            !output.contains("// dropped one"),
            "separate paragraph stripped: {output}"
        );
        assert!(
            !output.contains("// dropped two"),
            "separate paragraph stripped: {output}"
        );
    }

    #[test]
    fn trailing_keep_does_not_extend_to_standalone_neighbor() {
        let source = "fn f() {\n    let x = 1; // trailing ~keep\n    // standalone removable\n    let y = 2;\n}\n";
        let output = process_rust(source);
        assert!(
            output.contains("// trailing ~keep"),
            "trailing keep preserved per-comment: {output}"
        );
        assert!(
            !output.contains("// standalone removable"),
            "a trailing keep must not anchor a block: {output}"
        );
    }

    #[test]
    fn code_between_comments_breaks_keep_block() {
        let source = "fn f() {\n    // block a ~keep\n    let x = 1;\n    // block b removable\n    let y = 2;\n}\n";
        let output = process_rust(source);
        assert!(output.contains("// block a ~keep"), "marked line kept: {output}");
        assert!(
            !output.contains("// block b removable"),
            "code between comments ends the block: {output}"
        );
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
    fn honors_config_file_disabling_default_ignores() {
        // Regression for #106: a config with use_default_ignores = false must be honored
        // when --no-default-ignores was NOT passed. Previously the CLI default clobbered it,
        // leaving hardcoded NOTE/HACK patterns unremovable.
        let dir = tempdir().expect("create temp dir");
        let file_path = dir.path().join("sample.lua");
        std::fs::write(&file_path, "-- NOTE: remove me\n-- HACK: me too\nlocal x = 1\n").expect("write test file");

        let mut config = Config::default();
        config.global.use_default_ignores = false;
        let config_manager = ConfigManager::from_single_config(dir.path(), config).expect("config manager");

        // CLI defaults: use_default_ignores = true because --no-default-ignores absent.
        let overrides = ProcessingOptions {
            remove_todo: false,
            remove_fixme: false,
            remove_doc: false,
            custom_preserve_patterns: Vec::new(),
            use_default_ignores: true,
            dry_run: true,
            show_diff: false,
            respect_gitignore: true,
            traverse_git_repos: false,
        };

        let mut processor = Processor::new();
        let result = processor
            .process_file_with_config(&file_path, &config_manager, Some(&overrides))
            .expect("process lua file");

        assert!(!result.processed_content.contains("NOTE"));
        assert!(!result.processed_content.contains("HACK"));
        assert!(result.processed_content.contains("local x = 1"));
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
    fn removes_objc_comments_without_touching_strings() {
        let source = r#"// remove me
NSString *s = @"// not a comment";
"#;

        let processed = process_language(source, LanguageConfig::objc());
        assert!(!processed.contains("// remove me"));
        assert!(processed.contains("@\"// not a comment\""));
    }

    #[test]
    fn preserves_objc_preprocessor_trailing_comments() {
        let source = r#"#import "Local.h" // fallback
#define kTimeout 30 // seconds
// remove me
NSString *s = @"// not a comment";
"#;

        let processed = process_language(source, LanguageConfig::objc());
        assert!(processed.contains("#import \"Local.h\" // fallback"));
        assert!(processed.contains("#define kTimeout 30 // seconds"));
        assert!(!processed.contains("remove me"));
        assert!(processed.contains("@\"// not a comment\""));
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

    /// Sources covering the comment shapes that behave differently from each other:
    /// Rust `///` and `//!` (recorded as nested node pairs) plus a `/** */` block,
    /// Python docstrings (`string` nodes, not comments), JSDoc, and a `#`-comment
    /// language with a shebang.
    fn inventory_cases() -> Vec<(&'static str, LanguageConfig, &'static str)> {
        vec![
            (
                "sample.rs",
                LanguageConfig::rust(),
                "//! Crate entry point.\n\n/// Documented.\npub fn a() {}\n\n/** Block doc. */\npub fn b() {}\n\n// plain removable\npub fn c() {\n    let x = 1; // trailing removable\n    /* block\n       removable */\n}\n",
            ),
            (
                "module.py",
                LanguageConfig::python(),
                "\"\"\"Module docstring.\"\"\"\n\n\ndef f():\n    \"\"\"Function docstring.\"\"\"\n    # remove me\n    return 1  # trailing removable\n",
            ),
            (
                "app.js",
                LanguageConfig::javascript(),
                "/** JSDoc summary. */\nfunction f() {\n  // remove me\n  return 1; /* block removable */\n}\n",
            ),
            (
                "script.sh",
                LanguageConfig::shell(),
                "#!/usr/bin/env bash\n# remove me\necho \"ok\"  # trailing removable\n",
            ),
        ]
    }

    /// Apply a removal plan to `source` the way a host tool would, so the plan can be
    /// compared against what the rewriting path actually produces.
    fn apply_removals(source: &str, removals: &[Removal]) -> String {
        let merged = merge_ranges(
            &removals
                .iter()
                .map(|removal| (removal.remove_start, removal.remove_end))
                .collect::<Vec<_>>(),
        );
        let mut output = String::with_capacity(source.len());
        let mut cursor = 0;
        for (start, end) in merged {
            if cursor < start {
                output.push_str(&source[cursor..start]);
            }
            cursor = cursor.max(end);
        }
        output.push_str(&source[cursor..]);
        output
    }

    #[test]
    fn plan_removals_is_inspect_filtered_to_remove_verdicts() {
        for (path, _, source) in inventory_cases() {
            let mut processor = Processor::new();
            let config = default_resolved_config();

            let expected: Vec<Removal> = processor
                .inspect(source, Path::new(path), &config)
                .expect("inspect")
                .into_iter()
                .filter_map(|comment| match comment.verdict {
                    Verdict::Remove {
                        expanded_start,
                        expanded_end,
                    } => Some(Removal {
                        comment_start: comment.start_byte,
                        comment_end: comment.end_byte,
                        remove_start: expanded_start,
                        remove_end: expanded_end,
                        start_row: comment.start_row,
                        is_documentation: comment.is_documentation,
                        preview: first_line_preview(&comment.text),
                    }),
                    Verdict::Preserve => None,
                })
                .collect();

            let removals = processor
                .plan_removals(source, Path::new(path), &config)
                .expect("plan removals");
            assert_eq!(removals, expected, "plan_removals diverged from inspect for {path}");
            assert!(
                removals.len() >= 2,
                "{path} must exercise several removals, got {removals:?}"
            );
        }
    }

    #[test]
    fn inspect_removals_reproduce_the_rewritten_source() {
        for (path, language_config, source) in inventory_cases() {
            let mut processor = Processor::new();
            let removals = processor
                .plan_removals(source, Path::new(path), &default_resolved_config())
                .expect("plan removals");
            assert_eq!(
                apply_removals(source, &removals),
                process_language(source, language_config),
                "the inventory's removals disagree with the rewriting path for {path}"
            );
        }
    }

    #[test]
    fn inspect_is_ordered_and_every_preserved_comment_names_a_reason() {
        for (path, _, source) in inventory_cases() {
            let mut processor = Processor::new();
            let inspected = processor
                .inspect(source, Path::new(path), &default_resolved_config())
                .expect("inspect");

            assert!(
                inspected
                    .windows(2)
                    .all(|pair| pair[0].start_byte <= pair[1].start_byte),
                "{path} is not ordered by start_byte: {inspected:?}"
            );
            assert!(
                inspected.iter().any(|comment| comment.verdict == Verdict::Preserve),
                "{path} must exercise at least one preserved comment"
            );
            for comment in &inspected {
                match comment.verdict {
                    Verdict::Preserve => assert!(
                        comment.reason.is_some(),
                        "preserved comment has no reason in {path}: {comment:?}"
                    ),
                    Verdict::Remove { .. } => assert!(
                        comment.reason.is_none(),
                        "removed comment carries a reason in {path}: {comment:?}"
                    ),
                }
                assert_eq!(
                    comment.text,
                    &source[comment.start_byte..comment.end_byte],
                    "text does not match its byte range in {path}"
                );
            }
        }
    }

    #[test]
    fn inspect_reports_one_entry_per_comment_for_nested_node_pairs() {
        // Rust records each `///` line twice, as the outer `line_comment` and as the
        // inner doc node; the inventory is per comment, not per node.
        let source = "/// Doc one.\npub fn a() {}\n\n/// Doc two.\npub fn b() {}\n";
        let mut processor = Processor::new();
        let inspected = processor
            .inspect(source, Path::new("sample.rs"), &default_resolved_config())
            .expect("inspect");
        assert_eq!(inspected.len(), 2, "two doc comments, not four nodes: {inspected:?}");
        assert!(
            inspected.iter().all(
                |comment| comment.kind == CommentKind::Doc && comment.reason == Some(PreserveReason::Documentation)
            )
        );
    }

    #[test]
    fn inspect_classifies_comment_shapes() {
        let source = "//! Crate docs.\n\n/// Documented.\npub fn a() {}\n\n/** Block doc. */\npub fn b() {}\n\n// plain\npub fn c() {}\n\n/* block\n   comment */\npub fn d() {}\n";
        let mut processor = Processor::new();
        let inspected = processor
            .inspect(source, Path::new("sample.rs"), &default_resolved_config())
            .expect("inspect");
        let shapes: Vec<(CommentKind, &str)> = inspected
            .iter()
            .map(|comment| (comment.kind, comment.text.lines().next().unwrap_or_default()))
            .collect();
        assert_eq!(
            shapes,
            vec![
                (CommentKind::Doc, "//! Crate docs."),
                (CommentKind::Doc, "/// Documented."),
                (CommentKind::Doc, "/** Block doc. */"),
                (CommentKind::Line, "// plain"),
                (CommentKind::Block, "/* block"),
            ]
        );

        let python = "\"\"\"Module docstring.\"\"\"\n# plain\n";
        let inspected = processor
            .inspect(python, Path::new("module.py"), &default_resolved_config())
            .expect("inspect python");
        assert_eq!(inspected[0].kind, CommentKind::Docstring);
        assert_eq!(inspected[1].kind, CommentKind::Line);
    }

    #[test]
    fn above_line_marker_separates_the_marker_from_what_it_extends_over() {
        let source = "// ~keep\n/// Parent element ID.\npub fn a() {}\n";
        let config = ResolvedConfig {
            remove_docs: true,
            ..default_resolved_config()
        };
        let mut processor = Processor::new();
        let inspected = processor
            .inspect(source, Path::new("sample.rs"), &config)
            .expect("inspect");

        let reasons: Vec<(&str, Option<&PreserveReason>)> = inspected
            .iter()
            .map(|comment| (comment.text.lines().next().unwrap_or_default(), comment.reason.as_ref()))
            .collect();
        assert_eq!(
            reasons,
            vec![
                ("// ~keep", Some(&PreserveReason::KeepMarker)),
                (
                    "/// Parent element ID.",
                    Some(&PreserveReason::ExtendedByNeighbourMarker)
                ),
            ]
        );
    }

    #[test]
    fn keep_block_members_are_attributed_to_the_marker_they_borrow() {
        // The `// TODO` line has a reason of its own even though the marker also covers
        // it, so `keep` must not read it as marker-dependent.
        let source = "fn f() {\n    // TODO: later\n    // context line\n    // rationale ~keep\n    let x = 1;\n}\n";
        let mut processor = Processor::new();
        let inspected = processor
            .inspect(source, Path::new("sample.rs"), &default_resolved_config())
            .expect("inspect");

        let reasons: Vec<Option<&PreserveReason>> = inspected.iter().map(|comment| comment.reason.as_ref()).collect();
        assert_eq!(
            reasons,
            vec![
                Some(&PreserveReason::Pattern("TODO".to_string())),
                Some(&PreserveReason::ExtendedByNeighbourMarker),
                Some(&PreserveReason::KeepMarker),
            ]
        );
    }

    #[test]
    fn inspect_names_shebang_and_pattern_reasons() {
        let source = "#!/usr/bin/env bash\n# NOTE: load bearing\n# remove me\necho ok\n";
        let mut processor = Processor::new();
        let inspected = processor
            .inspect(source, Path::new("script.sh"), &default_resolved_config())
            .expect("inspect");
        assert_eq!(inspected[0].reason, Some(PreserveReason::Shebang));
        assert_eq!(inspected[1].reason, Some(PreserveReason::Pattern("NOTE".to_string())));
        assert!(matches!(inspected[2].verdict, Verdict::Remove { .. }));
    }

    #[test]
    fn grammar_forced_preservation_is_named_a_language_directive() {
        // The trailing `/* GUARD */` survives because the C handler recognises a
        // comment trailing a preprocessor line, not because any pattern matched it.
        let source = "#ifndef GUARD\n#define GUARD\n// remove me\nint x;\n#endif  /* GUARD */\n";
        let mut processor = Processor::new();
        let inspected = processor
            .inspect(source, Path::new("header.h"), &default_resolved_config())
            .expect("inspect");
        let guard = inspected
            .iter()
            .find(|comment| comment.text.contains("GUARD */"))
            .expect("guard comment inspected");
        assert_eq!(guard.reason, Some(PreserveReason::LanguageDirective));
        assert_eq!(guard.verdict, Verdict::Preserve);
    }

    #[test]
    fn inspect_expanded_range_swallows_a_standalone_comment_line() {
        let source = "// standalone\nfn main() {\n    let x = 1; // trailing\n}\n";
        let mut processor = Processor::new();
        let inspected = processor
            .inspect(source, Path::new("sample.rs"), &default_resolved_config())
            .expect("inspect");
        let ranges: Vec<&str> = inspected
            .iter()
            .filter_map(|comment| match comment.verdict {
                Verdict::Remove {
                    expanded_start,
                    expanded_end,
                } => Some(&source[expanded_start..expanded_end]),
                Verdict::Preserve => None,
            })
            .collect();
        assert_eq!(ranges, vec!["// standalone\n", "// trailing"]);
    }

    #[test]
    fn inspect_unsupported_extension_errors() {
        let mut processor = Processor::new();
        let result = processor.inspect("noop", Path::new("file.unknownext"), &default_resolved_config());
        assert!(result.is_err());
    }
}
