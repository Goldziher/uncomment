//! `uncomment scan`: a machine-readable inventory of every comment in a tree.
//!
//! The inventory is read-only — no source file is opened for writing — and it is produced by the
//! same engine a real run uses ([`Processor::inspect`], of which the removal planner is a filter),
//! so a `remove` verdict here is a comment the equivalent `uncomment` invocation would strip. The
//! comment-selection flags are shared with that run for the same reason: `--remove-doc`,
//! `--ignore`, `-c` and the rest change verdicts exactly as they change behaviour.
//!
//! # Output
//!
//! `--format jsonl` (the default) emits one object per line:
//!
//! ```json
//! {"id":"a3f9c1d2ab","path":"dev_tools/acli/cli.py","line":42,"end_line":42,
//!  "start_byte":1180,"end_byte":1203,"kind":"line","verdict":"remove","text":"# legacy shim"}
//! ```
//!
//! `line` and `end_line` are 1-based; `start_byte`/`end_byte` are 0-based offsets into the file as
//! it was read. `path` is repo-relative with `/` separators (falling back to a path relative to the
//! working directory outside a repository), so a decisions file is portable across checkouts.
//!
//! A preserved record carries why it survived. [`PreserveReason`] is encoded externally tagged with
//! snake-case names: every reason but one is a bare string — `"reason":"documentation"`,
//! `"file_header"`, `"shebang"`, `"keep_marker"`, `"extended_by_neighbour_marker"`,
//! `"language_directive"` — and the pattern variant carries its pattern as
//! `"reason":{"pattern":"TODO"}`.
//!
//! `--format json` wraps the same objects in an array, `--format text` prints
//! `path:line  [verdict]  text`.
//!
//! # Identifiers
//!
//! Ids come from [`super::id`], which deliberately hashes path, exact bytes and occurrence index
//! and not location, so an id survives edits above the comment. Truncation to 10 hex characters is
//! fallible at scale, so collisions are resolved rather than ignored:
//!
//! - Within one file, if two comments share a short id, *every* record carrying it is widened to
//!   the 32-hex [`full_id`]. `keep` re-derives ids per file, so an intra-file collision is the one
//!   that could make it mark the wrong comment.
//! - Across files, the first claimant keeps the short id and every later one is widened. Each id in
//!   a report is therefore unique, which is what a consumer needs, without buffering the whole tree
//!   to discover it. Both cases are noted on stderr.
//!
//! # Scale
//!
//! Files are parsed in parallel in bounded batches and records are written as each batch completes,
//! so `jsonl`, `json` and `text` hold one batch of comments at a time rather than the whole tree.
//! `--group-identical` is the exception: collapsing byte-different but equivalent comments onto one
//! decision is inherently whole-tree, and accumulates one entry per distinct normalized text. The
//! one thing every format retains is the set of ids already emitted, at ten bytes a comment, which
//! is what makes the cross-file collision check possible without holding the comments themselves.

use std::collections::{BTreeMap, HashSet};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use rayon::prelude::*;
use serde::Serialize;

use crate::config::{Config, ConfigManager, ExcludeSet, ResolvedConfig};
use crate::languages::LanguageRegistry;
use crate::languages::registry::warn_languages_without_a_grammar;
use crate::paths::{absolute_normalized, find_repo_root, repo_relative, to_slash};
use crate::processor::{CommentKind, PreserveReason, ProcessingOptions, Processor, Verdict};
use crate::ui;

use super::id::{ambiguous_ids, assign_occurrence_indices, comment_id, full_id, group_id, normalize_comment_text};

/// Sites listed per group before the list is capped and marked `truncated`.
const SITE_CAP: usize = 50;

/// Files parsed per batch. Bounds how many comments are held in memory at once while keeping
/// batches large enough that the thread pool stays busy.
const BATCH_FILES: usize = 256;

