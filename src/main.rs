use clap::Parser;
use std::io;

use uncomment::cli::Cli;
use uncomment::language::detection::detect_language;
use uncomment::models::options::ProcessOptions;
use uncomment::processing::file::process_file;
use uncomment::utils::path::expand_paths;

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    let paths = expand_paths(&cli.paths);
    if paths.is_empty() {
        eprintln!("No files found matching the provided patterns.");
        return Ok(());
    }

    let mut processed_count = 0;
    let mut modified_count = 0;

    for path in &paths {
        if let Some(language) = detect_language(path) {
            let options = ProcessOptions {
                remove_todo: cli.remove_todo,
                remove_fixme: cli.remove_fixme,
                remove_doc: cli.remove_doc,
                ignore_patterns: &cli.ignore_patterns,
                output_dir: &cli.output_dir,
                disable_default_ignores: cli.disable_default_ignores,
                dry_run: cli.dry_run,
            };

            match process_file(path, &language, &options) {
                Ok(was_modified) => {
                    processed_count += 1;
                    if was_modified {
                        modified_count += 1;
                    }
                }
                Err(e) => {
                    eprintln!("Error processing file {}: {}", path.display(), e);
                }
            }
        } else {
            eprintln!("Unsupported file type: {}", path.display());
        }
    }

    println!(
        "Processed {} files, modified {}{}",
        processed_count,
        modified_count,
        if cli.dry_run { " (dry run)" } else { "" }
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    // Tests are moved to the appropriate modules
}
