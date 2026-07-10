mod ast;
mod cli;
mod config;
pub mod languages;
pub mod processor;
mod rules;
mod ui;

use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Commands};
use config::ConfigManager;
use glob::glob;
use once_cell::sync::Lazy;
use processor::OutputWriter;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;

static DEFAULT_LANGUAGE_REGISTRY: Lazy<languages::LanguageRegistry> = Lazy::new(languages::LanguageRegistry::new);

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
        // Avoid panicking on broken pipes (e.g. `uncomment ... | head`) by restoring
        // SIGPIPE default behavior.
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
        };
    }

    let options = cli.args.processing_options();

    if cli.args.paths.is_empty() {
        anstream::eprintln!(
            "{} No input paths specified. Run {} for usage information.",
            ui::danger("error:"),
            ui::accent("uncomment --help")
        );
        std::process::exit(1);
    }

    let current_dir = std::env::current_dir().context("Failed to get current directory")?;

    let config_manager = if let Some(config_path) = &cli.args.config {
        let config = config::Config::from_file(config_path)
            .with_context(|| format!("Failed to load config file: {}", config_path.display()))?;

        ConfigManager::from_single_config(current_dir, config)?
    } else {
        ConfigManager::new(&current_dir).context("Failed to initialize configuration manager")?
    };

    let mut unsupported_report = UnsupportedFilesReport::default();
    let files = collect_files(&cli.args.paths, &options, &mut unsupported_report)?;

    print_unsupported_files_report(&unsupported_report, cli.args.verbose);

    if files.is_empty() {
        anstream::eprintln!(
            "{} No supported files found to process in the specified paths.",
            ui::warn("!")
        );
        anstream::eprintln!("{}", ui::dim(supported_extensions_message()));
        if options.respect_gitignore {
            anstream::eprintln!(
                "{}",
                ui::dim("Tip: Use --no-gitignore to process files ignored by git.")
            );
        }
        return Ok(());
    }

    let num_threads = if cli.args.threads == 0 {
        num_cpus::get()
    } else {
        cli.args.threads
    };

    if cli.args.verbose && num_threads > 1 {
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

    let output_writer = Arc::new(OutputWriter::new(
        options.dry_run,
        cli.args.verbose,
        options.dry_run && options.show_diff,
    ));

    let total_files = files.len();

    // Show a progress bar only for larger, non-verbose runs on a terminal;
    // verbose mode prints per-file lines that would fight the bar, and
    // `progress_bar` itself returns a hidden bar when stderr is not a TTY.
    let progress = if total_files >= ui::PROGRESS_MIN_FILES && !cli.args.verbose {
        ui::progress_bar(total_files as u64)
    } else {
        indicatif::ProgressBar::hidden()
    };

    let process_file = |file_path: &PathBuf| -> Option<processor::ProcessedFile> {
        let mut proc = processor::Processor::new_with_config(&config_manager);
        let result = match proc.process_file_with_config(file_path, &config_manager, Some(&options)) {
            Ok(mut pf) => {
                pf.modified = pf.original_content != pf.processed_content;
                Some(pf)
            }
            Err(e) => {
                progress.suspend(|| {
                    anstream::eprintln!("{} processing {}: {e}", ui::danger("error"), ui::path(file_path));
                    if cli.args.verbose {
                        anstream::eprintln!("  {}", ui::dim(format!("Full error: {e:?}")));
                    }
                });
                None
            }
        };
        progress.inc(1);
        result
    };

    let results: Vec<processor::ProcessedFile> = if num_threads == 1 {
        files.iter().filter_map(process_file).collect()
    } else {
        files.par_iter().filter_map(process_file).collect()
    };

    progress.finish_and_clear();

    let mut modified_files = 0usize;
    let mut comments_removed_total = 0usize;
    let mut important_removal_count = 0usize;
    let mut important_removal_samples: Vec<ImportantRemovalSample> = Vec::new();

    for processed_file in &results {
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

    if important_removal_count > 0 {
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

fn collect_files(
    paths: &[String],
    options: &processor::ProcessingOptions,
    unsupported: &mut UnsupportedFilesReport,
) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for path_pattern in paths {
        let path = Path::new(path_pattern);

        if path.is_file() {
            if has_supported_extension(path) {
                files.push(path.to_path_buf());
            } else {
                record_unsupported_file(path, unsupported);
            }
        } else if path.is_dir() {
            let pattern = format!("{}/**/*", path.display());
            collect_from_pattern(&pattern, &mut files, options, unsupported)?
        } else {
            collect_from_pattern(path_pattern, &mut files, options, unsupported)?
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

                    if path.is_file() {
                        if has_supported_extension(path) {
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
                    if path.is_file() {
                        if has_supported_extension(&path) {
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

fn has_supported_extension(path: &Path) -> bool {
    DEFAULT_LANGUAGE_REGISTRY.detect_language(path).is_some()
}

fn supported_extensions_message() -> String {
    let mut extensions: Vec<String> = DEFAULT_LANGUAGE_REGISTRY
        .get_supported_extensions()
        .into_iter()
        .map(|ext| format!(".{ext}"))
        .collect();
    extensions.push(".d.ts".to_string());
    extensions.sort();
    extensions.dedup();

    let mut shown = extensions;
    shown.sort();

    // Keep the message readable.
    const MAX: usize = 20;
    if shown.len() > MAX {
        shown.truncate(MAX);
        format!("Supported extensions: {}, and more.", shown.join(", "))
    } else {
        format!("Supported extensions: {}.", shown.join(", "))
    }
}