#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum ScanFormat {
    /// One JSON object per line.
    Jsonl,
    /// A single JSON array.
    Json,
    /// `path:line  [verdict]  text`, for humans.
    Text,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum ScanSelection {
    /// Only comments a real run would strip.
    Removable,
    /// Only comments a real run would keep.
    Preserved,
    All,
}

#[derive(clap::Args, Debug)]
pub struct ScanArgs {
    #[command(flatten)]
    pub process: crate::cli::ProcessArgs,

    /// Output format
    #[arg(
        long,
        value_name = "FORMAT",
        value_enum,
        default_value_t = ScanFormat::Jsonl,
        help = "Report format: jsonl (default), json, or text",
        help_heading = "Inventory"
    )]
    pub format: ScanFormat,

    /// Which verdicts to report
    #[arg(
        long,
        value_name = "WHICH",
        value_enum,
        default_value_t = ScanSelection::All,
        help = "Report only removable comments, only preserved ones, or all",
        help_heading = "Inventory"
    )]
    pub only: ScanSelection,

    /// Collapse comments whose normalized text is identical
    #[arg(
        long = "group-identical",
        help = "Collapse equivalent comments into one record with a site list",
        help_heading = "Inventory"
    )]
    pub group_identical: bool,

    /// Write the report to a file instead of stdout
    #[arg(
        short = 'o',
        long = "output",
        value_name = "FILE",
        help = "Write the report to FILE instead of stdout",
        help_heading = "Inventory"
    )]
    pub output: Option<PathBuf>,
}

/// Produce the inventory for `args`, writing it to stdout or to `--output`.
///
/// # Errors
///
/// Fails when no paths were given, when configuration cannot be loaded, when the thread pool
/// cannot be built, or when the report cannot be written. A file that cannot be read, decoded or
/// parsed is reported on stderr and skipped, exactly as in a real run.
pub fn run(args: &ScanArgs) -> Result<()> {
    if args.process.paths.is_empty() {
        bail!("No input paths specified. Pass a file, directory or glob to scan.");
    }

    let options = args.process.processing_options();
    let current_dir = std::env::current_dir().context("Failed to get current directory")?;
    let mut config_manager = build_config_manager(args, &current_dir)?;

    // One config-aware registry for both collection and inspection, matching a real run: the
    // extensions a `[languages]` section declares decide what is collected, not only how it is read.
    config_manager.discover_language_sources(&args.process.paths, options.respect_gitignore);
    let mut registry = LanguageRegistry::new();
    warn_languages_without_a_grammar(&registry.register_configured_languages(&config_manager.get_all_languages()));

    let excludes = config_manager.exclude_set(&args.process.exclude)?;
    let files = collect_files(&args.process.paths, &options, &registry, &excludes)?;

    let num_threads = if args.process.threads == 0 {
        num_cpus::get()
    } else {
        args.process.threads
    };
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build_global()
        .context("Failed to initialize thread pool")?;

    let repo_root = find_repo_root(&current_dir);
    let context = ScanContext {
        config_manager: &config_manager,
        registry: &registry,
        options: &options,
        current_dir: &current_dir,
        repo_root: repo_root.as_deref(),
        selection: args.only,
        grouping: args.group_identical,
    };

    let writer: Box<dyn Write> = match &args.output {
        Some(path) => {
            Box::new(BufWriter::new(File::create(path).with_context(|| {
                format!("Failed to create report file: {}", path.display())
            })?))
        }
        None => Box::new(BufWriter::new(std::io::stdout())),
    };
    let mut emitter = Emitter::new(writer, args.format);

    let mut totals = Totals::default();
    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut groups: BTreeMap<String, Group> = BTreeMap::new();

    for batch in files.chunks(BATCH_FILES) {
        let scanned: Vec<FileScan> = if num_threads == 1 {
            batch.iter().map(|file| context.scan_file(file)).collect()
        } else {
            batch.par_iter().map(|file| context.scan_file(file)).collect()
        };

        for mut scan in scanned {
            if let Some(message) = scan.skipped {
                totals.skipped += 1;
                anstream::eprintln!("{} skipping {}: {message}", ui::warn("warning:"), ui::path(&scan.file));
                continue;
            }
            totals.files += 1;

            for widened in resolve_collisions(&mut scan.records, &mut seen_ids) {
                anstream::eprintln!(
                    "{} id {} is claimed by more than one comment; {}:{} is reported under its full identifier",
                    ui::warn("warning:"),
                    ui::accent(&widened.short),
                    widened.path,
                    widened.line
                );
            }

            for record in scan.records {
                totals.count(record.verdict);
                if context.grouping {
                    accumulate(&mut groups, record);
                } else {
                    emitter.emit(&record, || record.text_line())?;
                }
            }
        }
    }

    if context.grouping {
        emit_groups(&mut emitter, groups, &mut totals)?;
    }

    emitter.finish()?;

    if !args.process.quiet {
        report_totals(&totals, context.grouping);
    }

    Ok(())
}

