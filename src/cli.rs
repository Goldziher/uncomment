use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "uncomment",
    version = "2.0.0",
    about = "Remove comments from code files using tree-sitter parsing."
)]
pub struct Cli {
    /// Paths to files or directories to process
    pub paths: Vec<String>,

    /// Remove TODO comments
    #[arg(short = 'r', long, default_value_t = false)]
    pub remove_todo: bool,

    /// Remove FIXME comments
    #[arg(short = 'f', long, default_value_t = false)]
    pub remove_fixme: bool,

    /// Remove documentation comments
    #[arg(short = 'd', long, default_value_t = false)]
    pub remove_doc: bool,

    /// Patterns to ignore (comments containing these patterns will be kept)
    #[arg(short = 'i', long)]
    pub ignore_patterns: Vec<String>,

    /// Disable default ignore patterns for each language
    #[arg(long = "no-default-ignores", default_value_t = false)]
    pub no_default_ignores: bool,

    /// Perform a dry run (don't modify files)
    #[arg(short = 'n', long, default_value_t = false)]
    pub dry_run: bool,

    /// Enable verbose output
    #[arg(short = 'v', long, default_value_t = false)]
    pub verbose: bool,

    /// Disable .gitignore file processing
    #[arg(long = "no-gitignore", default_value_t = false)]
    pub no_gitignore: bool,
}

impl Cli {
    pub fn processing_options(&self) -> crate::processor::ProcessingOptions {
        crate::processor::ProcessingOptions {
            remove_todo: self.remove_todo,
            remove_fixme: self.remove_fixme,
            remove_doc: self.remove_doc,
            custom_preserve_patterns: self.ignore_patterns.clone(),
            use_default_ignores: !self.no_default_ignores,
            dry_run: self.dry_run,
            respect_gitignore: !self.no_gitignore,
        }
    }
}
