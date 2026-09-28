//! `uncomment lint` — comment linting that never removes anything.
//!
//! The command this crate is named for deletes comments. This one does the opposite: it reads every
//! comment, checks that the ones claiming to track work actually name an issue, and reports. Without
//! `--fix` it writes nothing at all, which is what lets it run as a pre-commit hook and in CI beside
//! the removal hook rather than instead of it.
//!
//! Adoption on a large codebase is a first-class concern, because a tool that reports a thousand
//! pre-existing violations on day one gets disabled on day one. `--changed-only` narrows the run to
//! what a branch touched, and `--baseline` records what is already there — keyed by
//! [`crate::scan::id`], which excludes line and byte offsets precisely so that an unrelated edit
//! above a violating comment does not resurrect it.

pub mod config;
pub mod rules;

use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use rayon::prelude::*;

use crate::changes::{ChangeScope, ChangeScopeArgs};
use crate::config::{ConfigManager, ExcludeSet};
use crate::edit::{Edit, apply_edits};
use crate::languages::registry::{LanguageRegistry, warn_languages_without_a_grammar};
use crate::paths::{absolute_normalized, find_repo_root, repo_relative, to_slash};
use crate::processor::Processor;
use crate::scan::id::{assign_occurrence_indices, comment_id};
use crate::ui;

use self::config::{LintConfig, Rule, Severity};

/// Version stamped into a baseline file, so a later change to the id scheme can be refused rather
/// than silently mismatched.
const BASELINE_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
}

#[derive(clap::Args, Debug)]
pub struct LintArgs {
    #[command(flatten)]
    pub process: crate::cli::ProcessArgs,

    /// Rewrite what can be rewritten; without this nothing is written at all
    #[arg(long, help = "Apply the fixes that can be applied safely", help_heading = "Linting")]
    pub fix: bool,

    #[command(flatten)]
    pub scope: ChangeScopeArgs,

    /// Issue key used to fix comments that carry none
    #[arg(
        long = "todo-key",
        value_name = "KEY",
        help = "Issue key to insert into tag comments that have none (with --fix)",
        help_heading = "Linting"
    )]
    pub todo_key: Option<String>,

    /// Violations recorded here are reported but do not fail the run
    #[arg(
        long,
        value_name = "FILE",
        help = "Treat violations recorded in FILE as informational",
        help_heading = "Linting"
    )]
    pub baseline: Option<PathBuf>,

    /// Record every current violation in the baseline file and exit successfully
    #[arg(
        long = "write-baseline",
        requires = "baseline",
        help = "Write the current violations to --baseline and exit 0",
        help_heading = "Linting"
    )]
    pub write_baseline: bool,

    #[arg(
        long,
        value_enum,
        default_value = "text",
        value_name = "FORMAT",
        help = "Output format",
        help_heading = "Linting"
    )]
    pub format: OutputFormat,
}

/// One reported rule violation, resolved to a line and column.
#[derive(Debug, Clone)]
pub struct Violation {
    /// As it should be printed: relative to the invocation directory where possible.
    pub path: PathBuf,
    /// Stable comment id, repo-relative — the baseline key.
    pub id: String,
    pub rule: Rule,
    pub severity: Severity,
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub excerpt: String,
    pub tag: String,
    pub key: Option<String>,
    pub fixed: bool,
    pub baselined: bool,
}

impl Violation {
    /// Whether this violation should fail the run.
    fn is_failing(&self) -> bool {
        self.severity == Severity::Error && !self.baselined && !self.fixed
    }
}

#[derive(Debug, Default, Clone)]
pub struct Summary {
    pub files_linted: usize,
    pub files_skipped_disabled: usize,
    pub fixed: usize,
    pub baselined: usize,
    pub errors: usize,
    pub warnings: usize,
    /// Files that matched but could not be inspected at all, so nothing about them was checked.
    pub uninspectable: usize,
}

/// Everything one lint run concluded, before any of it is printed.
///
/// Separating this from the reporting is what lets a test assert on rules, lines and exit codes
/// without capturing the process's stdout.
#[derive(Debug)]
pub struct Outcome {
    pub violations: Vec<Violation>,
    /// Anything worth saying that is not a violation: a skipped rule, an unreadable file.
    pub notes: Vec<String>,
    pub summary: Summary,
}

