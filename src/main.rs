mod ast;
mod cli;
mod config;
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

    // Handle subcommands
    if let Some(command) = &cli.command {
        return match command {
            Commands::Init { output, force } => Cli::handle_init_command(output, *force),
        };
    }

    // Process files with the main logic
    let options = cli.args.processing_options();

    // Validate input paths
    if cli.args.paths.is_empty() {
        eprintln!("Error: No input paths specified. Use 'uncomment --help' for usage information.");
        std::process::exit(1);
    }

    // Initialize configuration manager
    let current_dir = std::env::current_dir().context("Failed to get current directory")?;

    let config_manager = if let Some(config_path) = &cli.args.config {
        // Load specific config file
        let config = config::Config::from_file(config_path)
            .with_context(|| format!("Failed to load config file: {}", config_path.display()))?;

        // Create a temporary config manager with just this config
        ConfigManager::from_single_config(current_dir, config)?
    } else {
        // Discover and load all configs in the tree
        ConfigManager::new(&current_dir).context("Failed to initialize configuration manager")?
    };

    // Collect all files to process
    let files = collect_files(&cli.args.paths, &options)?;

    if files.is_empty() {
        eprintln!("No supported files found to process in the specified paths.");
        eprintln!("Supported extensions: .rs, .py, .js, .jsx, .mjs, .cjs, .ts, .tsx, .mts, .cts, .d.ts, .java, .go, .c, .cpp, .rb, and more.");
        if options.respect_gitignore {
            eprintln!("Tip: Use --no-gitignore to process files ignored by git.");
        }
        return Ok(());
    }

    // Configure thread pool
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

    // Create output writer
    let output_writer = Arc::new(OutputWriter::new(options.dry_run, cli.args.verbose));

    // Process files
    let total_files = files.len();
    let results = Arc::new(Mutex::new(Vec::new()));
    let modified_count = Arc::new(Mutex::new(0usize));

    if num_threads == 1 {
        // Single-threaded processing
        let mut processor = processor::Processor::new();

        for file_path in files {
            match processor.process_file_with_config(&file_path, &config_manager) {
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
        // Parallel processing
        files.par_iter().for_each(|file_path| {
            // Each thread gets its own processor
            let mut processor = processor::Processor::new();

            match processor.process_file_with_config(file_path, &config_manager) {
                Ok(mut processed_file) => {
                    processed_file.modified =
                        processed_file.original_content != processed_file.processed_content;

                    if processed_file.modified {
                        *modified_count.lock().unwrap() += 1;
                    }

                    // Collect results for sequential output
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

        // Write results sequentially to maintain output order
        let results = Arc::try_unwrap(results).unwrap().into_inner().unwrap();
        for processed_file in results {
            output_writer.write_file(&processed_file)?;
        }
    }

    let modified_files = *modified_count.lock().unwrap();
    output_writer.print_summary(total_files, modified_files);

    Ok(())
}

/// Collect all files to process based on paths and patterns
fn collect_files(paths: &[String], options: &processor::ProcessingOptions) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for path_pattern in paths {
        let path = Path::new(path_pattern);

        if path.is_file() {
            // Direct file path
            files.push(path.to_path_buf());
        } else if path.is_dir() {
            // Directory - expand to recursive pattern
            let pattern = format!("{}/**/*", path.display());
            collect_from_pattern(&pattern, &mut files, options)?
        } else {
            // Treat as glob pattern
            collect_from_pattern(path_pattern, &mut files, options)?
        }
    }

    // Remove duplicates
    files.sort();
    files.dedup();

    Ok(files)
}

/// Collect files matching a glob pattern
fn collect_from_pattern(
    pattern: &str,
    files: &mut Vec<PathBuf>,
    options: &processor::ProcessingOptions,
) -> Result<()> {
    // If we're respecting gitignore, use ignore crate's WalkBuilder for proper gitignore handling
    if options.respect_gitignore {
        use ignore::WalkBuilder;
        use std::path::PathBuf;

        // Extract the directory from the pattern for WalkBuilder
        let pattern_path = if pattern.contains("/**/*") {
            pattern.strip_suffix("/**/*").unwrap_or(".")
        } else {
            pattern
        };

        // Convert pattern to a PathBuf for manipulation
        let pattern_path_buf = PathBuf::from(pattern_path);
        let absolute_pattern = if pattern_path_buf.is_absolute() {
            pattern_path_buf.clone()
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(&pattern_path_buf)
        };

        // Find git repository root if we're in one
        let mut git_root = None;
        let mut current = absolute_pattern.as_path();
        while let Some(parent) = current.parent() {
            if parent.join(".git").exists() {
                git_root = Some(parent.to_path_buf());
                break;
            }
            current = parent;
        }

        // If we found a git root and our pattern is within it, start walking from the git root
        // but filter results to only include files under our target directory
        let (walk_root, filter_prefix) = if let Some(root) = git_root {
            if absolute_pattern.starts_with(&root) {
                // Walk from git root, but we'll filter to only include files under our target
                (root, Some(absolute_pattern.clone()))
            } else {
                // Pattern is outside git repo, use pattern directly
                (absolute_pattern.clone(), None)
            }
        } else {
            // No git repo, use pattern directly
            (absolute_pattern.clone(), None)
        };

        let walker = WalkBuilder::new(walk_root)
            .hidden(false) // We want to see hidden files if they match our extensions
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .parents(true) // Look for .gitignore in parent directories
            .require_git(false) // Work even outside git repos
            .build();

        for entry in walker {
            match entry {
                Ok(entry) => {
                    let path = entry.path();

                    // If we have a filter prefix, only include files under that directory
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
        // Fall back to glob for non-gitignore cases
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

/// Check if a file has a supported extension
fn has_supported_extension(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy().to_lowercase();
        // Check for supported extensions
        let supported_extensions = [
            "py", "pyw", "pyi", "pyx", "pxd", // Python
            "js", "jsx", "mjs", "cjs", // JavaScript
            "ts", "tsx", "mts", "cts",  // TypeScript
            "rs",   // Rust
            "go",   // Go
            "java", // Java
            "c", "h", // C
            "cpp", "cc", "cxx", "hpp", "hxx", "hh", // C++
            "rb", // Ruby
            "yml", "yaml", // YAML
            "hcl", "tf", "tfvars", // HCL/Terraform
        ];

        // Special handling for .d.ts files
        if path.to_string_lossy().ends_with(".d.ts") {
            return true;
        }

        // Special handling for Makefiles
        if let Some(filename) = path.file_name() {
            let filename_str = filename.to_string_lossy().to_lowercase();
            if filename_str == "makefile" || filename_str.ends_with(".mk") {
                return true;
            }
        }

        supported_extensions.iter().any(|&e| e == ext_str)
    } else {
        // Check for Makefiles without extension
        if let Some(filename) = path.file_name() {
            let filename_str = filename.to_string_lossy().to_lowercase();
            filename_str == "makefile"
        } else {
            false
        }
    }
}
