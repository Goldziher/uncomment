//! `uncomment keep` — writing a decision taken over a `scan` inventory back into the source.
//!
//! A marker only works if it lands somewhere the rest of the tool actually reads it, and there are
//! two shapes. A line comment takes ` ~keep` appended to its own text. A block, doc or docstring
//! comment takes a standalone marker line directly above it, at its own indentation, written with
//! the language's plain line-comment token. Which applies is not a matter of taste:
//!
//! - A Python docstring is a `string` node whose bytes *are* `__doc__` at runtime, so editing it
//!   in-body changes what the program reports about itself.
//! - [`CommentVisitor::redundant_keep_markers`](crate::ast::visitor::CommentVisitor::redundant_keep_markers)
//!   strips a bare `~keep` back out of a *doc* comment whenever docs are being preserved, so an
//!   in-body marker there is undone on the next run.
//! - A marker line written with a documentation prefix (`///`, `//!`, `/**`, `##`) is classified as
//!   documentation rather than as a marker, and does nothing at all — silently.
//!
//! None of that reasoning is trusted. Every marker this module writes is *proved* load-bearing: the
//! rewritten file is re-inspected under a config where a `~keep` marker is the only thing that can
//! preserve a comment, and a marker that fails to preserve its target is discarded and the comment
//! reported as unmarkable rather than written.

use crate::ast::visitor::CommentInfo;
use crate::config::{Config, ConfigManager, ResolvedConfig};
use crate::edit::{Edit, apply_edits};
use crate::languages::registry::LanguageRegistry;
use crate::paths::{absolute_normalized, find_repo_root, normalize_lexical, repo_relative, to_slash};
use crate::processor::{CommentKind, InspectedComment, PreserveReason, ProcessingOptions, Processor, Verdict};
use crate::rules::preservation::PreservationRule;
use crate::scan::id::{ambiguous_ids, assign_occurrence_indices, comment_id, full_id};
use crate::ui;
use anyhow::{Context, Result, bail};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

/// The marker token that protects a comment from removal.
pub const KEEP_MARKER: &str = "~keep";

/// `~keep` with the single leading space an appended marker carries.
const APPENDED_MARKER: &str = " ~keep";

#[derive(clap::Args, Debug)]
pub struct KeepArgs {
    #[command(flatten)]
    pub process: crate::cli::ProcessArgs,

    /// Read comment ids from a `scan` output file (JSONL or JSON; every field but `id` is ignored)
    #[arg(
        long = "from",
        value_name = "FILE",
        help = "Read comment ids from a scan output file (JSONL or JSON)",
        help_heading = "Comment selection"
    )]
    pub from: Option<PathBuf>,

    /// Mark the comment with this id (repeatable)
    #[arg(
        long = "id",
        value_name = "ID",
        help = "Mark the comment with this id (can be used multiple times)",
        help_heading = "Comment selection"
    )]
    pub ids: Vec<String>,

    /// Mark every comment whose text contains SUBSTRING
    #[arg(
        long = "match",
        value_name = "SUBSTRING",
        help = "Mark every comment whose text contains SUBSTRING",
        help_heading = "Comment selection"
    )]
    pub match_text: Option<String>,

    /// Mark every comment a default run would remove
    #[arg(
        long = "all-removable",
        help = "Mark every comment that would be removed",
        help_heading = "Comment selection"
    )]
    pub all_removable: bool,

    /// Warn about ids that no longer resolve instead of failing
    #[arg(
        long = "skip-missing",
        help = "Warn about ids that no longer resolve instead of failing",
        help_heading = "Comment selection"
    )]
    pub skip_missing: bool,
}

/// Apply `~keep` markers to the selected comments.
///
/// # Errors
///
/// Fails when no selector was given, when a requested id no longer resolves (unless
/// `--skip-missing`), when a requested id is ambiguous, when a comment requested by id cannot take a
/// marker, and on any I/O or parse failure.
pub fn run(args: &KeepArgs) -> Result<()> {
    let requests = IdRequests::collect(args)?;
    if !args.all_removable && args.match_text.is_none() && requests.ordered.is_empty() && args.from.is_none() {
        bail!("nothing selected: pass --from FILE, --id ID, --match SUBSTRING or --all-removable");
    }

    let options = args.process.processing_options();
    let current_dir = std::env::current_dir().context("Failed to get current directory")?;
    let config_manager = match &args.process.config {
        Some(path) => {
            let config =
                Config::from_file(path).with_context(|| format!("Failed to load config file: {}", path.display()))?;
            ConfigManager::from_single_config(current_dir.clone(), config)?
        }
        None => ConfigManager::new(&current_dir).context("Failed to initialize configuration manager")?,
    };

    let mut registry = LanguageRegistry::new();
    registry.register_configured_languages(&config_manager.get_all_languages());

    let paths: Cow<'_, [String]> = if args.process.paths.is_empty() {
        Cow::Owned(vec![".".to_string()])
    } else {
        Cow::Borrowed(&args.process.paths)
    };
    let files = collect_files(&paths, options.respect_gitignore, &registry)?;

    let mut processor = Processor::new_with_config(&config_manager);
    let mut inventory: Vec<FileWork> = Vec::new();
    for path in &files {
        match FileWork::read(path, &registry, &config_manager, &options, &mut processor) {
            Ok(Some(work)) => inventory.push(work),
            Ok(None) => {}
            Err(error) => anstream::eprintln!("{} {}: {error}", ui::danger("error"), ui::path(path)),
        }
    }

    let index = IdIndex::build(&inventory);
    let mut selected: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    let mut explicit: BTreeSet<(usize, usize)> = BTreeSet::new();
    let mut unresolved: Vec<&IdRequest> = Vec::new();
    let mut ambiguous: Vec<&IdRequest> = Vec::new();

    for request in &requests.ordered {
        if index.ambiguous.contains(&request.id) {
            ambiguous.push(request);
            continue;
        }
        match index.owners.get(&request.id) {
            Some(&(file, comment)) => {
                selected.entry(file).or_default().insert(comment);
                explicit.insert((file, comment));
            }
            None => unresolved.push(request),
        }
    }

    for (file, work) in inventory.iter().enumerate() {
        for (comment, inspected) in work.comments.iter().enumerate() {
            let wanted = args.all_removable && matches!(inspected.verdict, Verdict::Remove { .. })
                || args
                    .match_text
                    .as_ref()
                    .is_some_and(|needle| inspected.text.contains(needle.as_str()));
            if wanted {
                selected.entry(file).or_default().insert(comment);
            }
        }
    }

    // Reading the inventory is what pulls in the configs below the invocation directory, and a
    // config that failed to load is recorded there rather than returned. Marking under built-in
    // defaults would mark the wrong comments, so stop before anything is written.
    if let Some(error) = config_manager.deferred_config_error() {
        bail!("{error}");
    }

    let mut summary = Summary {
        unresolved: unresolved.len(),
        ..Summary::default()
    };
    let mut blocked_by_id = 0usize;

    for (&file, comments) in &selected {
        let work = &inventory[file];
        let outcome = apply_markers(
            &work.content,
            &work.path,
            &work.comments,
            comments,
            work.syntax,
            &mut processor,
            &work.config,
        )
        .with_context(|| format!("Failed to apply markers to {}", work.path.display()))?;

        summary.marked += outcome.marked();
        summary.already_marked += outcome.already_marked.len();
        summary.unmarkable += outcome.unmarkable.len();

        for &(comment, reason) in &outcome.unmarkable {
            let line = work.comments[comment].start_row + 1;
            anstream::eprintln!(
                "{} {}:{line}: cannot mark this comment — {reason}",
                ui::warn("unmarkable:"),
                ui::path(&work.path)
            );
            if explicit.contains(&(file, comment)) {
                blocked_by_id += 1;
            }
        }

        if outcome.placements.is_empty() {
            continue;
        }

        if args.process.diff {
            print_diff(&work.path, &work.content, &outcome.placements);
        }

        if args.process.dry_run {
            report_file(args, &work.path, outcome.marked(), "would mark");
        } else {
            std::fs::write(&work.path, &outcome.content)
                .with_context(|| format!("Failed to write {}", work.path.display()))?;
            report_file(args, &work.path, outcome.marked(), "marked");
        }
    }

    if args.skip_missing {
        for request in &unresolved {
            anstream::eprintln!(
                "{} id {} from {} no longer resolves to a comment",
                ui::warn("warning:"),
                request.id,
                request.source
            );
        }
    }

    summary.print(args.process.quiet, args.process.dry_run);

    // The ids and where they came from go in the error itself, not only on stderr: an id that stopped
    // resolving is the one thing the caller has to act on, and an exit code alone does not say which.
    if !ambiguous.is_empty() {
        bail!(
            "{} ambiguous id(s), refusing to guess which comment is meant; re-run scan to widen them:\n{}",
            ambiguous.len(),
            render_requests(&ambiguous)
        );
    }
    if !unresolved.is_empty() && !args.skip_missing {
        bail!(
            "{} id(s) no longer resolve to a comment (pass --skip-missing to continue anyway):\n{}",
            unresolved.len(),
            render_requests(&unresolved)
        );
    }
    if blocked_by_id > 0 {
        bail!("{blocked_by_id} comment(s) requested by id cannot take a marker");
    }
    Ok(())
}

