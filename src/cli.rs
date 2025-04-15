use clap::Parser;

/// Command-line interface for the uncomment tool
#[derive(Parser, Debug)]
#[command(
    name = "uncomment",
    version = "1.0.4",
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
}