/// Counts for the closing summary. Written to stderr so stdout carries only the report.
#[derive(Debug, Default)]
struct Totals {
    files: usize,
    skipped: usize,
    removable: usize,
    preserved: usize,
    groups: usize,
}

impl Totals {
    fn count(&mut self, verdict: RecordVerdict) {
        match verdict {
            RecordVerdict::Remove => self.removable += 1,
            RecordVerdict::Preserve => self.preserved += 1,
        }
    }
}

/// One record reported under its full identifier because another already claimed the short one.
#[derive(Debug, PartialEq, Eq)]
struct Widened {
    short: String,
    path: String,
    line: usize,
}

/// Leave every record in `records` under an identifier no other record in the report claims.
///
/// `seen` accumulates the short ids of the records already emitted. The first claimant of a short id
/// keeps it and each later one is widened, so a consumer can resolve any id in the report to exactly
/// one comment. A record already widened by the intra-file check is left alone and does not reserve
/// its short form, which no record is using.
fn resolve_collisions(records: &mut [Record], seen: &mut HashSet<String>) -> Vec<Widened> {
    let mut widened = Vec::new();

    for record in records {
        if record.id.len() == FULL_ID_HEX {
            continue;
        }
        if !seen.insert(record.id.clone()) {
            widened.push(Widened {
                short: std::mem::replace(&mut record.id, record.full_id.clone()),
                path: record.path.clone(),
                line: record.line,
            });
        }
    }

    widened
}

fn report_totals(totals: &Totals, grouping: bool) {
    let reported = totals.removable + totals.preserved;
    anstream::eprintln!(
        "{} {}",
        ui::dim(ui::BULLET),
        ui::dim(format!(
            "Scanned {} files, {reported} comments reported ({} removable, {} preserved){}{}",
            totals.files,
            totals.removable,
            totals.preserved,
            if grouping {
                format!(", {} groups", totals.groups)
            } else {
                String::new()
            },
            if totals.skipped > 0 {
                format!("; {} file(s) skipped", totals.skipped)
            } else {
                String::new()
            },
        ))
    );
}

/// Length of a [`full_id`] in hex characters, used to recognise an already-widened id.
const FULL_ID_HEX: usize = super::id::FULL_ID_BYTES * 2;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum RecordVerdict {
    Remove,
    Preserve,
}

impl RecordVerdict {
    fn as_str(self) -> &'static str {
        match self {
            RecordVerdict::Remove => "remove",
            RecordVerdict::Preserve => "preserve",
        }
    }
}

impl Serialize for RecordVerdict {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// Why a preserved comment survived, in the JSON encoding documented at the module level.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum Reason {
    Pattern(String),
    Documentation,
    FileHeader,
    Shebang,
    KeepMarker,
    ExtendedByNeighbourMarker,
    LanguageDirective,
}