fn render_requests(requests: &[&IdRequest]) -> String {
    requests
        .iter()
        .map(|request| format!("  {} (from {})", request.id, request.source))
        .collect::<Vec<String>>()
        .join("\n")
}

fn report_file(args: &KeepArgs, path: &Path, count: usize, verb: &str) {
    if args.process.quiet {
        return;
    }
    anstream::println!(
        "{} {verb} {} comment(s) in {}",
        ui::success(ui::CHECK),
        ui::accent(count),
        ui::path(path)
    );
}

#[derive(Debug, Default)]
struct Summary {
    marked: usize,
    already_marked: usize,
    unmarkable: usize,
    unresolved: usize,
}

impl Summary {
    fn print(&self, quiet: bool, dry_run: bool) {
        if quiet {
            return;
        }
        let lead = if dry_run { "would mark" } else { "marked" };
        anstream::println!(
            "{} {} {}, {} already marked, {} unmarkable, {} unresolved",
            ui::dim(ui::BULLET),
            ui::accent(self.marked),
            lead,
            ui::accent(self.already_marked),
            ui::accent(self.unmarkable),
            ui::accent(self.unresolved)
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Marker placement
// ---------------------------------------------------------------------------------------------

/// Where one comment's marker goes and what text lands there. Always an insertion, never a
/// replacement: nothing in a comment body is reflowed, re-indented or rewritten.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placement {
    /// Append ` ~keep` inside the comment's own text, at this offset.
    Append { offset: usize },
    /// Insert a whole marker line at this offset, which is always a line start.
    AboveLine { offset: usize, text: String },
}

impl Placement {
    fn offset(&self) -> usize {
        match self {
            Placement::Append { offset } | Placement::AboveLine { offset, .. } => *offset,
        }
    }

    fn edit(&self) -> Edit {
        match self {
            Placement::Append { offset } => Edit::insert(*offset, APPENDED_MARKER),
            Placement::AboveLine { offset, text } => Edit::insert(*offset, text.clone()),
        }
    }
}

/// Why a comment cannot take a marker. Reported with path and line rather than guessed around: a
/// marker in the wrong place is either inert or actively wrong, and both are worse than a refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unmarkable {
    /// The language has neither a line-comment token nor a block pair, so no marker line can be
    /// written.
    NoMarkerToken,
    /// Every comment token the language has is itself classified as documentation.
    DocPrefixToken,
    /// Code shares the comment's line, so a marker line above it would not attach to it.
    NotStandalone,
    /// The comment is load-bearing syntax — a shebang or a language directive — whose text cannot be
    /// extended without changing what it means.
    Directive,
    /// The marker was written and did not preserve the comment when the result was re-inspected.
    MarkerDidNotAttach,
}

impl fmt::Display for Unmarkable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Unmarkable::NoMarkerToken => "no comment token is known for this language to write a marker with",
            Unmarkable::DocPrefixToken => "this language's comment tokens all read as documentation",
            Unmarkable::NotStandalone => "code shares its line, so a marker line above it would not attach",
            Unmarkable::Directive => "it is a shebang or language directive, whose text cannot be extended",
            Unmarkable::MarkerDidNotAttach => "the marker did not preserve it when the result was re-inspected",
        };
        f.write_str(message)
    }
}

/// The literal comment tokens a marker line can be written with: the plain line form (`//`, `#`,
/// `--`, `;`, `%`) when the language has one, and the plain block pair (`("/*", "*/")`,
/// `("<!--", "-->")`) otherwise. A language may have either, both, or neither.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MarkerSyntax<'a> {
    pub line: Option<&'a str>,
    pub block: Option<(&'a str, &'a str)>,
}

impl MarkerSyntax<'_> {
    /// Every marker line this syntax could produce, line form first because [`Self::marker_line_text`]
    /// prefers it.
    ///
    /// Shared with [`is_emitted_marker_line`] so that the text this module writes and the text it later
    /// recognises as its own cannot drift apart.
    fn marker_line_candidates(self) -> [Option<String>; 2] {
        [
            self.line.map(|token| format!("{token} {KEEP_MARKER}")),
            self.block.map(|(open, close)| format!("{open} {KEEP_MARKER} {close}")),
        ]
    }

    /// The marker comment to put on its own line, preferring the line form because it is the shorter
    /// and the more conventional of the two.
    ///
    /// A token whose marker would be classified as documentation is skipped rather than written: such
    /// a marker is read as documentation and preserves nothing, so writing it would be silently inert.
    fn marker_line_text(self) -> std::result::Result<String, Unmarkable> {
        let mut had_token = false;
        for candidate in self.marker_line_candidates().into_iter().flatten() {
            had_token = true;
            if !reads_as_documentation(&candidate) {
                return Ok(candidate);
            }
        }

        Err(if had_token {
            Unmarkable::DocPrefixToken
        } else {
            Unmarkable::NoMarkerToken
        })
    }
}