impl Outcome {
    /// 1 when anything failed the run, 0 otherwise.
    pub fn exit_code(&self) -> i32 {
        i32::from(self.failing() > 0 || self.summary.uninspectable > 0)
    }

    pub fn failing(&self) -> usize {
        self.violations.iter().filter(|v| v.is_failing()).count()
    }

    pub fn by_rule(&self, rule: Rule) -> Vec<&Violation> {
        self.violations.iter().filter(|v| v.rule == rule).collect()
    }
}

/// Run the linter. Returns the process exit code: 1 when something failed the run, 0 otherwise.
///
/// Returning the code rather than calling `std::process::exit` keeps every `Drop` running and lets a
/// test call this directly.
pub fn run(args: &LintArgs) -> Result<i32> {
    let cwd = std::env::current_dir().context("failed to read the current directory")?;
    run_in(&cwd, args)
}

/// [`run`], with the directory every relative path, the config search and the branch lookup resolve
/// against passed in explicitly.
///
/// `run` supplies the process's working directory. A test supplies a fixture root, which is what lets
/// several of them run in the same process: the working directory is process-global state, and tests
/// that each `chdir` into their own fixture corrupt one another.
pub fn run_in(base: &Path, args: &LintArgs) -> Result<i32> {
    let outcome = lint(base, args)?;

    if let (Some(path), true) = (&args.baseline, args.write_baseline) {
        write_baseline(path, &outcome.violations)?;
        if args.format == OutputFormat::Text {
            anstream::println!(
                "{} recorded {} violation(s) in {}",
                ui::success(ui::CHECK),
                ui::accent(outcome.violations.len()),
                ui::path(path)
            );
        }
        return Ok(0);
    }

    report(&outcome, args.format)?;
    Ok(outcome.exit_code())
}

/// Lint without printing anything.
///
/// `base` is the directory relative paths, config discovery, the comment-id anchor and the branch
/// lookup all resolve against.
pub fn lint(base: &Path, args: &LintArgs) -> Result<Outcome> {
    let base = absolute_normalized(&std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")), base);
    let repo_root = find_repo_root(&base);
    let mut notes: Vec<String> = Vec::new();

    if args.process.paths.is_empty() {
        bail!("no input paths specified: pass one or more files or directories to lint");
    }

    // Configuration has to be loaded before collection, not after: a `[languages]` section can
    // declare the extension a file is recognised by, and a registry that has not seen it drops the
    // file before `[lint]` is ever consulted.
    let respect_gitignore = !args.process.no_gitignore;
    let mut config_manager = build_config_manager(&base, args.process.config.as_deref())?;
    config_manager.discover_language_sources(&args.process.paths, respect_gitignore);

    let mut registry = LanguageRegistry::new();
    warn_languages_without_a_grammar(&registry.register_configured_languages(&config_manager.get_all_languages()));

    let excludes = config_manager.exclude_set(&args.process.exclude)?;
    let mut files = collect_files(&base, &args.process.paths, respect_gitignore, &registry, &excludes)?;

    let scope = ChangeScope::resolve(&args.scope, repo_root.as_deref())?;
    if let Some(scope) = &scope {
        let before = files.len();
        files.retain(|file| scope.contains_file(&absolute_normalized(&base, file)));
        notes.push(scope.note(files.len(), before));
    }

    // Resolving `[lint]` up front means a bad pattern is reported before a single file is inspected,
    // and certainly before `--fix` has rewritten anything.
    let mut resolver = config::Resolver::new(&base, &config_manager);
    let mut targets: Vec<(PathBuf, Arc<LintConfig>)> = Vec::with_capacity(files.len());
    let mut disabled = 0usize;
    for file in files {
        let config = resolver.for_file(&file)?;
        if config.enabled {
            targets.push((file, config));
        } else {
            disabled += 1;
        }
    }

    // A config file below the invocation directory is discovered during that pre-pass, from a call
    // that cannot return an error, so a rejected one is recorded instead. `--fix` rewrites files, so
    // refuse the run rather than lint under defaults the user never asked for.
    if let Some(error) = config_manager.deferred_config_error() {
        bail!("{error}");
    }

    let mut summary = Summary {
        files_linted: targets.len(),
        files_skipped_disabled: disabled,
        ..Summary::default()
    };

    if targets.is_empty() {
        if disabled > 0 {
            notes.push(format!(
                "lint is not enabled for {disabled} matched file(s): set `enabled = true` under `[lint]` in \
                 .uncomment.toml"
            ));
        }
        return Ok(Outcome {
            violations: Vec::new(),
            notes,
            summary,
        });
    }

    let current_issue = resolve_current_issue(repo_root.as_deref(), &targets, &mut notes);

    let baseline = match (&args.baseline, args.write_baseline) {
        (Some(path), false) => load_baseline(path)?,
        _ => HashSet::new(),
    };

    let context = FileContext {
        repo_root: repo_root.clone(),
        base: base.clone(),
        current_issue,
        todo_key: args.todo_key.clone(),
        // `--write-baseline` records what is there; rewriting it in the same run would record
        // violations that no longer exist.
        fix: args.fix && !args.write_baseline,
        baseline,
    };

    let threads = if args.process.threads == 0 {
        num_cpus::get()
    } else {
        args.process.threads
    };

    let lint_one = |(path, config): &(PathBuf, Arc<LintConfig>)| lint_file(path, config, &config_manager, &context);

    let outcomes: Vec<FileOutcome> = if threads <= 1 {
        targets.iter().map(lint_one).collect()
    } else {
        // A pool of this run's own rather than rayon's global one, which the removal command
        // installs once per process and which a library caller may already own.
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .context("failed to build the lint thread pool")?;
        pool.install(|| targets.par_iter().map(lint_one).collect())
    };

    let mut violations = Vec::new();
    for outcome in outcomes {
        match outcome {
            FileOutcome::Linted { violations: found } => violations.extend(found),
            FileOutcome::Skipped { note } => {
                summary.files_linted -= 1;
                notes.push(note);
            }
            FileOutcome::Failed { path, error } => {
                summary.files_linted -= 1;
                summary.uninspectable += 1;
                notes.push(format!("{}: {error}", to_slash(&path)));
            }
        }
    }

    if let Some(scope) = scope.as_ref().filter(|scope| scope.is_per_line()) {
        violations.retain(|violation| {
            scope.touches(
                &absolute_normalized(&base, &violation.path),
                violation.line,
                violation.line,
            )
        });
    }

    violations.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then(a.line.cmp(&b.line))
            .then(a.column.cmp(&b.column))
            .then(a.rule.cmp(&b.rule))
    });

    for violation in &violations {
        if violation.baselined {
            summary.baselined += 1;
        }
        if violation.fixed {
            summary.fixed += 1;
        }
        match violation.severity {
            Severity::Error => summary.errors += 1,
            Severity::Warn => summary.warnings += 1,
            Severity::Off => {}
        }
    }

    Ok(Outcome {
        violations,
        notes,
        summary,
    })
}

