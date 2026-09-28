use anyhow::{Context, Result};
use clap::CommandFactory;
use clap::Parser;
use glob::glob;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uncomment::changes::ChangeScope;
use uncomment::check;
use uncomment::cli::{Cli, Commands};
use uncomment::config::{self, ConfigManager, ExcludeSet};
use uncomment::languages::LanguageRegistry;
use uncomment::languages::registry::warn_languages_without_a_grammar;
use uncomment::paths::{absolute_normalized, find_repo_root, to_slash};
use uncomment::processor::{self, OutputWriter};
use uncomment::ui;

#[derive(Debug, Default)]
struct UnsupportedFilesReport {
    total: usize,
    by_extension: std::collections::BTreeMap<String, usize>,
    samples: Vec<PathBuf>,
}

type ImportantRemovalSample = (Arc<PathBuf>, processor::ImportantRemoval);

fn main() -> Result<()> {
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    let cli = Cli::parse();

    if let Some(command) = &cli.command {
        return match command {
            Commands::Init {
                output,
                force,
                comprehensive,
                interactive,
            } => Cli::handle_init_command(output, *force, *comprehensive, *interactive),
            Commands::Scan(args) => uncomment::scan::command::run(args),
            Commands::Keep(args) => uncomment::keep::run(args),
            // `lint` reports its verdict through the exit code, so a pre-commit hook or a CI step can
            // gate on it. Exiting only on failure keeps the success path running every `Drop`.
            Commands::Lint(args) => match uncomment::lint::run(args)? {
                0 => Ok(()),
                code => std::process::exit(code),
            },
        };
    }

    // The change-scope flags are shared with `lint`, where they need no `--check`, so clap cannot
    // express the requirement on the struct itself.
    if cli.check.scope.is_active() && !cli.check.check {
        Cli::command()
            .error(
                clap::error::ErrorKind::MissingRequiredArgument,
                "--changed-only, --changed-lines and --staged narrow a check, so they require --check",
            )
            .exit();
    }

    // Under `--check` a failure to run is exit 2, never 1, so a gate can tell "found comments" from
    // "could not look" — the latter must not be mistaken for either verdict.
    match run(&cli) {
        Ok(check::EXIT_CLEAN) => Ok(()),
        Ok(code) => std::process::exit(code),
        Err(error) if cli.check.check => {
            anstream::eprintln!("{} {error:#}", ui::danger("error:"));
            std::process::exit(check::EXIT_ERROR);
        }
        Err(error) => Err(error),
    }
}

/// What processing one file came to.
enum FileResult {
    Processed(processor::ProcessedFile),
    /// Not UTF-8 text, so there are no comments to judge. Only `--check` distinguishes this.
    Skipped(PathBuf),
    /// Already reported on stderr.
    Failed,
}

/// Whether `error` is a file that is not valid UTF-8, which `read_to_string` reports as
/// `InvalidData`.
fn is_not_utf8(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io| io.kind() == std::io::ErrorKind::InvalidData)
    })
}

