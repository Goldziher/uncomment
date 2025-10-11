mod ast;
mod cli;
mod config;
mod grammar;
pub mod languages;
pub mod processor;
mod rules;

use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Commands};
use config::ConfigManager;
use glob::glob;
use processor::OutputWriter;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

fn main() -> Result<()> {
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
        eprintln!("Error: No input paths specified. Use 'uncomment --help' for usage information.");
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

    let files = collect_files(&cli.args.paths, &options)?;

    if files.is_empty() {
        eprintln!("No supported files found to process in the specified paths.");
        eprintln!("Supported extensions: .rs, .py, .js, .jsx, .mjs, .cjs, .ts, .tsx, .mts, .cts, .d.ts, .java, .go, .c, .cpp, .rb, and more.");
        if options.respect_gitignore {
            eprintln!("Tip: Use --no-gitignore to process files ignored by git.");
        }
        return Ok(());
    }

    let num_threads = if cli.args.threads == 0 {
        num_cpus::get()
    } else {
        cli.args.threads
    };

    if cli.args.verbose && num_threads > 1 {
        println!("🔧 Using {num_threads} parallel threads");
    }

    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build_global()
        .unwrap();

    let output_writer = Arc::new(OutputWriter::new(options.dry_run, cli.args.verbose));

    let total_files = files.len();
    let results = Arc::new(Mutex::new(Vec::new()));
    let modified_count = Arc::new(Mutex::new(0usize));

    if num_threads == 1 {
        let mut processor = processor::Processor::new_with_config(&config_manager);

        for file_path in files {
            match processor.process_file_with_config(&file_path, &config_manager, Some(&options)) {
                Ok(mut processed_file) => {
                    processed_file.modified =
                        processed_file.original_content != processed_file.processed_content;

                    if processed_file.modified {
                        *modified_count.lock().unwrap() += 1;
                    }

                    output_writer.write_file(&processed_file)?;
                }
                Err(e) => {
                    eprintln!("Error processing {}: {}", file_path.display(), e);
                    if cli.args.verbose {
                        eprintln!("  Full error: {e:?}");
                    }
                }
            }
        }
    } else {
        files.par_iter().for_each(|file_path| {
            let mut processor = processor::Processor::new_with_config(&config_manager);

            match processor.process_file_with_config(file_path, &config_manager, Some(&options)) {
                Ok(mut processed_file) => {
                    processed_file.modified =
                        processed_file.original_content != processed_file.processed_content;

                    if processed_file.modified {
                        *modified_count.lock().unwrap() += 1;
                    }

                    results.lock().unwrap().push(processed_file);
                }
                Err(e) => {
                    eprintln!("Error processing {}: {}", file_path.display(), e);
                    if cli.args.verbose {
                        eprintln!("  Full error: {e:?}");
                    }
                }
            }
        });

        let results = Arc::try_unwrap(results).unwrap().into_inner().unwrap();
        for processed_file in results {
            output_writer.write_file(&processed_file)?;
        }
    }

    let modified_files = *modified_count.lock().unwrap();
    output_writer.print_summary(total_files, modified_files);

    Ok(())
}

fn collect_files(paths: &[String], options: &processor::ProcessingOptions) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for path_pattern in paths {
        let path = Path::new(path_pattern);

        if path.is_file() {
            files.push(path.to_path_buf());
        } else if path.is_dir() {
            let pattern = format!("{}/**/*", path.display());
            collect_from_pattern(&pattern, &mut files, options)?
        } else {
            collect_from_pattern(path_pattern, &mut files, options)?
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
            pattern_path_buf.clone()
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
                (root, Some(absolute_pattern.clone()))
            } else {
                (absolute_pattern.clone(), None)
            }
        } else {
            (absolute_pattern.clone(), None)
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

                    if let Some(ref prefix) = filter_prefix {
                        if !path.starts_with(prefix) {
                            continue;
                        }
                    }

                    if path.is_file() && has_supported_extension(path) {
                        files.push(path.to_path_buf());
                    }
                }
                Err(e) => eprintln!("Error reading path: {e}"),
            }
        }
    } else {
        for entry in glob(pattern).context("Failed to parse glob pattern")? {
            match entry {
                Ok(path) => {
                    if path.is_file() && has_supported_extension(&path) {
                        files.push(path);
                    }
                }
                Err(e) => eprintln!("Error reading path: {e}"),
            }
        }
    }
    Ok(())
}

fn has_supported_extension(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy().to_lowercase();
        let supported_extensions = [
            "py", "pyw", "pyi", "pyx", "pxd", "js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts",
            "rs", "go", "java", "c", "h", "cpp", "cc", "cxx", "hpp", "hxx", "hh", "rb", "yml",
            "yaml", "hcl", "tf", "tfvars",
        ];

        if path.to_string_lossy().ends_with(".d.ts") {
            return true;
        }

        if let Some(filename) = path.file_name() {
            let filename_str = filename.to_string_lossy().to_lowercase();
            if filename_str == "makefile" || filename_str.ends_with(".mk") {
                return true;
            }
        }

        supported_extensions.iter().any(|&e| e == ext_str)
    } else if let Some(filename) = path.file_name() {
        let filename_str = filename.to_string_lossy().to_lowercase();
        filename_str == "makefile"
    } else {
        false
    }
}