/// Where a marker for `comment` belongs, or why it cannot have one.
///
/// `line_ending` is the terminator new lines are written with, so a CRLF file stays CRLF.
pub fn plan_marker(
    content: &str,
    comment: &InspectedComment,
    syntax: MarkerSyntax<'_>,
    line_ending: &str,
) -> std::result::Result<Placement, Unmarkable> {
    if matches!(
        comment.reason,
        Some(PreserveReason::Shebang | PreserveReason::LanguageDirective)
    ) {
        return Err(Unmarkable::Directive);
    }

    match comment.kind {
        CommentKind::Line => Ok(Placement::Append {
            offset: text_end(content, comment.start_byte, comment.end_byte),
        }),
        CommentKind::Block | CommentKind::Doc | CommentKind::Docstring => {
            let marker = syntax.marker_line_text()?;
            if !is_standalone(content, comment.start_byte) {
                return Err(Unmarkable::NotStandalone);
            }
            let line_start = line_start_of(content, comment.start_byte);
            let indent = &content[line_start..comment.start_byte];
            Ok(Placement::AboveLine {
                offset: line_start,
                text: format!("{indent}{marker}{line_ending}"),
            })
        }
    }
}

/// Whether `text` would be classified as documentation, and so read as documentation rather than as
/// a marker.
///
/// Asked of the real classifier instead of a hand-kept list of prefixes, because the list that
/// matters is the one [`PreservationRule::Documentation`] applies.
fn reads_as_documentation(text: &str) -> bool {
    let probe = CommentInfo {
        start_byte: 0,
        end_byte: text.len(),
        start_row: 0,
        end_row: 0,
        node_type: "comment".to_string(),
        should_preserve: false,
        is_documentation: false,
    };
    PreservationRule::documentation().matches(&probe, text)
}

/// Offset just past the comment's last non-whitespace byte, so ` ~keep` is appended to the text
/// rather than after a trailing `\r` or a node that swallowed its own newline.
fn text_end(content: &str, start: usize, end: usize) -> usize {
    let trimmed = content[start..end].trim_end();
    if trimmed.is_empty() { end } else { start + trimmed.len() }
}

/// Offset of the first byte of the line containing `offset`.
fn line_start_of(content: &str, offset: usize) -> usize {
    content[..offset].rfind('\n').map_or(0, |position| position + 1)
}

/// The terminator to write new lines with: CRLF when the file is predominantly CRLF, LF otherwise.
fn dominant_line_ending(content: &str) -> &'static str {
    let crlf = content.matches("\r\n").count();
    let newlines = content.matches('\n').count();
    if crlf * 2 > newlines { "\r\n" } else { "\n" }
}

/// Whether `gap` separates two comments by nothing but the line break, mirroring the adjacency rule
/// [`CommentVisitor::extend_keep_above`](crate::ast::visitor::CommentVisitor::extend_keep_above)
/// applies — a marker only reaches a comment across such a gap.
fn gap_is_adjacent(gap: &str) -> bool {
    gap.bytes().all(|byte| byte.is_ascii_whitespace()) && gap.bytes().filter(|&byte| byte == b'\n').count() <= 1
}

/// What marking one file produced.
#[derive(Debug, Default)]
pub struct MarkOutcome {
    /// The rewritten content. Equal to the input when nothing was marked.
    pub content: String,
    /// Comment index and its marker, in source order.
    pub placements: Vec<(usize, Placement)>,
    /// Comments this run protected with a marker written for an adjacent comment rather than one of
    /// their own.
    pub covered: Vec<usize>,
    /// Comments a marker already protects, which are left exactly as they are.
    pub already_marked: Vec<usize>,
    /// Comments that could not take a marker, with why.
    pub unmarkable: Vec<(usize, Unmarkable)>,
}

impl MarkOutcome {
    /// How many comments this run newly protected.
    #[must_use]
    pub fn marked(&self) -> usize {
        self.placements.len() + self.covered.len()
    }
}

/// One comment queued for marking. `placement` is `None` for a comment an adjacent comment's marker
/// already reaches, which still has to be verified but needs no edit of its own.
#[derive(Debug)]
struct Candidate {
    comment: usize,
    placement: Option<Placement>,
    /// The placement [`elide_covered_markers`] dropped, kept so a coverage judgement that verification
    /// disproves can be undone rather than costing the comment its marker.
    elided: Option<Placement>,
}

/// Plan, apply and then *verify* markers for `selected` in one pass over `content`.
///
/// Verification is the point: after the edits are applied the result is re-inspected under
/// [`marker_only_config`], where a `~keep` marker is the only thing that can preserve a comment. A
/// target that is not preserved there did not actually get protected — the marker prefix was
/// classified as documentation, the grammar refused to read it as standalone, the gap was too wide —
/// so a marker elided as covered is restored first, and only a target that still fails has its edit
/// dropped and is reported unmarkable. The loop repeats until every remaining marker holds, which
/// terminates because each round either restores a candidate (possible once per candidate) or discards
/// at least one.
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_markers(
    content: &str,
    path: &Path,
    comments: &[InspectedComment],
    selected: &BTreeSet<usize>,
    syntax: MarkerSyntax<'_>,
    processor: &mut Processor,
    config: &ResolvedConfig,
) -> Result<MarkOutcome> {
    let strict = marker_only_config(config);
    let protected = marker_protection(processor, content, path, &strict)?;
    let line_ending = dominant_line_ending(content);

    let mut outcome = MarkOutcome::default();
    let mut candidates: Vec<Candidate> = Vec::new();
    for &index in selected {
        let comment = &comments[index];
        if is_marker_protected(&protected, comment) {
            outcome.already_marked.push(index);
            continue;
        }
        match plan_marker(content, comment, syntax, line_ending) {
            Ok(placement) => candidates.push(Candidate {
                comment: index,
                placement: Some(placement),
                elided: None,
            }),
            Err(reason) => outcome.unmarkable.push((index, reason)),
        }
    }
    candidates.sort_by_key(|candidate| {
        (
            candidate.placement.as_ref().map_or(0, Placement::offset),
            comments[candidate.comment].start_byte,
        )
    });
    elide_covered_markers(content, comments, &mut candidates);

    loop {
        let edits: Vec<Edit> = candidates
            .iter()
            .filter_map(|candidate| candidate.placement.as_ref().map(Placement::edit))
            .collect();
        let rewritten = apply_edits(content, edits.clone())?;

        let mut failed: BTreeSet<usize> = BTreeSet::new();
        if !candidates.is_empty() {
            let after = marker_protection(processor, &rewritten, path, &strict)?;
            for candidate in &candidates {
                let comment = &comments[candidate.comment];
                let start = shifted(&edits, comment.start_byte);
                let end = shifted(&edits, comment.end_byte);
                if !after
                    .get(&(start, end))
                    .copied()
                    .unwrap_or_else(|| protection_at(&after, start))
                {
                    failed.insert(candidate.comment);
                }
            }
        }

        if failed.is_empty() {
            outcome.content = rewritten;
            for candidate in candidates {
                match candidate.placement {
                    Some(placement) => outcome.placements.push((candidate.comment, placement)),
                    None => outcome.covered.push(candidate.comment),
                }
            }
            outcome.unmarkable.sort_by_key(|&(index, _)| index);
            return Ok(outcome);
        }

        // A comment whose own marker was elided as covered may simply have been misjudged as covered.
        // Give it its marker back before concluding that no marker can protect it. Each candidate is
        // restored at most once, so the loop still terminates.
        let mut restored = false;
        for candidate in &mut candidates {
            if failed.contains(&candidate.comment)
                && candidate.placement.is_none()
                && let Some(placement) = candidate.elided.take()
            {
                candidate.placement = Some(placement);
                restored = true;
            }
        }
        if restored {
            continue;
        }

        candidates.retain(|candidate| !failed.contains(&candidate.comment));
        outcome
            .unmarkable
            .extend(failed.into_iter().map(|index| (index, Unmarkable::MarkerDidNotAttach)));
    }
}