/// The removal run, or under `--check` the gate. Returns the process exit code.
fn run(cli: &Cli) -> Result<i32> {
    let checking = cli.check.check;
    let mut options = cli.args.processing_options();
    // `--check` writes nothing whatever else was passed; `--dry-run` alongside it is redundant.
    options.dry_run |= checking;

    if cli.args.paths.is_empty() {
        anstream::eprintln!(
            "{} No input paths specified. Run {} for usage information.",
            ui::danger("error:"),
            ui::accent("uncomment --help")
        );
        return Ok(if checking { check::EXIT_ERROR } else { 1 });
    }

    let current_dir = std::env::current_dir().context("Failed to get current directory")?;

    let mut config_manager = if let Some(config_path) = &cli.args.config {
        let config = config::Config::from_file(config_path)
            .with_context(|| format!("Failed to load config file: {}", config_path.display()))?;

        ConfigManager::from_single_config(current_dir.clone(), config)?
    } else {
        ConfigManager::new(&current_dir).context("Failed to initialize configuration manager")?
    };

    // Which files are collected at all depends on the custom languages in force, so the
    // declarations have to be gathered before the walk rather than per file with everything else.
    config_manager.discover_language_sources(&cli.args.paths, options.respect_gitignore);
    let mut registry = LanguageRegistry::new();
    warn_languages_without_a_grammar(&registry.register_configured_languages(&config_manager.get_all_languages()));

    let excludes = config_manager.exclude_set(&cli.args.exclude)?;

    let mut unsupported_report = UnsupportedFilesReport::default();
    let mut files = collect_files(&cli.args.paths, &options, &registry, &excludes, &mut unsupported_report)?;

    // A pre-commit hook hands over every staged file, most of them not source; under `--check` that
    // is expected, not news.
    if !checking || cli.args.verbose {
        print_unsupported_files_report(&unsupported_report, cli.args.verbose);
    }

    let mut notes = Vec::new();
    let scope = ChangeScope::resolve(&cli.check.scope, find_repo_root(&current_dir).as_deref())?;
    if let Some(scope) = &scope {
        let before = files.len();
        files.retain(|file| scope.contains_file(&absolute_normalized(&current_dir, file)));
        notes.push(scope.note(files.len(), before));
    }

    if files.is_empty() {
        if checking {
            let outcome = check::Outcome {
                notes,
                ..check::Outcome::default()
            };
            check::report(&outcome, cli.check.format, cli.args.quiet)?;
            return Ok(outcome.exit_code());
        }
        anstream::eprintln!(
            "{} No supported files found to process in the specified paths.",
            ui::warn("!")
        );
        anstream::eprintln!("{}", ui::dim(supported_extensions_message(&registry)));
        if options.respect_gitignore {
            anstream::eprintln!(
                "{}",
                ui::dim("Tip: Use --no-gitignore to process files ignored by git.")
            );
        }
        return Ok(0);
    }

    let num_threads = if cli.args.threads == 0 {
        num_cpus::get()
    } else {
        cli.args.threads
    };

    if cli.args.verbose && num_threads > 1 && !checking {
        anstream::println!(
            "{} {}",
            ui::dim(ui::BULLET),
            ui::dim(format!("Using {num_threads} parallel threads"))
        );
    }

    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build_global()
        .context("Failed to initialize thread pool")?;

    let total_files = files.len();

    let progress = if total_files >= ui::PROGRESS_MIN_FILES && !cli.args.verbose {
        ui::progress_bar(total_files as u64)
    } else {
        indicatif::ProgressBar::hidden()
    };

    let process_file = |file_path: &PathBuf| -> FileResult {
        let mut proc = processor::Processor::new_with_config(&config_manager);
        let result = match proc.process_file_with_config(file_path, &config_manager, Some(&options)) {
            Ok(mut pf) => {
                pf.modified = pf.original_content != pf.processed_content;
                FileResult::Processed(pf)
            }
            Err(e) if checking && is_not_utf8(&e) => FileResult::Skipped(file_path.clone()),
            Err(e) => {
                progress.suspend(|| {
                    anstream::eprintln!("{} processing {}: {e}", ui::danger("error"), ui::path(file_path));
                    if cli.args.verbose {
                        anstream::eprintln!("  {}", ui::dim(format!("Full error: {e:?}")));
                    }
                });
                FileResult::Failed
            }
        };
        progress.inc(1);
        result
    };

    let outcomes: Vec<FileResult> = if num_threads == 1 {
        files.iter().map(process_file).collect()
    } else {
        files.par_iter().map(process_file).collect()
    };

    progress.finish_and_clear();

    // A config below the invocation directory is only read during the per-file pass, from a call
    // that cannot return an error, so it is recorded instead. It has already been printed;
    // what is left is to not act on results computed under built-in defaults the user never
    // asked for. Checked before the write loop so nothing is rewritten.
    if config_manager.deferred_config_error().is_some() {
        anstream::eprintln!(
            "{} configuration was rejected, so no file was modified.",
            ui::danger("error:")
        );
        return Ok(if checking { check::EXIT_ERROR } else { 1 });
    }

    let mut results = Vec::with_capacity(outcomes.len());
    let mut uninspectable = 0usize;
    for outcome in outcomes {
        match outcome {
            FileResult::Processed(processed) => results.push(processed),
            FileResult::Skipped(path) => notes.push(format!(
                "skipped {}: not UTF-8 text",
                to_slash(&check::display_path(&current_dir, &path))
            )),
            FileResult::Failed => uninspectable += 1,
        }
    }

    if checking {
        let mut outcome = check::Outcome::from_results(&current_dir, &results, scope.as_ref());
        notes.append(&mut outcome.notes);
        outcome.notes = notes;
        outcome.uninspectable = uninspectable;
        check::report(&outcome, cli.check.format, cli.args.quiet)?;
        return Ok(outcome.exit_code());
    }

    report_removal(cli, &options, total_files, &results, &registry)?;
    Ok(0)
}