impl From<&PreserveReason> for Reason {
    fn from(reason: &PreserveReason) -> Self {
        match reason {
            PreserveReason::Pattern(pattern) => Reason::Pattern(pattern.clone()),
            PreserveReason::Documentation => Reason::Documentation,
            PreserveReason::FileHeader => Reason::FileHeader,
            PreserveReason::Shebang => Reason::Shebang,
            PreserveReason::KeepMarker => Reason::KeepMarker,
            PreserveReason::ExtendedByNeighbourMarker => Reason::ExtendedByNeighbourMarker,
            PreserveReason::LanguageDirective => Reason::LanguageDirective,
        }
    }
}

/// One comment in the report.
#[derive(Debug, Serialize)]
struct Record {
    id: String,
    path: String,
    line: usize,
    end_line: usize,
    start_byte: usize,
    end_byte: usize,
    kind: &'static str,
    verdict: RecordVerdict,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<Reason>,

    /// The widened form, carried so a collision found later needs no re-derivation. Not part of the
    /// report.
    #[serde(skip)]
    full_id: String,
}

impl Record {
    fn text_line(&self) -> String {
        format!(
            "{}:{}  [{}]  {}",
            self.path,
            self.line,
            self.verdict.as_str(),
            single_line(&self.text)
        )
    }
}

/// The outcome of one file: either records, or the reason it was skipped.
struct FileScan {
    file: PathBuf,
    records: Vec<Record>,
    skipped: Option<String>,
}

struct ScanContext<'a> {
    config_manager: &'a ConfigManager,
    registry: &'a LanguageRegistry,
    options: &'a ProcessingOptions,
    current_dir: &'a Path,
    repo_root: Option<&'a Path>,
    selection: ScanSelection,
    grouping: bool,
}