/// Drop the marker line for a comment an earlier marker in the same batch already reaches.
///
/// `extend_keep_above` runs forward from a marker through every comment separated from the last by
/// nothing but one line break, so a single marker above a run of `///` lines protects the whole run.
/// A second marker line inside that run would be inert clutter, and would split the run in two for
/// every tool that reads consecutive doc lines as one block.
fn elide_covered_markers(content: &str, comments: &[InspectedComment], candidates: &mut [Candidate]) {
    let mut run_end: Option<usize> = None;
    for candidate in candidates.iter_mut() {
        let comment = &comments[candidate.comment];
        let continues =
            run_end.is_some_and(|end| end <= comment.start_byte && gap_is_adjacent(&content[end..comment.start_byte]));
        if continues && matches!(candidate.placement, Some(Placement::AboveLine { .. })) {
            candidate.elided = candidate.placement.take();
        }
        run_end = if continues {
            Some(run_end.map_or(comment.end_byte, |end| end.max(comment.end_byte)))
        } else if anchors_a_run(content, comment, candidate.placement.as_ref()) {
            Some(comment.end_byte)
        } else {
            None
        };
    }
}

/// Whether the marker this batch writes for `comment` is read as a marker line, and so extends
/// forward to the comments beneath it.
///
/// [`CommentVisitor::is_keep_marker_line`](crate::ast::visitor::CommentVisitor) requires a
/// *standalone*, non-documentation comment, so ` ~keep` appended to a trailing comment (`code // x`)
/// protects that comment alone and reaches nothing below it. Treating it as an anchor would elide the
/// marker the comment beneath still needs.
fn anchors_a_run(content: &str, comment: &InspectedComment, placement: Option<&Placement>) -> bool {
    match placement {
        // The inserted line is standalone and non-documentation by construction.
        Some(Placement::AboveLine { .. }) => true,
        Some(Placement::Append { .. }) => is_standalone(content, comment.start_byte),
        None => false,
    }
}

/// Whether only whitespace precedes `start` on its line.
fn is_standalone(content: &str, start: usize) -> bool {
    content[line_start_of(content, start)..start]
        .bytes()
        .all(|byte| byte.is_ascii_whitespace())
}

/// Whether the comment at `start` in a re-inspected file is protected by a marker, used when the
/// exact `(start, end)` pair is not present because the grammar reported a differently-sized node.
fn protection_at(protected: &BTreeMap<(usize, usize), bool>, start: usize) -> bool {
    protected
        .range((start, 0)..=(start, usize::MAX))
        .any(|(_, &is_protected)| is_protected)
}

/// A config under which a `~keep` marker is the only thing that preserves a comment.
///
/// Inspecting under it is how "is this comment protected by a marker?" is answered without a
/// substring search: documentation, TODO/FIXME and every default ignore are switched off, so a
/// preserved comment reports [`PreserveReason::KeepMarker`] or
/// [`PreserveReason::ExtendedByNeighbourMarker`] and nothing else can stand in for them.
fn marker_only_config(base: &ResolvedConfig) -> ResolvedConfig {
    ResolvedConfig {
        remove_todos: true,
        remove_fixme: true,
        remove_docs: true,
        preserve_patterns: Vec::new(),
        use_default_ignores: false,
        ..base.clone()
    }
}

/// Per-comment "a marker protects this", keyed by byte range.
fn marker_protection(
    processor: &mut Processor,
    content: &str,
    path: &Path,
    strict: &ResolvedConfig,
) -> Result<BTreeMap<(usize, usize), bool>> {
    let inspected = processor.inspect(content, path, strict)?;
    Ok(inspected
        .into_iter()
        .map(|comment| {
            let protected = matches!(
                comment.reason,
                Some(PreserveReason::KeepMarker | PreserveReason::ExtendedByNeighbourMarker)
            );
            ((comment.start_byte, comment.end_byte), protected)
        })
        .collect())
}

fn is_marker_protected(protected: &BTreeMap<(usize, usize), bool>, comment: &InspectedComment) -> bool {
    protected
        .get(&(comment.start_byte, comment.end_byte))
        .copied()
        .unwrap_or_else(|| protection_at(protected, comment.start_byte))
}

/// `offset` after every insertion at or before it has been applied. Exact because every edit this
/// module produces is a pure insertion.
fn shifted(edits: &[Edit], offset: usize) -> usize {
    offset
        + edits
            .iter()
            .filter(|edit| edit.start <= offset)
            .map(|edit| edit.replacement.len())
            .sum::<usize>()
}

// ---------------------------------------------------------------------------------------------
// Inventory and id resolution
// ---------------------------------------------------------------------------------------------

/// One file's comments, the ids they answer to, and the config they were inspected under.
struct FileWork {
    path: PathBuf,
    content: String,
    comments: Vec<InspectedComment>,
    config: ResolvedConfig,
    syntax: MarkerSyntax<'static>,
    /// Per comment, the ids that resolve to it.
    ids: Vec<Vec<String>>,
}