/// Write the processed files and print what the removal run did.
fn report_removal(
    cli: &Cli,
    options: &processor::ProcessingOptions,
    total_files: usize,
    results: &[processor::ProcessedFile],
    registry: &LanguageRegistry,
) -> Result<()> {
    let output_writer = OutputWriter::new(options.dry_run, cli.args.verbose, options.show_diff, cli.args.quiet);

    let mut modified_files = 0usize;
    let mut comments_removed_total = 0usize;
    let mut important_removal_count = 0usize;
    let mut important_removal_samples: Vec<ImportantRemovalSample> = Vec::new();

    for processed_file in results {
        if processed_file.modified {
            modified_files += 1;
            comments_removed_total += processed_file.comments_removed;
        }

        if !processed_file.important_removals.is_empty() {
            important_removal_count += processed_file.important_removals.len();
            const MAX_SAMPLES: usize = 20;
            let remaining = MAX_SAMPLES.saturating_sub(important_removal_samples.len());
            if remaining > 0 {
                let sample_path = Arc::new(processed_file.path.clone());
                for removal in processed_file.important_removals.iter().take(remaining) {
                    important_removal_samples.push((Arc::clone(&sample_path), removal.clone()));
                }
            }
        }

        output_writer.write_file(processed_file)?;
    }

    output_writer.print_summary(total_files, modified_files, comments_removed_total);

    if comments_removed_total > 0 && !cli.args.quiet {
        let marker_line = results
            .iter()
            .find(|processed_file| processed_file.modified)
            .and_then(|processed_file| registry.detect_language(&processed_file.path))
            .map_or_else(|| "comment".to_string(), marker_line_hint);
        anstream::eprintln!();
        anstream::eprintln!(
            "{}",
            ui::dim(format!(
                "Tip: to keep a comment, add `~keep` to it, or to a `{marker_line}` line just above it — TODO, \
                 FIXME and doc comments are kept by default."
            ))
        );
        anstream::eprintln!(
            "{}",
            ui::dim(
                "     Preserve matching text everywhere with --ignore \"<pattern>\" or preserve_patterns in .uncomment.toml."
            )
        );
        anstream::eprintln!("{}", ui::dim("     Preview changes first with --dry-run --diff."));
    }

    if important_removal_count > 0 && !cli.args.quiet {
        anstream::eprintln!(
            "{} removed {} potentially important comment(s). Re-run with {} to inspect.",
            ui::warn("warning:"),
            ui::warn(important_removal_count),
            ui::accent("--dry-run --diff")
        );
        if cli.args.verbose {
            anstream::eprintln!("{}", ui::dim("Examples:"));
            for (path, removal) in &important_removal_samples {
                anstream::eprintln!(
                    "  {}",
                    ui::dim(format!(
                        "- {}:{} [{}] {}",
                        path.display(),
                        removal.line,
                        removal.reason,
                        removal.preview
                    ))
                );
            }
        }
    }

    Ok(())
}

/// What to put inside the backticks of the "keep a comment" tip's second clause, for `language`'s
/// own comment syntax: the plain line token when it has one (`#`, `--`, ...), else its block pair
/// with the marker already inside it (`/* ~keep */`), since a bare open delimiter alone would not
/// read as a complete line.
fn marker_line_hint(language: &uncomment::languages::LanguageConfig) -> String {
    match language.line_comment_token() {
        Some(token) => token.to_string(),
        None => match language.block_comment_delimiters() {
            Some((open, close)) => format!("{open} ~keep {close}"),
            None => "comment".to_string(),
        },
    }
}

fn collect_files(
    paths: &[String],
    options: &processor::ProcessingOptions,
    registry: &LanguageRegistry,
    excludes: &ExcludeSet,
    unsupported: &mut UnsupportedFilesReport,
) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for path_pattern in paths {
        let path = Path::new(path_pattern);

        if path.is_file() {
            // Before the extension check: an excluded file is not a file the run declined to
            // support, and reporting it as unsupported would ask the user to act on it.
            if excludes.is_excluded(path) {
                continue;
            }
            if has_supported_extension(path, registry) {
                files.push(path.to_path_buf());
            } else {
                record_unsupported_file(path, unsupported);
            }
        } else if path.is_dir() {
            if excludes.prunes_dir(path) {
                continue;
            }
            let pattern = format!("{}/**/*", path.display());
            collect_from_pattern(&pattern, &mut files, options, registry, excludes, unsupported)?
        } else {
            collect_from_pattern(path_pattern, &mut files, options, registry, excludes, unsupported)?
        }
    }

    files.sort();
    files.dedup();

    Ok(files)
}