impl ScanContext<'_> {
    fn scan_file(&self, file: &Path) -> FileScan {
        let skip = |message: String| FileScan {
            file: file.to_path_buf(),
            records: Vec::new(),
            skipped: Some(message),
        };

        let content = match std::fs::read_to_string(file) {
            Ok(content) => content,
            Err(error) => return skip(error.to_string()),
        };

        let language = self
            .registry
            .detect_language(file)
            .map(|language| language.name.to_lowercase());
        let config = self.resolved_config(file, language.as_deref());

        let mut processor = Processor::new_with_config(self.config_manager);
        let inspected = match processor.inspect(&content, file, &config) {
            Ok(inspected) => inspected,
            Err(error) => return skip(format!("{error:#}")),
        };

        let path = self.display_path(file);
        let texts: Vec<&str> = inspected.iter().map(|comment| comment.text.as_str()).collect();
        let occurrences = assign_occurrence_indices(&texts);
        let ids: Vec<String> = texts
            .iter()
            .zip(&occurrences)
            .map(|(text, &occurrence)| comment_id(&path, text, occurrence))
            .collect();
        // Every comment in the file participates, not just the reported ones: `keep` re-derives ids
        // over the whole file, so a short id shared with a filtered-out comment is still ambiguous.
        let ambiguous = ambiguous_ids(&ids);

        let mut records = Vec::new();
        for ((comment, id), occurrence) in inspected.iter().zip(ids).zip(occurrences) {
            let verdict = match comment.verdict {
                Verdict::Remove { .. } => RecordVerdict::Remove,
                Verdict::Preserve => RecordVerdict::Preserve,
            };
            if !self.selects(verdict) {
                continue;
            }

            let wide = full_id(&path, &comment.text, occurrence);
            let id = if ambiguous.contains(&id) { wide.clone() } else { id };
            records.push(Record {
                id,
                path: path.clone(),
                line: comment.start_row + 1,
                end_line: comment.end_row + 1,
                start_byte: comment.start_byte,
                end_byte: comment.end_byte,
                kind: kind_name(comment.kind),
                verdict,
                text: comment.text.clone(),
                reason: comment.reason.as_ref().map(Reason::from),
                full_id: wide,
            });
        }

        FileScan {
            file: file.to_path_buf(),
            records,
            skipped: None,
        }
    }

    fn selects(&self, verdict: RecordVerdict) -> bool {
        match self.selection {
            ScanSelection::All => true,
            ScanSelection::Removable => verdict == RecordVerdict::Remove,
            ScanSelection::Preserved => verdict == RecordVerdict::Preserve,
        }
    }

    /// Repo-relative with `/` separators, so an id and a site survive being carried to another
    /// checkout. Outside a repository the working directory stands in for the root; a file under
    /// neither is reported by its absolute path rather than silently losing components.
    fn display_path(&self, file: &Path) -> String {
        let absolute = absolute_normalized(self.current_dir, file);
        let relative = self
            .repo_root
            .and_then(|root| repo_relative(root, &absolute))
            .or_else(|| repo_relative(self.current_dir, &absolute));
        to_slash(relative.as_deref().unwrap_or(&absolute))
    }

    /// Resolve `file`'s configuration the way a real run does.
    ///
    /// [`Processor::inspect`] takes an already-resolved config and performs no discovery, so the
    /// CLI overrides are applied here instead. They are applied one-directionally, mirroring
    /// `Processor::process_file_with_config`: an unset flag must never clobber a config-file value.
    fn resolved_config(&self, file: &Path, language: Option<&str>) -> ResolvedConfig {
        let mut config = match language {
            Some(language) => self.config_manager.get_config_for_file_with_language(file, language),
            None => self.config_manager.get_config_for_file(file),
        };

        if self.options.remove_doc {
            config.remove_docs = true;
        }
        if !self.options.use_default_ignores {
            config.use_default_ignores = false;
        }
        if self.options.remove_todo {
            config.remove_todos = true;
        }
        if self.options.remove_fixme {
            config.remove_fixme = true;
        }
        if !self.options.custom_preserve_patterns.is_empty() {
            config
                .preserve_patterns
                .extend(self.options.custom_preserve_patterns.iter().cloned());
        }
        if !self.options.respect_gitignore {
            config.respect_gitignore = false;
        }
        if self.options.traverse_git_repos {
            config.traverse_git_repos = true;
        }

        config
    }
}

fn kind_name(kind: CommentKind) -> &'static str {
    match kind {
        CommentKind::Line => "line",
        CommentKind::Block => "block",
        CommentKind::Doc => "doc",
        CommentKind::Docstring => "docstring",
    }
}

/// One decision covering every site whose comment normalizes to the same text.
#[derive(Debug)]
struct Group {
    id: String,
    text: String,
    count: usize,
    removable: usize,
    preserved: usize,
    sites: Vec<String>,
    truncated: bool,
}

#[derive(Debug, Serialize)]
struct GroupRecord<'a> {
    id: &'a str,
    text: &'a str,
    count: usize,
    sites: &'a [String],
    #[serde(skip_serializing_if = "is_false")]
    truncated: bool,
    verdict: &'static str,
    /// Present only for a `mixed` group, where one decision cannot speak for every site.
    #[serde(skip_serializing_if = "Option::is_none")]
    removable: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    preserved: Option<usize>,
    /// Two different normalized texts produced the same group id; a consumer must not treat this id
    /// as naming one decision.
    #[serde(skip_serializing_if = "is_false")]
    ambiguous: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn accumulate(groups: &mut BTreeMap<String, Group>, record: Record) {
    let normalized = normalize_comment_text(&record.text);
    let group = groups.entry(normalized).or_insert_with_key(|normalized| Group {
        id: group_id(normalized),
        text: record.text.clone(),
        count: 0,
        removable: 0,
        preserved: 0,
        sites: Vec::new(),
        truncated: false,
    });

    group.count += 1;
    match record.verdict {
        RecordVerdict::Remove => group.removable += 1,
        RecordVerdict::Preserve => group.preserved += 1,
    }
    if group.sites.len() < SITE_CAP {
        group.sites.push(format!("{}:{}", record.path, record.line));
    } else {
        group.truncated = true;
    }
}