/// Everything the per-file work needs, shared immutably across threads.
struct FileContext {
    repo_root: Option<PathBuf>,
    /// Relative paths, the config search and the id anchor all resolve against this.
    base: PathBuf,
    current_issue: Option<String>,
    todo_key: Option<String>,
    fix: bool,
    baseline: HashSet<(String, Rule)>,
}

enum FileOutcome {
    Linted { violations: Vec<Violation> },
    Skipped { note: String },
    Failed { path: PathBuf, error: String },
}

fn lint_file(path: &Path, config: &LintConfig, config_manager: &ConfigManager, context: &FileContext) -> FileOutcome {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) => {
            // Not valid UTF-8, or unreadable: nothing to lint and nothing worth failing over.
            return FileOutcome::Skipped {
                note: format!("skipped {}: {error}", to_slash(path)),
            };
        }
    };

    let resolved = config_manager.get_config_for_file(path);
    let mut processor = Processor::new_with_config(config_manager);
    let comments = match processor.inspect(&content, path, &resolved) {
        Ok(comments) => comments,
        Err(error) => {
            return FileOutcome::Failed {
                path: path.to_path_buf(),
                error: format!("{error:#}"),
            };
        }
    };

    let texts: Vec<&str> = comments.iter().map(|comment| comment.text.as_str()).collect();
    let occurrences = assign_occurrence_indices(&texts);
    let id_path = id_path(context.repo_root.as_deref(), path, &context.base);
    let display = display_path(path, &context.base);

    let mut violations = Vec::new();
    let mut edits: Vec<Edit> = Vec::new();

    for (comment, occurrence) in comments.iter().zip(occurrences) {
        let findings = rules::check(
            comment,
            config,
            context.current_issue.as_deref(),
            context.todo_key.as_deref(),
        );
        if findings.is_empty() {
            continue;
        }

        let id = comment_id(&id_path, &comment.text, occurrence);
        for mut finding in findings {
            // `tag-not-canonical` covers a tag spelled differently from the canonical one and the
            // canonical tag spelled in the wrong casing, which cannot share one wording. The
            // phrasing lives with the casing rules, on [`LintConfig`].
            if finding.rule == Rule::TagNotCanonical {
                finding.message = config.tag_not_canonical_message(&finding.tag);
            }

            let (line, column) = line_and_column(&content, finding.offset);
            let fixed = context.fix && finding.is_fixable();
            if fixed {
                edits.extend(finding.edits.iter().cloned());
            }

            violations.push(Violation {
                path: display.clone(),
                baselined: context.baseline.contains(&(id.clone(), finding.rule)),
                id: id.clone(),
                rule: finding.rule,
                severity: finding.severity,
                line,
                column,
                message: finding.message,
                excerpt: finding.excerpt,
                tag: finding.tag,
                key: finding.key,
                fixed,
            });
        }
    }

    if !edits.is_empty() {
        match apply_edits(&content, edits)
            .and_then(|fixed| fs::write(path, &fixed).with_context(|| format!("failed to write {}", path.display())))
        {
            Ok(()) => {}
            Err(error) => {
                return FileOutcome::Failed {
                    path: path.to_path_buf(),
                    error: format!("{error:#}"),
                };
            }
        }
    }

    FileOutcome::Linted { violations }
}