fn collect_from_pattern(
    pattern: &str,
    files: &mut Vec<PathBuf>,
    options: &processor::ProcessingOptions,
    registry: &LanguageRegistry,
    excludes: &ExcludeSet,
    unsupported: &mut UnsupportedFilesReport,
) -> Result<()> {
    if options.respect_gitignore {
        use ignore::WalkBuilder;
        use std::path::PathBuf;

        let pattern_path = if pattern.contains("/**/*") {
            pattern.strip_suffix("/**/*").unwrap_or(".")
        } else {
            pattern
        };

        let pattern_path_buf = PathBuf::from(pattern_path);
        let absolute_pattern = if pattern_path_buf.is_absolute() {
            pattern_path_buf
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(&pattern_path_buf)
        };

        let mut git_root = None;
        let mut current = absolute_pattern.as_path();
        while let Some(parent) = current.parent() {
            if parent.join(".git").exists() {
                git_root = Some(parent.to_path_buf());
                break;
            }
            current = parent;
        }

        let (walk_root, filter_prefix) = if let Some(root) = git_root {
            if absolute_pattern.starts_with(&root) {
                (root, Some(absolute_pattern))
            } else {
                (absolute_pattern, None)
            }
        } else {
            (absolute_pattern, None)
        };

        let walker = WalkBuilder::new(walk_root)
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

                    if let Some(ref prefix) = filter_prefix
                        && !path.starts_with(prefix)
                    {
                        continue;
                    }

                    if excludes.is_excluded(path) {
                        continue;
                    }

                    if path.is_file() {
                        if has_supported_extension(path, registry) {
                            files.push(path.to_path_buf());
                        } else {
                            record_unsupported_file(path, unsupported);
                        }
                    }
                }
                Err(e) => anstream::eprintln!("{} reading path: {e}", ui::danger("error")),
            }
        }
    } else {
        for entry in glob(pattern).context("Failed to parse glob pattern")? {
            match entry {
                Ok(path) => {
                    if excludes.is_excluded(&path) {
                        continue;
                    }
                    if path.is_file() {
                        if has_supported_extension(&path, registry) {
                            files.push(path);
                        } else {
                            record_unsupported_file(&path, unsupported);
                        }
                    }
                }
                Err(e) => anstream::eprintln!("{} reading path: {e}", ui::danger("error")),
            }
        }
    }
    Ok(())
}

fn record_unsupported_file(path: &Path, report: &mut UnsupportedFilesReport) {
    report.total += 1;

    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| format!(".{}", s.to_lowercase()))
        .unwrap_or_else(|| "<no extension>".to_string());

    *report.by_extension.entry(extension).or_insert(0) += 1;

    const MAX_SAMPLES: usize = 10;
    if report.samples.len() < MAX_SAMPLES {
        report.samples.push(path.to_path_buf());
    }
}

fn print_unsupported_files_report(report: &UnsupportedFilesReport, verbose: bool) {
    if report.total == 0 {
        return;
    }

    let mut top: Vec<(&String, &usize)> = report.by_extension.iter().collect();
    top.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));

    const MAX_TOP: usize = 8;
    let shown = top.into_iter().take(MAX_TOP).collect::<Vec<_>>();
    let mut summary = String::new();
    for (i, (ext, count)) in shown.iter().enumerate() {
        if i > 0 {
            summary.push_str(", ");
        }
        summary.push_str(&format!("{ext}={count}"));
    }

    anstream::eprintln!(
        "{} {}",
        ui::dim(ui::BULLET),
        ui::dim(format!("Skipping {} unsupported file(s) ({summary}).", report.total))
    );

    if verbose && !report.samples.is_empty() {
        anstream::eprintln!("{}", ui::dim("Examples:"));
        for sample in &report.samples {
            anstream::eprintln!("  {}", ui::dim(format!("- {}", sample.display())));
        }
    }
}

fn has_supported_extension(path: &Path, registry: &LanguageRegistry) -> bool {
    registry.detect_language(path).is_some()
}

fn supported_extensions_message(registry: &LanguageRegistry) -> String {
    let mut extensions: Vec<String> = registry
        .get_supported_extensions()
        .into_iter()
        .map(|ext| format!(".{ext}"))
        .collect();
    extensions.push(".d.ts".to_string());
    extensions.sort();
    extensions.dedup();

    let mut shown = extensions;
    shown.sort();

    const MAX: usize = 20;
    if shown.len() > MAX {
        shown.truncate(MAX);
        format!("Supported extensions: {}, and more.", shown.join(", "))
    } else {
        format!("Supported extensions: {}.", shown.join(", "))
    }
}