fn emit_groups(emitter: &mut Emitter, groups: BTreeMap<String, Group>, totals: &mut Totals) -> Result<()> {
    // Collected from a BTreeMap and then totally ordered, so the sequence is the same on every run:
    // the count and id decide, and the normalized text breaks a tie between two colliding ids.
    let mut ordered: Vec<(String, Group)> = groups.into_iter().collect();
    ordered.sort_by(|(left_key, left), (right_key, right)| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.id.cmp(&right.id))
            .then_with(|| left_key.cmp(right_key))
    });
    totals.groups = ordered.len();

    let ids: Vec<&str> = ordered.iter().map(|(_, group)| group.id.as_str()).collect();
    let ambiguous = ambiguous_ids(&ids);
    for id in &ambiguous {
        anstream::eprintln!(
            "{} group id {} covers more than one distinct comment text",
            ui::warn("warning:"),
            ui::accent(id)
        );
    }

    for (_, group) in &ordered {
        let mixed = group.removable > 0 && group.preserved > 0;
        let verdict = if mixed {
            "mixed"
        } else if group.removable > 0 {
            "remove"
        } else {
            "preserve"
        };
        let record = GroupRecord {
            id: &group.id,
            text: &group.text,
            count: group.count,
            sites: &group.sites,
            truncated: group.truncated,
            verdict,
            removable: mixed.then_some(group.removable),
            preserved: mixed.then_some(group.preserved),
            ambiguous: ambiguous.iter().any(|id| id == &group.id),
        };
        emitter.emit(&record, || {
            format!("{:>7}x  [{verdict}]  {}", group.count, single_line(&group.text))
        })?;
    }

    Ok(())
}

/// Writes records in the chosen format, one at a time.
struct Emitter {
    writer: Box<dyn Write>,
    format: ScanFormat,
    written: usize,
}

impl Emitter {
    fn new(writer: Box<dyn Write>, format: ScanFormat) -> Self {
        Self {
            writer,
            format,
            written: 0,
        }
    }

    fn emit(&mut self, value: &impl Serialize, text_line: impl FnOnce() -> String) -> Result<()> {
        match self.format {
            ScanFormat::Text => writeln!(self.writer, "{}", text_line())?,
            ScanFormat::Jsonl => {
                serde_json::to_writer(&mut self.writer, value).context("Failed to serialize record")?;
                writeln!(self.writer)?;
            }
            ScanFormat::Json => {
                if self.written == 0 {
                    writeln!(self.writer, "[")?;
                } else {
                    writeln!(self.writer, ",")?;
                }
                serde_json::to_writer(&mut self.writer, value).context("Failed to serialize record")?;
            }
        }
        self.written += 1;
        Ok(())
    }

    fn finish(&mut self) -> Result<()> {
        if self.format == ScanFormat::Json {
            if self.written == 0 {
                writeln!(self.writer, "[]")?;
            } else {
                writeln!(self.writer)?;
                writeln!(self.writer, "]")?;
            }
        }
        self.writer.flush().context("Failed to write the report")
    }
}

/// Collapse a comment onto one line for the human-facing format, which promises one record per
/// line and must not be broken by a block comment.
fn single_line(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for (index, line) in text.lines().enumerate() {
        if index > 0 {
            out.push(' ');
        }
        out.push_str(line.trim_end());
    }
    out
}

fn build_config_manager(args: &ScanArgs, current_dir: &Path) -> Result<ConfigManager> {
    match &args.process.config {
        Some(path) => {
            let config =
                Config::from_file(path).with_context(|| format!("Failed to load config file: {}", path.display()))?;
            ConfigManager::from_single_config(current_dir, config)
        }
        None => ConfigManager::new(current_dir).context("Failed to initialize configuration manager"),
    }
}

