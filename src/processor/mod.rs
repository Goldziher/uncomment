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
    /// the now-blank line. A comment that *trails* code takes the horizontal
    /// whitespace separating it from that code as well — it exists only to hold the
    /// comment off the code, and leaving it behind fails every trailing-whitespace
    /// lint. Returns `None` for degenerate ranges (empty or past the end of
    /// `bytes`); otherwise the expanded range, or the original span when code sits
    /// on both sides of the comment.
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
        } else if after_ws {
            Some((start - trailing_horizontal_whitespace(before), end))
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
            PreservationRule::PatternCaseInsensitive(pattern) => PreserveReason::Pattern(pattern.as_ref().to_string()),
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

/// Length of the run of spaces and tabs at the end of `bytes`. Only horizontal whitespace counts:
/// a newline is the line boundary, not a separator, and swallowing it would splice two lines.
fn trailing_horizontal_whitespace(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .rev()
        .take_while(|byte| matches!(byte, b' ' | b'\t'))
        .count()
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
mod tests;