/// The issue key of the current branch, or `None` with a note explaining why there is none.
///
/// Never an error: not being in a repository, sitting on a detached `HEAD`, or working on a branch
/// whose name carries no key are all normal, and `todo-self-reference` simply cannot be judged in
/// those cases.
fn resolve_current_issue(
    repo_root: Option<&Path>,
    targets: &[(PathBuf, Arc<LintConfig>)],
    notes: &mut Vec<String>,
) -> Option<String> {
    // Any target's pattern will do for the branch: they differ only if the configs do, and the note
    // below names the branch so a mismatch is visible.
    let (_, config) = targets
        .iter()
        .find(|(_, config)| config.is_on(Rule::TodoSelfReference))?;

    let skipped = |notes: &mut Vec<String>, reason: &str| {
        notes.push(format!("{} not checked: {reason}", Rule::TodoSelfReference.as_str()));
        None
    };

    let Some(root) = repo_root else {
        return skipped(notes, "no git repository encloses the files being linted");
    };
    let Some(branch) = crate::git::current_branch(root) else {
        return skipped(notes, "HEAD is detached or unreadable");
    };
    match crate::git::current_issue_key(root, &config.branch_pattern) {
        Some(key) => Some(key),
        None => skipped(
            notes,
            &format!("branch `{branch}` matches lint.current_issue_from_branch nowhere"),
        ),
    }
}

/// The path a comment id is anchored at: repo-relative when there is a repository, else relative to
/// the base directory. Anchoring at the repository root is what lets a baseline file be shared.
fn id_path(repo_root: Option<&Path>, file: &Path, base: &Path) -> String {
    let absolute = absolute_normalized(base, file);
    let anchor = repo_root.unwrap_or(base);
    to_slash(&repo_relative(anchor, &absolute).unwrap_or(absolute))
}

fn display_path(file: &Path, base: &Path) -> PathBuf {
    let absolute = absolute_normalized(base, file);
    repo_relative(base, &absolute).unwrap_or(absolute)
}

/// 1-based line and 1-based byte column of `offset` in `content`.
fn line_and_column(content: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(content.len());
    let preceding = &content[..offset];
    let line = preceding.matches('\n').count() + 1;
    let column = offset - preceding.rfind('\n').map_or(0, |index| index + 1) + 1;
    (line, column)
}

/// The configuration every part of a lint run reads: the `[lint]` table itself, and the `[languages]`
/// entries `inspect` needs in order to find comments at all.
///
/// Lint cares only about which comments exist, never which to remove, so the removal policy in
/// `[global]` is irrelevant here — but it travels in the same file, and since `[lint]` is a field on
/// [`crate::config::Config`] one load serves both. Per-directory layering below `base` comes with it.
fn build_config_manager(base: &Path, forced: Option<&Path>) -> Result<ConfigManager> {
    match forced {
        Some(path) => {
            let path = absolute_normalized(base, path);
            ConfigManager::from_config_file(base, &path)
                .with_context(|| format!("failed to load config file: {}", path.display()))
        }
        None => ConfigManager::new(base).context("failed to initialize configuration manager"),
    }
}

