use clap::Parser;
use ignore::gitignore::Gitignore;
use std::path::{Path, PathBuf};
use std::fs;
use crate::processing::file::create_gitignore_matcher;

/// Command-line interface for the uncomment tool
#[derive(Parser, Debug)]
#[command(
    name = "uncomment",
    version = "1.0.5",
    about = "Remove comments from code files."
)]
pub struct Cli {
    /// Paths to files or directories to process
    pub paths: Vec<String>,

    /// Remove TODO comments
    #[arg(short, long, default_value_t = false)]
    pub remove_todo: bool,

    /// Remove FIXME comments
    #[arg(short = 'f', long, default_value_t = false)]
    pub remove_fixme: bool,

    /// Remove documentation comments
    #[arg(short = 'd', long, default_value_t = false)]
    pub remove_doc: bool,

    /// Patterns to ignore (comments containing these patterns will be kept)
    #[arg(short = 'i', long)]
    pub ignore_patterns: Option<Vec<String>>,

    /// Disable default ignore patterns for each language
    #[arg(long = "no-default-ignores", default_value_t = false)]
    pub disable_default_ignores: bool,

    /// Output directory for processed files
    #[arg(short, long, hide = true)]
    pub output_dir: Option<String>,

    /// Perform a dry run (don't modify files)
    #[arg(short = 'n', long, default_value_t = false)]
    pub dry_run: bool,

    /// Disable .gitignore file processing
    #[arg(long = "no-gitignore", default_value_t = false)]
    pub no_gitignore: bool,
}

/// Collect files to process, respecting .gitignore rules unless disabled
pub fn collect_files(paths: &[PathBuf], respect_gitignore: bool) -> Vec<PathBuf> {
    let mut files = Vec::new();
    
    for path in paths {
        if path.is_file() {
            // Skip .gitignore files
            if path.file_name().map(|name| name != ".gitignore").unwrap_or(true) {
                files.push(path.clone());
            }
        } else if path.is_dir() {
            // Use an empty Gitignore when not respecting .gitignore
            let gitignore = if respect_gitignore {
                create_gitignore_matcher(path)
            } else {
                Gitignore::empty()
            };
            walk_dir(path, &gitignore, &mut files, respect_gitignore);
        }
    }
    
    files
}

fn walk_dir(dir: &Path, gitignore: &Gitignore, files: &mut Vec<PathBuf>, respect_gitignore: bool) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            // Skip .gitignore files
            if path.file_name().map(|name| name == ".gitignore").unwrap_or(false) {
                continue;
            }
            if let Ok(relative_path) = path.strip_prefix(dir) {
                if !respect_gitignore || !gitignore.matched(relative_path, path.is_dir()).is_ignore() {
                    if path.is_dir() {
                        walk_dir(&path, gitignore, files, respect_gitignore);
                    } else if path.is_file() {
                        files.push(path);
                    }
                }
            }
        }
    }
}

/// Parse CLI arguments and return configuration
pub fn parse_args() -> Cli {
    Cli::parse()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use tempfile::tempdir;

    #[test]
    fn test_collect_files_respects_gitignore() {
        let dir = tempdir().unwrap();
        
        // Create test files
        File::create(dir.path().join("processed.rs")).unwrap();
        File::create(dir.path().join("ignored.rs")).unwrap();
        
        // Create .gitignore
        fs::write(dir.path().join(".gitignore"), "ignored.rs").unwrap();
        
        let paths = vec![dir.path().to_path_buf()];
        
        // Test with gitignore enabled
        let files = collect_files(&paths, true);
        assert_eq!(files.len(), 1, "Expected 1 file, got {}: {:?}", files.len(), files);
        assert!(files[0].ends_with("processed.rs"));
        
        // Test with gitignore disabled
        let files = collect_files(&paths, false);
        assert_eq!(files.len(), 2, "Expected 2 files, got {}: {:?}", files.len(), files);
    }

    #[test]
    fn test_cli_parsing() {
        let cli = Cli::parse_from(["uncomment", "src/main.rs", "--remove-todo"]);
        assert_eq!(cli.paths, vec!["src/main.rs"]);
        assert!(cli.remove_todo);
        assert!(!cli.remove_fixme);
        assert!(!cli.no_gitignore);
    }
}