impl FileWork {
    fn read(
        path: &Path,
        registry: &LanguageRegistry,
        config_manager: &ConfigManager,
        options: &ProcessingOptions,
        processor: &mut Processor,
    ) -> Result<Option<Self>> {
        let Some(language) = registry.detect_language_arc(path) else {
            return Ok(None);
        };
        let content =
            std::fs::read_to_string(path).with_context(|| format!("Failed to read file: {}", path.display()))?;
        let language_name = language.name.to_lowercase();
        let config = resolve_config(config_manager, path, &language_name, options);
        let comments = processor.inspect(&content, path, &config)?;
        let syntax = MarkerSyntax {
            line: language.line_comment_token(),
            block: language.block_comment_delimiters(),
        };
        let ids = comment_ids(&id_path(path), &comments, syntax);

        Ok(Some(Self {
            path: path.to_path_buf(),
            content,
            comments,
            config,
            syntax,
            ids,
        }))
    }
}

/// Mirrors the CLI-override application in
/// [`Processor::process_file_with_config`](crate::processor::Processor::process_file_with_config).
/// Flags are applied one-directionally so an unset one never clobbers a config-file value.
fn resolve_config(
    config_manager: &ConfigManager,
    path: &Path,
    language_name: &str,
    options: &ProcessingOptions,
) -> ResolvedConfig {
    let mut config = config_manager.get_config_for_file_with_language(path, language_name);
    if options.remove_doc {
        config.remove_docs = true;
    }
    if !options.use_default_ignores {
        config.use_default_ignores = false;
    }
    if options.remove_todo {
        config.remove_todos = true;
    }
    if options.remove_fixme {
        config.remove_fixme = true;
    }
    if !options.custom_preserve_patterns.is_empty() {
        config
            .preserve_patterns
            .extend(options.custom_preserve_patterns.iter().cloned());
    }
    config
}

/// The path ids are computed against: repo-relative with `/` separators, matching what `scan` wrote.
fn id_path(path: &Path) -> String {
    let absolute = std::env::current_dir()
        .map(|dir| absolute_normalized(&dir, path))
        .unwrap_or_else(|_| normalize_lexical(path));
    find_repo_root(&absolute)
        .and_then(|root| repo_relative(&root, &absolute))
        .map_or_else(|| to_slash(&absolute), |relative| to_slash(&relative))
}

/// Every id that resolves to each comment: the id its current bytes produce, plus the id it had
/// *before* `keep` marked it.
///
/// The second is what makes `keep` idempotent through a decisions file. Appending ` ~keep` changes a
/// comment's bytes, and an id is a hash of those bytes, so a scan taken before the first run would
/// otherwise stop resolving and the second run would report every comment it just marked as missing.
/// Reconstructing the pre-marker view — marker lines this module emitted removed, appended markers
/// stripped, occurrence indices reassigned over *that* text — reproduces exactly the ids the earlier
/// scan wrote.
fn comment_ids(path: &str, comments: &[InspectedComment], syntax: MarkerSyntax<'_>) -> Vec<Vec<String>> {
    let texts: Vec<&str> = comments.iter().map(|comment| comment.text.as_str()).collect();
    let occurrences = assign_occurrence_indices(&texts);
    let mut ids: Vec<Vec<String>> = texts
        .iter()
        .zip(&occurrences)
        .map(|(text, &occurrence)| vec![comment_id(path, text, occurrence), full_id(path, text, occurrence)])
        .collect();

    let prior: Vec<Option<String>> = comments
        .iter()
        .map(|comment| {
            if is_emitted_marker_line(comment, syntax) {
                return None;
            }
            Some(unmarked_text(comment))
        })
        .collect();
    let surviving: Vec<usize> = (0..comments.len()).filter(|&index| prior[index].is_some()).collect();
    let prior_texts: Vec<&str> = surviving.iter().filter_map(|&index| prior[index].as_deref()).collect();
    let prior_occurrences = assign_occurrence_indices(&prior_texts);
    for (position, &index) in surviving.iter().enumerate() {
        let Some(text) = prior[index].as_deref() else {
            continue;
        };
        let occurrence = prior_occurrences.get(position).copied().unwrap_or(0);
        for id in [comment_id(path, text, occurrence), full_id(path, text, occurrence)] {
            if !ids[index].contains(&id) {
                ids[index].push(id);
            }
        }
    }
    ids
}

/// Whether this comment is a marker line this module wrote, and so did not exist before.
///
/// Every form the language could have produced counts, not only the one [`MarkerSyntax::marker_line_text`]
/// would choose today. A marker line mistaken for a pre-existing comment is reconstructed into the
/// pre-marker view with its ` ~keep` stripped out, where it claims the id of any real comment with that
/// text — and two comments answering to one id is what [`IdIndex`] reports as ambiguous, so `keep`
/// refuses to mark the comment the id was written for.
fn is_emitted_marker_line(comment: &InspectedComment, syntax: MarkerSyntax<'_>) -> bool {
    let text = comment.text.trim();
    syntax
        .marker_line_candidates()
        .iter()
        .flatten()
        .any(|candidate| candidate == text)
}

/// The comment's text with one appended marker removed, for a comment a marker of its own preserves.
fn unmarked_text(comment: &InspectedComment) -> String {
    if comment.reason == Some(PreserveReason::KeepMarker) && comment.text.contains(APPENDED_MARKER) {
        return comment.text.replacen(APPENDED_MARKER, "", 1);
    }
    comment.text.clone()
}

/// Which comment each id names, and which ids name more than one.
struct IdIndex {
    owners: BTreeMap<String, (usize, usize)>,
    ambiguous: BTreeSet<String>,
}

impl IdIndex {
    fn build(inventory: &[FileWork]) -> Self {
        let mut pairs: BTreeSet<(String, usize, usize)> = BTreeSet::new();
        for (file, work) in inventory.iter().enumerate() {
            for (comment, ids) in work.ids.iter().enumerate() {
                for id in ids {
                    pairs.insert((id.clone(), file, comment));
                }
            }
        }

        let flat: Vec<&str> = pairs.iter().map(|(id, _, _)| id.as_str()).collect();
        let ambiguous: BTreeSet<String> = ambiguous_ids(&flat).into_iter().collect();
        let owners = pairs
            .into_iter()
            .filter(|(id, _, _)| !ambiguous.contains(id))
            .map(|(id, file, comment)| (id, (file, comment)))
            .collect();
        Self { owners, ambiguous }
    }
}

/// One requested id and where it came from, so an id that no longer resolves can be reported against
/// the file that asked for it.
struct IdRequest {
    id: String,
    source: String,
}

struct IdRequests {
    ordered: Vec<IdRequest>,
}

impl IdRequests {
    fn collect(args: &KeepArgs) -> Result<Self> {
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut ordered: Vec<IdRequest> = Vec::new();

        if let Some(path) = &args.from {
            for id in ids_from_decisions(path)? {
                if seen.insert(id.clone()) {
                    ordered.push(IdRequest {
                        id,
                        source: path.display().to_string(),
                    });
                }
            }
        }
        for id in &args.ids {
            if seen.insert(id.clone()) {
                ordered.push(IdRequest {
                    id: id.clone(),
                    source: "--id".to_string(),
                });
            }
        }
        Ok(Self { ordered })
    }
}