/// Files to lint, in a stable order, filtered to extensions a grammar is known for.
fn collect_files(
    base: &Path,
    paths: &[String],
    respect_gitignore: bool,
    registry: &LanguageRegistry,
    excludes: &ExcludeSet,
) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for pattern in paths {
        let path = absolute_normalized(base, Path::new(pattern));
        if path.is_file() {
            if !excludes.is_excluded(&path) && registry.detect_language(&path).is_some() {
                files.push(path);
            }
        } else if path.is_dir() {
            if !excludes.prunes_dir(&path) {
                walk_dir(&path, respect_gitignore, registry, excludes, &mut files);
            }
        } else {
            let pattern = to_slash(&path);
            for entry in glob::glob(&pattern).with_context(|| format!("invalid path or glob: {pattern}"))? {
                let entry = entry.with_context(|| format!("failed to read a match for {pattern}"))?;
                if entry.is_file() && !excludes.is_excluded(&entry) && registry.detect_language(&entry).is_some() {
                    files.push(entry);
                }
            }
        }
    }

    files.sort();
    files.dedup();
    Ok(files)
}

fn walk_dir(
    dir: &Path,
    respect_gitignore: bool,
    registry: &LanguageRegistry,
    excludes: &ExcludeSet,
    files: &mut Vec<PathBuf>,
) {
    let walker = ignore::WalkBuilder::new(dir)
        .hidden(false)
        .git_ignore(respect_gitignore)
        .git_global(respect_gitignore)
        .git_exclude(respect_gitignore)
        .parents(respect_gitignore)
        .require_git(false)
        .filter_entry(excludes.walk_filter())
        .build();

    for entry in walker.flatten() {
        let path = entry.path();
        if path.is_file() && !excludes.is_excluded(path) && registry.detect_language(path).is_some() {
            files.push(path.to_path_buf());
        }
    }
}

