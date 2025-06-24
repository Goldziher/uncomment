mod ast;
mod cli;
pub mod languages;
pub mod processor;
mod rules;

use anyhow::{Context, Result};
use clap::Parser;
use glob::glob;
use processor::OutputWriter;
use std::path::{Path, PathBuf};

fn main() -> Result<()> {
    let cli = cli::Cli::parse();
    let options = cli.processing_options();

    // Validate input paths
    if cli.paths.is_empty() {
        eprintln!("Error: No input paths specified. Use 'uncomment --help' for usage information.");
        std::process::exit(1);
    }

    // Collect all files to process
    let files = collect_files(&cli.paths, &options)?;

    if files.is_empty() {
        eprintln!("No supported files found to process in the specified paths.");
        eprintln!("Supported extensions: .rs, .py, .js, .ts, .java, .go, .c, .cpp, .rb, and more.");
        if options.respect_gitignore {
            eprintln!("Tip: Use --no-gitignore to process files ignored by git.");
        }
        return Ok(());
    }

    // Create processor and output writer
    let mut processor = processor::Processor::new();
    let output_writer = OutputWriter::new(options.dry_run, cli.verbose);

    // Process files
    let mut total_files = 0;
    let mut modified_files = 0;

    for file_path in files {
        match processor.process_file(&file_path, &options) {
            Ok(mut processed_file) => {
                processed_file.modified =
                    processed_file.original_content != processed_file.processed_content;

                if processed_file.modified {
                    modified_files += 1;
                }

                output_writer.write_file(&processed_file)?;
                total_files += 1;
            }
            Err(e) => {
                eprintln!("Error processing {}: {}", file_path.display(), e);
                if cli.verbose {
                    eprintln!("  Full error: {:?}", e);
                }
            }
        }
    }

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
    for entry in glob(pattern).context("Failed to parse glob pattern")? {
        match entry {
            Ok(path) => {
                if path.is_file() && should_process_file(&path, options)? {
                    files.push(path);
                }
            }
            Err(e) => eprintln!("Error reading path: {}", e),
        }
    }
    Ok(())
}

/// Check if a file should be processed
fn should_process_file(path: &Path, options: &processor::ProcessingOptions) -> Result<bool> {
    // Check if file has a supported extension
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy();
        // Check for supported extensions
        let supported_extensions = [
            "py", "pyw", "pyi", // Python
            "js", "jsx", "mjs", "cjs", // JavaScript
            "ts", "tsx", "mts", "cts",  // TypeScript
            "rs",   // Rust
            "go",   // Go
            "java", // Java
            "c", "h", // C
            "cpp", "cc", "cxx", // C++
            "hpp", "hxx", // C++ headers
            "rb", "rake", // Ruby
        ];

        if !supported_extensions.iter().any(|&e| e == ext_str) {
            return Ok(false);
        }
    } else {
        return Ok(false);
    }

    // Check gitignore if needed
    if options.respect_gitignore {
        // Use the ignore crate to check gitignore rules
        use ignore::gitignore::GitignoreBuilder;

        let mut builder = GitignoreBuilder::new(path.parent().unwrap_or(Path::new(".")));

        // Add .gitignore file if it exists
        let gitignore_path = path.parent().unwrap_or(Path::new(".")).join(".gitignore");
        if gitignore_path.exists() {
            builder.add(gitignore_path);
        }

        if let Ok(gitignore) = builder.build() {
            let matched = gitignore.matched(path, path.is_dir());
            // If the file is ignored, don't process it
            if matched.is_ignore() {
                return Ok(false);
            }
        }
    }

    Ok(true)
}
