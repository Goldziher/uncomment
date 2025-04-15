use clap::Parser;
use std::fs;
use std::io;

use uncomment::cli::Cli;
use uncomment::language::detection::detect_language;
use uncomment::models::options::ProcessOptions;
use uncomment::processing::file::process_file;
use uncomment::utils::path::expand_paths;

fn create_diff(original: &str, modified: &str, file_path: &str) -> String {
    let original_lines: Vec<&str> = original.lines().collect();
    let modified_lines: Vec<&str> = modified.lines().collect();

    let mut diff = format!("Diff for file: {}\n", file_path);
    diff.push_str("----------------------------\n");

    let mut i = 0;
    let mut j = 0;

    while i < original_lines.len() || j < modified_lines.len() {
        if i >= original_lines.len() {
            diff.push_str(&format!("+ [{}] {}\n", j + 1, modified_lines[j]));
            j += 1;
            continue;
        }

        if j >= modified_lines.len() {
            diff.push_str(&format!("- [{}] {}\n", i + 1, original_lines[i]));
            i += 1;
            continue;
        }

        if original_lines[i] != modified_lines[j] {
            if original_lines[i].trim().is_empty() && !modified_lines[j].trim().is_empty() {
                diff.push_str(&format!(
                    "~ [{}] (empty) -> [{}] {}\n",
                    i + 1,
                    j + 1,
                    modified_lines[j]
                ));
            } else if !original_lines[i].trim().is_empty() && modified_lines[j].trim().is_empty() {
                diff.push_str(&format!(
                    "~ [{}] {} -> [{}] (empty)\n",
                    i + 1,
                    original_lines[i],
                    j + 1
                ));
            } else {
                diff.push_str(&format!(
                    "~ [{}] {} -> [{}] {}\n",
                    i + 1,
                    original_lines[i],
                    j + 1,
                    modified_lines[j]
                ));
            }
        }

        i += 1;
        j += 1;
    }

    diff
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    let paths = expand_paths(&cli.paths);
    if paths.is_empty() {
        eprintln!("No files found matching the provided patterns.");
        return Ok(());
    }

    let mut processed_count = 0;
    let mut modified_count = 0;
    let mut diffs = Vec::new();

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

            let original_content = fs::read_to_string(path)?;

            match process_file(path, &language, &options) {
                Ok(was_modified) => {
                    processed_count += 1;
                    if was_modified {
                        modified_count += 1;

                        if cli.dry_run {
                            let processed_content = if let Some(output_dir) = &cli.output_dir {
                                let output_path = std::path::PathBuf::from(output_dir)
                                    .join(path.file_name().unwrap());
                                fs::read_to_string(&output_path)?
                            } else {
                                let temp_dir = tempfile::tempdir()?;
                                let temp_output_dir = temp_dir.path().to_string_lossy().to_string();

                                let temp_options = ProcessOptions {
                                    remove_todo: cli.remove_todo,
                                    remove_fixme: cli.remove_fixme,
                                    remove_doc: cli.remove_doc,
                                    ignore_patterns: &cli.ignore_patterns,
                                    output_dir: &Some(temp_output_dir.clone()),
                                    disable_default_ignores: cli.disable_default_ignores,
                                    dry_run: false,
                                };

                                process_file(path, &language, &temp_options)?;

                                let output_path = std::path::PathBuf::from(temp_output_dir)
                                    .join(path.file_name().unwrap());
                                fs::read_to_string(&output_path)?
                            };

                            let diff = create_diff(
                                &original_content,
                                &processed_content,
                                &path.to_string_lossy(),
                            );
                            diffs.push(diff);
                        }
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

    if cli.dry_run && !diffs.is_empty() {
        println!("\nDiff Output (showing changes that would be made):");
        println!("================================================");
        for diff in diffs {
            println!("{}", diff);
            println!();
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