/// Baseline file: `{"version": 1, "violations": [{"id": …, "rule": …, "path": …, "excerpt": …}]}`.
///
/// `id` and `rule` are the key; `path` and `excerpt` are there so a human reviewing the file can see
/// what was accepted.
fn load_baseline(path: &Path) -> Result<HashSet<(String, Rule)>> {
    let text = fs::read_to_string(path).with_context(|| {
        format!(
            "failed to read baseline {} (write one with --write-baseline)",
            path.display()
        )
    })?;
    let document: serde_json::Value =
        serde_json::from_str(&text).with_context(|| format!("failed to parse baseline {}", path.display()))?;

    let version = document.get("version").and_then(serde_json::Value::as_u64);
    if version != Some(u64::from(BASELINE_VERSION)) {
        bail!(
            "baseline {} has version {:?}, expected {BASELINE_VERSION}: rewrite it with --write-baseline",
            path.display(),
            version
        );
    }

    let mut entries = HashSet::new();
    let recorded = document
        .get("violations")
        .and_then(serde_json::Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    for entry in recorded {
        let id = entry.get("id").and_then(serde_json::Value::as_str);
        let rule = entry
            .get("rule")
            .and_then(serde_json::Value::as_str)
            .and_then(rule_from_str);
        if let (Some(id), Some(rule)) = (id, rule) {
            entries.insert((id.to_string(), rule));
        }
    }

    Ok(entries)
}

fn write_baseline(path: &Path, violations: &[Violation]) -> Result<()> {
    // A set keyed the way `load_baseline` reads it, so the file holds no duplicate entries and its
    // order does not depend on how the files were walked.
    let mut entries: BTreeSet<(String, &'static str, String, String)> = BTreeSet::new();
    for violation in violations {
        entries.insert((
            violation.id.clone(),
            violation.rule.as_str(),
            to_slash(&violation.path),
            violation.excerpt.clone(),
        ));
    }

    let document = serde_json::json!({
        "version": BASELINE_VERSION,
        "violations": entries
            .into_iter()
            .map(|(id, rule, path, excerpt)| serde_json::json!({
                "id": id,
                "rule": rule,
                "path": path,
                "excerpt": excerpt,
            }))
            .collect::<Vec<_>>(),
    });

    let mut rendered = serde_json::to_string_pretty(&document).context("failed to render the baseline")?;
    rendered.push('\n');
    fs::write(path, rendered).with_context(|| format!("failed to write baseline {}", path.display()))
}

fn rule_from_str(name: &str) -> Option<Rule> {
    Rule::ALL.into_iter().find(|rule| rule.as_str() == name)
}

fn report(outcome: &Outcome, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Text => report_text(outcome),
        OutputFormat::Json => report_json(outcome),
    }
}

fn report_text(outcome: &Outcome) -> Result<()> {
    let Outcome {
        violations,
        notes,
        summary,
    } = outcome;

    for note in notes {
        anstream::eprintln!("{} {}", ui::dim(ui::BULLET), ui::dim(note));
    }

    for violation in violations {
        let label = if violation.fixed {
            ui::success("fixed")
        } else if violation.baselined {
            ui::dim("baselined")
        } else {
            match violation.severity {
                Severity::Error => ui::danger("error"),
                Severity::Warn => ui::warn("warning"),
                Severity::Off => ui::dim("off"),
            }
        };

        anstream::println!(
            "{}:{}:{} {} [{}] {}",
            ui::path(&violation.path),
            violation.line,
            violation.column,
            label,
            ui::accent(violation.rule.as_str()),
            violation.message
        );
        if !violation.excerpt.is_empty() {
            anstream::println!("    {}", ui::dim(&violation.excerpt));
        }
    }

    let failing = outcome.failing();
    let mut detail = vec![format!("{} file(s) linted", summary.files_linted)];
    if summary.baselined > 0 {
        detail.push(format!("{} baselined", summary.baselined));
    }
    if summary.fixed > 0 {
        detail.push(format!("{} fixed", summary.fixed));
    }
    if summary.warnings > 0 {
        detail.push(format!("{} warning(s)", summary.warnings));
    }
    if summary.uninspectable > 0 {
        detail.push(format!("{} file(s) could not be inspected", summary.uninspectable));
    }

    if failing == 0 && summary.uninspectable == 0 {
        anstream::println!(
            "{} {} ({})",
            ui::success(ui::CHECK),
            ui::success("no lint violations"),
            ui::dim(detail.join(", "))
        );
    } else {
        anstream::println!(
            "{} {} ({})",
            ui::danger("✗"),
            ui::danger(format!("{failing} violation(s)")),
            ui::dim(detail.join(", "))
        );
    }

    Ok(())
}

fn report_json(outcome: &Outcome) -> Result<()> {
    let Outcome {
        violations,
        notes,
        summary,
    } = outcome;

    let document = serde_json::json!({
        "violations": violations
            .iter()
            .map(|violation| serde_json::json!({
                "path": to_slash(&violation.path),
                "line": violation.line,
                "column": violation.column,
                "rule": violation.rule.as_str(),
                "severity": violation.severity.as_str(),
                "message": violation.message,
                "excerpt": violation.excerpt,
                "tag": violation.tag,
                "key": violation.key,
                "id": violation.id,
                "baselined": violation.baselined,
                "fixed": violation.fixed,
            }))
            .collect::<Vec<_>>(),
        "notes": notes,
        "summary": {
            "files_linted": summary.files_linted,
            "files_skipped_lint_disabled": summary.files_skipped_disabled,
            "violations": violations.len(),
            "errors": summary.errors,
            "warnings": summary.warnings,
            "baselined": summary.baselined,
            "fixed": summary.fixed,
            "uninspectable_files": summary.uninspectable,
            "failing": outcome.failing(),
        },
    });

    anstream::println!(
        "{}",
        serde_json::to_string_pretty(&document).context("failed to render the lint report")?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_and_column_are_one_based() {
        let content = "alpha\nbeta\ngamma\n";
        assert_eq!(line_and_column(content, 0), (1, 1));
        assert_eq!(line_and_column(content, 6), (2, 1));
        assert_eq!(line_and_column(content, 8), (2, 3));
        assert_eq!(line_and_column(content, content.len()), (4, 1));
    }

    #[test]
    fn a_rule_name_round_trips() {
        for rule in Rule::ALL {
            assert_eq!(rule_from_str(rule.as_str()), Some(rule));
        }
        assert_eq!(rule_from_str("no-such-rule"), None);
    }
}
