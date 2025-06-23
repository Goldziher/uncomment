mod ast;
mod cli;
pub mod languages;
mod output;
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

    // Collect all files to process
    let files = collect_files(&cli.paths, options.respect_gitignore)?;

    if files.is_empty() {
        eprintln!("No files found to process");
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
            }
        }
    }

    output_writer.print_summary(total_files, modified_files);

    Ok(())
}

/// Collect all files to process based on paths and patterns
fn collect_files(paths: &[String], respect_gitignore: bool) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for path_pattern in paths {
        let path = Path::new(path_pattern);

        if path.is_file() {
            // Direct file path
            files.push(path.to_path_buf());
        } else if path.is_dir() {
            // Directory - expand to recursive pattern
            let pattern = format!("{}/**/*", path.display());
            collect_from_pattern(&pattern, &mut files, respect_gitignore)?;
        } else {
            // Treat as glob pattern
            collect_from_pattern(path_pattern, &mut files, respect_gitignore)?;
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
    respect_gitignore: bool,
) -> Result<()> {
    for entry in glob(pattern).context("Failed to parse glob pattern")? {
        match entry {
            Ok(path) => {
                if path.is_file() && should_process_file(&path, respect_gitignore)? {
                    files.push(path);
                }
            }
            Err(e) => eprintln!("Error reading path: {}", e),
        }
    }
    Ok(())
}

/// Check if a file should be processed
fn should_process_file(path: &Path, respect_gitignore: bool) -> Result<bool> {
    // Check if file has a supported extension
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy();
        // Quick check for common extensions
        let supported_extensions = [
            "py", "pyw", "pyi", "js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts", "rs", "go",
            "java", "c", "h", "cpp", "cc", "cxx", "hpp", "hxx", "rb", "rake",
        ];

        if !supported_extensions.iter().any(|&e| e == ext_str) {
            return Ok(false);
        }
    } else {
        return Ok(false);
    }

    // Check gitignore if needed
    if respect_gitignore {
        // Use the ignore crate to check gitignore rules
        use ignore::WalkBuilder;
        let walker = WalkBuilder::new(path.parent().unwrap_or(Path::new(".")))
            .max_depth(Some(0))
            .hidden(false)
            .build();

        for entry in walker.flatten() {
            if entry.path() == path {
                return Ok(true);
            }
        }
        return Ok(false);
    }

    Ok(true)
}