/// The supported files named by `paths`, sorted and deduplicated.
///
/// Mirrors the default run's collection — gitignore-aware directory walks, globs otherwise — so
/// the inventory covers exactly the files that run would have processed.
fn collect_files(
    paths: &[String],
    options: &ProcessingOptions,
    registry: &LanguageRegistry,
    excludes: &ExcludeSet,
) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for pattern in paths {
        let path = Path::new(pattern);

        if path.is_file() {
            if !excludes.is_excluded(path) && registry.detect_language(path).is_some() {
                files.push(path.to_path_buf());
            }
        } else if path.is_dir() {
            if excludes.prunes_dir(path) {
                continue;
            }
            collect_from_pattern(
                &format!("{}/**/*", path.display()),
                &mut files,
                options,
                registry,
                excludes,
            )?;
        } else {
            collect_from_pattern(pattern, &mut files, options, registry, excludes)?;
        }
    }

    files.sort();
    files.dedup();
    Ok(files)
}

fn collect_from_pattern(
    pattern: &str,
    files: &mut Vec<PathBuf>,
    options: &ProcessingOptions,
    registry: &LanguageRegistry,
    excludes: &ExcludeSet,
) -> Result<()> {
    if !options.respect_gitignore {
        for entry in glob::glob(pattern).context("Failed to parse glob pattern")? {
            match entry {
                Ok(path)
                    if path.is_file() && !excludes.is_excluded(&path) && registry.detect_language(&path).is_some() =>
                {
                    files.push(path);
                }
                Ok(_) => {}
                Err(error) => anstream::eprintln!("{} reading path: {error}", ui::danger("error")),
            }
        }
        return Ok(());
    }

    let root = pattern.strip_suffix("/**/*").unwrap_or(pattern);
    let absolute = absolute_normalized(
        &std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        Path::new(root),
    );

    // Walk from the git root when there is one, so ignore rules recorded above the requested
    // directory still apply, then keep only what lies under that directory.
    let git_root = absolute.ancestors().skip(1).find(|dir| dir.join(".git").exists());
    let (walk_root, prefix) = match git_root {
        Some(root) if absolute.starts_with(root) => (root.to_path_buf(), Some(absolute.clone())),
        _ => (absolute, None),
    };

    let walker = ignore::WalkBuilder::new(walk_root)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .parents(true)
        .require_git(false)
        .filter_entry(excludes.walk_filter())
        .build();

    for entry in walker {
        match entry {
            Ok(entry) => {
                let path = entry.path();
                if let Some(prefix) = &prefix
                    && !path.starts_with(prefix)
                {
                    continue;
                }
                if path.is_file() && !excludes.is_excluded(path) && registry.detect_language(path).is_some() {
                    files.push(path.to_path_buf());
                }
            }
            Err(error) => anstream::eprintln!("{} reading path: {error}", ui::danger("error")),
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A record with the fields the unit under test reads, and defaults elsewhere.
    fn record(id: &str, path: &str, line: usize, text: &str, verdict: RecordVerdict) -> Record {
        Record {
            id: id.to_string(),
            path: path.to_string(),
            line,
            end_line: line,
            start_byte: 0,
            end_byte: 0,
            kind: "line",
            verdict,
            text: text.to_string(),
            reason: None,
            full_id: format!("{id}{}", "f".repeat(FULL_ID_HEX - id.len())),
        }
    }

    #[test]
    fn the_first_claimant_of_an_id_keeps_it_and_later_ones_are_widened() {
        let mut seen = HashSet::new();
        let mut first = [record("a3f9c1d2ab", "src/a.rs", 4, "// x", RecordVerdict::Remove)];
        assert!(resolve_collisions(&mut first, &mut seen).is_empty());
        assert_eq!(first[0].id, "a3f9c1d2ab");

        // The same short id turning up in another file: that record moves to its full form, so the
        // short id still names exactly one comment in the report.
        let mut later = [
            record("a3f9c1d2ab", "src/b.rs", 9, "// y", RecordVerdict::Remove),
            record("0000000001", "src/b.rs", 11, "// z", RecordVerdict::Preserve),
        ];
        let widened = resolve_collisions(&mut later, &mut seen);

        assert_eq!(
            widened,
            vec![Widened {
                short: "a3f9c1d2ab".to_string(),
                path: "src/b.rs".to_string(),
                line: 9,
            }]
        );
        assert_eq!(later[0].id.len(), FULL_ID_HEX);
        assert!(later[0].id.starts_with("a3f9c1d2ab"));
        assert_eq!(later[1].id, "0000000001");
    }

    #[test]
    fn an_already_widened_record_is_left_alone() {
        let mut seen = HashSet::new();
        let wide = "a".repeat(FULL_ID_HEX);
        let mut records = [
            record(&wide, "src/a.rs", 1, "// x", RecordVerdict::Remove),
            record(&wide, "src/b.rs", 1, "// y", RecordVerdict::Remove),
        ];

        assert!(resolve_collisions(&mut records, &mut seen).is_empty());
        assert!(records.iter().all(|record| record.id == wide));
        assert!(seen.is_empty(), "a full id must not reserve a short one");
    }

    #[test]
    fn a_preserve_reason_serializes_as_a_string_except_for_a_pattern() {
        let cases = [
            (PreserveReason::Documentation, "\"documentation\""),
            (PreserveReason::FileHeader, "\"file_header\""),
            (PreserveReason::Shebang, "\"shebang\""),
            (PreserveReason::KeepMarker, "\"keep_marker\""),
            (
                PreserveReason::ExtendedByNeighbourMarker,
                "\"extended_by_neighbour_marker\"",
            ),
            (PreserveReason::LanguageDirective, "\"language_directive\""),
            (PreserveReason::Pattern("TODO".to_string()), "{\"pattern\":\"TODO\"}"),
        ];

        for (reason, expected) in cases {
            let encoded = serde_json::to_string(&Reason::from(&reason)).expect("reason serializes");
            assert_eq!(encoded, expected, "encoding {reason:?}");
        }
    }

    #[test]
    fn a_block_comment_stays_on_one_line_in_the_text_format() {
        assert_eq!(single_line("/*\n * one\n * two\n */"), "/*  * one  * two  */");
        assert_eq!(single_line("// only"), "// only");
    }

    #[test]
    fn grouping_counts_every_site_but_caps_the_list() {
        let mut groups = BTreeMap::new();
        for index in 0..SITE_CAP + 5 {
            let verdict = if index == 0 {
                RecordVerdict::Preserve
            } else {
                RecordVerdict::Remove
            };
            accumulate(
                &mut groups,
                record(
                    &format!("{index:010x}"),
                    &format!("src/file{index}.rs"),
                    index + 1,
                    "//   Legacy  Shim",
                    verdict,
                ),
            );
        }

        assert_eq!(groups.len(), 1);
        let group = groups.values().next().expect("one group");
        assert_eq!(group.count, SITE_CAP + 5);
        assert_eq!(group.sites.len(), SITE_CAP);
        assert!(group.truncated);
        assert_eq!(group.removable, SITE_CAP + 4);
        assert_eq!(group.preserved, 1);
        assert_eq!(group.id, group_id(&normalize_comment_text("// legacy shim")));
    }

    #[test]
    fn differently_written_but_equivalent_comments_land_in_one_group() {
        let mut groups = BTreeMap::new();
        for text in ["// Legacy shim", "//   legacy   SHIM", "/* legacy shim */"] {
            accumulate(
                &mut groups,
                record("0000000000", "src/a.rs", 1, text, RecordVerdict::Remove),
            );
        }

        assert_eq!(groups.len(), 1);
        assert_eq!(groups.values().next().map(|group| group.count), Some(3));
    }
}