/// Ids in a `scan` output file, which may be a JSON document or JSONL.
///
/// Every field but `id` is ignored, so a decisions file is literally the scan output with the lines
/// for comments that should be removed deleted.
fn ids_from_decisions(path: &Path) -> Result<Vec<String>> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("Failed to read decisions file: {}", path.display()))?;
    let mut ids = Vec::new();

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
        collect_ids(&value, &mut ids);
        return Ok(ids);
    }

    for (number, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: serde_json::Value =
            serde_json::from_str(line).with_context(|| format!("{}:{}: not valid JSON", path.display(), number + 1))?;
        collect_ids(&value, &mut ids);
    }
    Ok(ids)
}

fn collect_ids(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for key in ["id", "full_id"] {
                if let Some(serde_json::Value::String(id)) = map.get(key) {
                    out.push(id.clone());
                    break;
                }
            }
            for nested in map.values() {
                collect_ids(nested, out);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_ids(item, out);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// File discovery and diff output
// ---------------------------------------------------------------------------------------------

fn collect_files(paths: &[String], respect_gitignore: bool, registry: &LanguageRegistry) -> Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = Vec::new();

    for pattern in paths {
        let path = Path::new(pattern);
        if path.is_file() {
            files.push(path.to_path_buf());
        } else if path.is_dir() {
            let walker = ignore::WalkBuilder::new(path)
                .hidden(false)
                .git_ignore(respect_gitignore)
                .git_global(respect_gitignore)
                .git_exclude(respect_gitignore)
                .parents(respect_gitignore)
                .require_git(false)
                .build();
            for entry in walker {
                match entry {
                    Ok(entry) if entry.path().is_file() => files.push(entry.path().to_path_buf()),
                    Ok(_) => {}
                    Err(error) => anstream::eprintln!("{} reading path: {error}", ui::danger("error")),
                }
            }
        } else {
            for entry in glob::glob(pattern).context("Failed to parse glob pattern")? {
                match entry {
                    Ok(found) if found.is_file() => files.push(found),
                    Ok(_) => {}
                    Err(error) => anstream::eprintln!("{} reading path: {error}", ui::danger("error")),
                }
            }
        }
    }

    files.retain(|path| registry.detect_language(path).is_some());
    files.sort();
    files.dedup();
    Ok(files)
}

/// Show each marker as a one-line diff against the original content.
fn print_diff(path: &Path, content: &str, placements: &[(usize, Placement)]) {
    anstream::println!("{}", ui::bold(ui::path(path)));
    for (_, placement) in placements {
        let offset = placement.offset();
        let line_number = content[..offset].matches('\n').count() + 1;
        match placement {
            Placement::AboveLine { text, .. } => {
                anstream::println!("  {line_number} {}", ui::success(format!("+{}", text.trim_end())));
            }
            Placement::Append { .. } => {
                let start = line_start_of(content, offset);
                let end = content[offset..]
                    .find('\n')
                    .map_or(content.len(), |position| offset + position);
                let old = content[start..end].trim_end_matches('\r');
                let mut new = String::with_capacity(old.len() + APPENDED_MARKER.len());
                new.push_str(&content[start..offset]);
                new.push_str(APPENDED_MARKER);
                new.push_str(content[offset..end].trim_end_matches('\r'));
                anstream::println!("  {line_number} {}", ui::danger(format!("-{old}")));
                anstream::println!("  {line_number} {}", ui::success(format!("+{new}")));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> ResolvedConfig {
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

    /// The marker syntax the registry reports for `file_name`, so a test asks the same question the
    /// command does rather than restating a token the language definition owns.
    fn syntax_for(file_name: &str) -> MarkerSyntax<'static> {
        let path = PathBuf::from(file_name);
        let registry = LanguageRegistry::new();
        let language = registry
            .detect_language(&path)
            .unwrap_or_else(|| panic!("no language for {file_name}"));
        MarkerSyntax {
            line: language.line_comment_token(),
            block: language.block_comment_delimiters(),
        }
    }

    /// Mark the comments `select` accepts in `content`, returning the rewritten text and the outcome.
    fn mark_with(
        content: &str,
        file_name: &str,
        remove_docs: bool,
        select: impl Fn(&InspectedComment) -> bool,
    ) -> (String, MarkOutcome, Vec<InspectedComment>) {
        let path = PathBuf::from(file_name);
        let syntax = syntax_for(file_name);

        let mut config = default_config();
        config.remove_docs = remove_docs;

        let mut processor = Processor::new();
        let comments = processor.inspect(content, &path, &config).expect("inspect");
        let selected: BTreeSet<usize> = comments
            .iter()
            .enumerate()
            .filter(|(_, comment)| select(comment))
            .map(|(index, _)| index)
            .collect();

        let outcome = apply_markers(content, &path, &comments, &selected, syntax, &mut processor, &config)
            .expect("apply markers");
        (outcome.content.clone(), outcome, comments)
    }

    fn mark_removable(content: &str, file_name: &str) -> String {
        mark_with(content, file_name, false, |comment| {
            matches!(comment.verdict, Verdict::Remove { .. })
        })
        .0
    }

    /// Whether a default run would remove anything from `content`.
    fn removable_count(content: &str, file_name: &str) -> usize {
        removable_count_with(content, file_name, false)
    }

    /// As [`removable_count`], for a run that also removes documentation.
    fn removable_count_with(content: &str, file_name: &str, remove_docs: bool) -> usize {
        let path = PathBuf::from(file_name);
        let mut config = default_config();
        config.remove_docs = remove_docs;
        let mut processor = Processor::new();
        processor
            .inspect(content, &path, &config)
            .expect("inspect")
            .iter()
            .filter(|comment| matches!(comment.verdict, Verdict::Remove { .. }))
            .count()
    }

    #[test]
    fn a_line_comment_takes_the_marker_in_its_own_text() {
        let before = "// legacy shim\nfn main() {}\n";
        let after = mark_removable(before, "a.rs");
        assert_eq!(after, "// legacy shim ~keep\nfn main() {}\n");
        assert_eq!(removable_count(&after, "a.rs"), 0);
    }

    #[test]
    fn a_trailing_line_comment_takes_the_marker_before_no_whitespace() {
        let before = "let x = 1; // why\n";
        let after = mark_removable(before, "a.rs");
        assert_eq!(after, "let x = 1; // why ~keep\n");
    }

    #[test]
    fn a_rust_doc_comment_takes_a_plain_marker_line_above_it() {
        let before = "/// Public API.\npub fn f() {}\n";
        let after = mark_with(before, "a.rs", true, |comment| comment.kind == CommentKind::Doc).0;
        assert_eq!(after, "// ~keep\n/// Public API.\npub fn f() {}\n");
        // Not `/// ~keep`: a doc prefix is classified as documentation, never read as a marker.
        assert!(after.starts_with("// ~keep\n"), "{after}");
    }

    #[test]
    fn a_python_docstring_is_never_edited_in_body() {
        let before = "def f():\n    \"\"\"Explain f.\"\"\"\n    return 1\n";
        let (after, outcome, comments) =
            mark_with(before, "a.py", true, |comment| comment.kind == CommentKind::Docstring);
        assert_eq!(
            after,
            "def f():\n    # ~keep\n    \"\"\"Explain f.\"\"\"\n    return 1\n"
        );
        assert_eq!(outcome.placements.len(), 1);

        let docstring = comments
            .iter()
            .find(|comment| comment.kind == CommentKind::Docstring)
            .expect("docstring");
        assert!(
            after.contains(&docstring.text),
            "docstring bytes changed: {:?}",
            docstring.text
        );
    }

    #[test]
    fn a_c_block_comment_takes_a_marker_line_above_it() {
        let before = "/* legacy path */\nint main(void) { return 0; }\n";
        let after = mark_removable(before, "a.c");
        assert_eq!(after, "// ~keep\n/* legacy path */\nint main(void) { return 0; }\n");
        assert_eq!(removable_count(&after, "a.c"), 0);
    }

    #[test]
    fn the_marker_is_space_delimited_so_the_guards_read_it_as_a_marker() {
        // `is_marker_occurrence` rejects `~keepsake` and backticked prose; a plain space-delimited
        // append is what it accepts.
        let after = mark_removable("// note\n", "a.rs");
        assert!(after.contains(" ~keep"), "{after}");
        assert!(!after.contains("~keepsake"), "{after}");
        assert!(!after.contains('`'), "{after}");
    }

    #[test]
    fn marker_lines_use_a_plain_comment_token_and_never_a_doc_prefix() {
        for marker in ["// ~keep", "# ~keep", "-- ~keep", "/* ~keep */", "<!-- ~keep -->"] {
            assert!(!reads_as_documentation(marker), "{marker}");
        }
        for marker in ["/// ~keep", "//! ~keep", "/** ~keep */", "## ~keep"] {
            assert!(reads_as_documentation(marker), "{marker}");
        }
    }

    #[test]
    fn several_comments_in_one_file_are_marked_in_a_single_pass() {
        let before = "// one\nfn a() {}\n// two\nfn b() {}\n/* three */\nfn c() {}\n";
        let (after, outcome, _) = mark_with(before, "a.rs", false, |comment| {
            matches!(comment.verdict, Verdict::Remove { .. })
        });
        assert_eq!(outcome.placements.len(), 3);
        assert_eq!(
            after,
            "// one ~keep\nfn a() {}\n// two ~keep\nfn b() {}\n// ~keep\n/* three */\nfn c() {}\n"
        );
        assert_eq!(removable_count(&after, "a.rs"), 0);
    }

    #[test]
    fn marking_is_idempotent() {
        let before = "// one\nfn a() {}\n/* two */\nfn b() {}\n";
        let once = mark_removable(before, "a.rs");
        let twice = mark_removable(&once, "a.rs");
        assert_eq!(once, twice);
    }

    #[test]
    fn a_comment_already_protected_by_a_marker_is_left_alone() {
        let before = "// one ~keep\nfn a() {}\n";
        let (after, outcome, _) = mark_with(before, "a.rs", false, |_| true);
        assert_eq!(after, before);
        assert_eq!(outcome.placements.len(), 0);
        assert_eq!(outcome.already_marked.len(), 1);
    }

    #[test]
    fn a_comment_on_the_first_line_of_the_file_gets_an_adjacent_marker() {
        let after = mark_removable("/* first */\nint x;\n", "a.c");
        assert_eq!(after, "// ~keep\n/* first */\nint x;\n");
        assert_eq!(removable_count(&after, "a.c"), 0);
    }

    #[test]
    fn a_blank_line_above_the_comment_stays_a_blank_line() {
        let before = "int x;\n\n/* later */\nint y;\n";
        let after = mark_removable(before, "a.c");
        assert_eq!(after, "int x;\n\n// ~keep\n/* later */\nint y;\n");
        assert_eq!(removable_count(&after, "a.c"), 0);
    }

    #[test]
    fn an_indented_comment_keeps_its_indentation() {
        let before = "fn f() {\n    /* inner */\n}\n";
        let after = mark_removable(before, "a.rs");
        assert_eq!(after, "fn f() {\n    // ~keep\n    /* inner */\n}\n");
    }

    #[test]
    fn a_crlf_file_stays_crlf() {
        let before = "// one\r\nfn a() {}\r\n/* two */\r\nfn b() {}\r\n";
        let after = mark_removable(before, "a.rs");
        assert_eq!(
            after,
            "// one ~keep\r\nfn a() {}\r\n// ~keep\r\n/* two */\r\nfn b() {}\r\n"
        );
        assert!(!after.contains("\n\n"), "a bare LF crept in: {after:?}");
        assert_eq!(removable_count(&after, "a.rs"), 0);
    }

    #[test]
    fn a_file_without_a_trailing_newline_does_not_gain_one() {
        let before = "// one";
        assert_eq!(mark_removable(before, "a.rs"), "// one ~keep");
    }

    /// CSS and HTML have no line-comment form at all, so the marker line has to be written with the
    /// block pair. Refusing them instead would leave every comment in those languages unprotectable.
    #[test]
    fn a_language_with_only_a_block_pair_gets_a_block_marker_line() {
        for (file, before, expected) in [
            (
                "a.css",
                "/* legacy rule */\na { color: red; }\n",
                "/* ~keep */\n/* legacy rule */\na { color: red; }\n",
            ),
            (
                "a.html",
                "<!-- legacy markup -->\n<p>x</p>\n",
                "<!-- ~keep -->\n<!-- legacy markup -->\n<p>x</p>\n",
            ),
        ] {
            let after = mark_removable(before, file);
            assert_eq!(after, expected, "{file}");
            assert_eq!(removable_count(&after, file), 0, "{file}");
        }
    }

    #[test]
    fn a_language_with_neither_comment_token_reports_the_comment_unmarkable() {
        let content = "/* only blocks here */\na { color: red; }\n";
        let (_, _, comments) = mark_with(content, "a.css", false, |_| false);
        let error =
            plan_marker(content, &comments[0], MarkerSyntax::default(), "\n").expect_err("no token can carry a marker");
        assert_eq!(error, Unmarkable::NoMarkerToken);
    }

    #[test]
    fn a_block_comment_sharing_its_line_with_code_is_reported_rather_than_guessed_at() {
        let before = "int x = 1; /* why */\n";
        let (after, outcome, _) = mark_with(before, "a.c", false, |comment| {
            matches!(comment.verdict, Verdict::Remove { .. })
        });
        assert_eq!(after, before);
        assert_eq!(outcome.unmarkable[0].1, Unmarkable::NotStandalone);
    }

    #[test]
    fn a_shebang_is_never_extended() {
        let before = "#!/usr/bin/env python\nx = 1\n";
        let (after, outcome, _) = mark_with(before, "a.py", false, |_| true);
        assert_eq!(after, before);
        assert!(
            outcome
                .unmarkable
                .iter()
                .any(|&(_, reason)| reason == Unmarkable::Directive),
            "{:?}",
            outcome.unmarkable
        );
    }

    #[test]
    fn one_marker_covers_a_run_of_adjacent_doc_lines() {
        let before = "/// One.\n/// Two.\npub fn f() {}\n";
        let (after, outcome, _) = mark_with(before, "a.rs", true, |comment| comment.kind == CommentKind::Doc);
        assert_eq!(outcome.placements.len(), 1, "{after}");
        assert_eq!(after, "// ~keep\n/// One.\n/// Two.\npub fn f() {}\n");
    }

    #[test]
    fn a_trailing_comment_does_not_cover_the_comment_beneath_it() {
        // `is_keep_marker_line` requires a *standalone* comment, so ` ~keep` appended to a trailing
        // comment protects only that comment and reaches nothing below it. The doc comment beneath
        // must still get a marker line of its own — eliding it leaves it removable.
        let before = "fn g() { let _x = 1; } // shim\n/// Public API.\npub fn f() {}\n";
        let (after, outcome, _) = mark_with(before, "a.rs", true, |comment| {
            matches!(comment.verdict, Verdict::Remove { .. })
        });
        assert_eq!(
            after,
            "fn g() { let _x = 1; } // shim ~keep\n// ~keep\n/// Public API.\npub fn f() {}\n"
        );
        assert!(outcome.unmarkable.is_empty(), "{:?}", outcome.unmarkable);
        assert_eq!(removable_count_with(&after, "a.rs", true), 0);
    }

    #[test]
    fn the_line_ending_is_taken_from_the_file() {
        assert_eq!(dominant_line_ending("a\r\nb\r\n"), "\r\n");
        assert_eq!(dominant_line_ending("a\nb\n"), "\n");
        assert_eq!(dominant_line_ending(""), "\n");
        assert_eq!(dominant_line_ending("a\nb\r\n"), "\n");
    }

    #[test]
    fn an_appended_markers_prior_id_still_resolves_to_the_marked_comment() {
        let before = "// one\nfn a() {}\n";
        let path = PathBuf::from("a.rs");
        let mut processor = Processor::new();

        let original = processor.inspect(before, &path, &default_config()).expect("inspect");
        let original_ids = comment_ids("a.rs", &original, syntax_for("a.rs"));

        let after = mark_removable(before, "a.rs");
        let marked = processor.inspect(&after, &path, &default_config()).expect("inspect");
        let marked_ids = comment_ids("a.rs", &marked, syntax_for("a.rs"));

        assert!(
            marked_ids[0].contains(&original_ids[0][0]),
            "{:?} does not carry {:?}",
            marked_ids[0],
            original_ids[0][0]
        );
    }

    #[test]
    fn a_marker_line_this_module_wrote_is_not_treated_as_a_pre_existing_comment() {
        let path = PathBuf::from("a.rs");
        let mut processor = Processor::new();
        let content = "// ~keep\n/* body */\nfn a() {}\n";
        let comments = processor.inspect(content, &path, &default_config()).expect("inspect");
        let marker = comments
            .iter()
            .find(|comment| comment.text.trim() == "// ~keep")
            .expect("marker line");
        assert!(is_emitted_marker_line(marker, syntax_for("a.rs")));
        assert!(!is_emitted_marker_line(&comments[1], syntax_for("a.rs")));
    }

    /// The same invariant for a language whose marker line is itself a block comment, and it is not
    /// cosmetic. The pre-marker view strips ` ~keep` back out of each comment it keeps, so a
    /// `/* ~keep */` marker line mistaken for a pre-existing comment is reconstructed as `/* */` and
    /// claims the scan's id for the file's real empty comment. Two comments answering to one id is
    /// what [`IdIndex`] reports as ambiguous, and `keep` then refuses to mark either of them.
    #[test]
    fn a_block_form_marker_line_is_not_treated_as_a_pre_existing_comment() {
        let path = PathBuf::from("a.css");
        let syntax = syntax_for("a.css");
        let mut processor = Processor::new();

        // The empty comment is the collision: `/* */` is exactly what ` ~keep` stripped out of a
        // `/* ~keep */` marker line leaves behind.
        let before = "/* legacy rule */\na { color: red; }\n/* */\nb { color: blue; }\n";
        let original = processor.inspect(before, &path, &default_config()).expect("inspect");
        let original_ids = comment_ids("a.css", &original, syntax);

        let after = mark_removable(before, "a.css");
        let marked = processor.inspect(&after, &path, &default_config()).expect("inspect");
        let marked_ids = comment_ids("a.css", &marked, syntax);

        let marker = marked
            .iter()
            .find(|comment| comment.text.trim() == "/* ~keep */")
            .expect("marker line");
        assert!(is_emitted_marker_line(marker, syntax));

        let empty = |comments: &[InspectedComment]| {
            comments
                .iter()
                .position(|comment| comment.text.trim() == "/* */")
                .expect("empty comment")
        };
        let (was, is) = (empty(&original), empty(&marked));
        let scanned = &original_ids[was][0];
        assert!(
            marked_ids[is].contains(scanned),
            "{:?} does not carry {scanned:?}",
            marked_ids[is]
        );
        let owners = marked_ids.iter().filter(|ids| ids.contains(scanned)).count();
        assert_eq!(owners, 1, "{scanned} is claimed by {owners} comments: {marked_ids:?}");
    }

    #[test]
    fn ids_are_read_out_of_both_jsonl_and_json() {
        let dir = tempfile::tempdir().expect("tempdir");
        let jsonl = dir.path().join("scan.jsonl");
        std::fs::write(
            &jsonl,
            "{\"id\":\"aaaaaaaaaa\",\"path\":\"a.rs\"}\n\n{\"id\":\"bbbbbbbbbb\"}\n",
        )
        .expect("write");
        assert_eq!(
            ids_from_decisions(&jsonl).expect("read"),
            vec!["aaaaaaaaaa".to_string(), "bbbbbbbbbb".to_string()]
        );

        let json = dir.path().join("scan.json");
        std::fs::write(&json, "[{\"id\":\"cccccccccc\",\"line\":3}]").expect("write");
        assert_eq!(ids_from_decisions(&json).expect("read"), vec!["cccccccccc".to_string()]);
    }
}
