use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "uncomment",
    version,
    about = "Remove comments from code files using tree-sitter parsing",
    long_about = "A fast, accurate CLI tool that removes comments from source code files using tree-sitter AST parsing. Automatically preserves important comments like linting directives, documentation, and metadata."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[command(flatten)]
    pub args: ProcessArgs,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// ~keep Initialize a configuration file in the current directory
    #[command(about = "Create a template configuration file")]
    Init {
        /// ~keep Output file name
        #[arg(short, long, default_value = ".uncommentrc.toml")]
        output: PathBuf,

        /// ~keep Overwrite existing file
        #[arg(short, long)]
        force: bool,

        /// ~keep Generate configuration for all supported languages
        #[arg(
            long,
            help = "Generate comprehensive config with all supported languages"
        )]
        comprehensive: bool,

        /// ~keep Interactive mode to select languages
        #[arg(short, long, help = "Interactive mode to select languages and options")]
        interactive: bool,
    },
}

#[derive(Parser, Debug)]
pub struct ProcessArgs {
    /// ~keep Files or directories to process (supports glob patterns)
    #[arg(help = "Files, directories, or glob patterns to process")]
    pub paths: Vec<String>,

    /// ~keep Remove TODO comments (normally preserved)
    #[arg(short = 'r', long, help = "Remove TODO comments (normally preserved)")]
    pub remove_todo: bool,

    /// ~keep Remove FIXME comments (normally preserved)
    #[arg(short = 'f', long, help = "Remove FIXME comments (normally preserved)")]
    pub remove_fixme: bool,

    /// ~keep Remove documentation comments (normally preserved)
    #[arg(
        short = 'd',
        long,
        help = "Remove documentation comments and docstrings"
    )]
    pub remove_doc: bool,

    /// ~keep Additional patterns to preserve (beyond defaults)
    #[arg(
        short = 'i',
        long = "ignore",
        help = "Additional patterns to preserve (can be used multiple times)"
    )]
    pub ignore_patterns: Vec<String>,

    /// ~keep Disable automatic preservation of linting directives
    #[arg(
        long = "no-default-ignores",
        help = "Disable built-in preservation patterns (ESLint, Clippy, etc.)"
    )]
    pub no_default_ignores: bool,

    /// ~keep Show what would be changed without modifying files
    #[arg(short = 'n', long, help = "Show changes without modifying files")]
    pub dry_run: bool,

    /// ~keep Show detailed processing information
    #[arg(short = 'v', long, help = "Show detailed processing information")]
    pub verbose: bool,

    /// ~keep Ignore .gitignore rules when finding files
    #[arg(long = "no-gitignore", help = "Process files ignored by .gitignore")]
    pub no_gitignore: bool,

    /// ~keep Process files in nested git repositories
    #[arg(
        long = "traverse-git-repos",
        help = "Traverse into other git repositories (useful for monorepos)"
    )]
    pub traverse_git_repos: bool,

    /// ~keep Number of parallel threads (0 = number of CPU cores)
    #[arg(
        short = 'j',
        long = "threads",
        help = "Number of parallel threads (0 = auto-detect)",
        default_value = "1"
    )]
    pub threads: usize,

    /// ~keep Path to configuration file
    #[arg(
        short = 'c',
        long = "config",
        help = "Path to configuration file (overrides automatic discovery)"
    )]
    pub config: Option<PathBuf>,
}

impl ProcessArgs {
    pub fn processing_options(&self) -> crate::processor::ProcessingOptions {
        crate::processor::ProcessingOptions {
            remove_todo: self.remove_todo,
            remove_fixme: self.remove_fixme,
            remove_doc: self.remove_doc,
            custom_preserve_patterns: self.ignore_patterns.clone(),
            use_default_ignores: !self.no_default_ignores,
            dry_run: self.dry_run,
            respect_gitignore: !self.no_gitignore,
            traverse_git_repos: self.traverse_git_repos,
        }
    }
}

impl Cli {
    /// ~keep Handle the init command
    pub fn handle_init_command(
        output: &PathBuf,
        force: bool,
        comprehensive: bool,
        interactive: bool,
    ) -> anyhow::Result<()> {
        if output.exists() && !force {
            return Err(anyhow::anyhow!(
                "Configuration file already exists: {}. Use --force to overwrite.",
                output.display()
            ));
        }

        let (template, detected_info) = if comprehensive {
            (crate::config::Config::comprehensive_template_clean(), None)
        } else if interactive {
            (crate::config::Config::interactive_template_clean()?, None)
        } else {
            // Smart template based on detected files in current directory
            let current_dir =
                std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let (template, info) = crate::config::Config::smart_template_with_info(&current_dir)?;
            (template, Some(info))
        };

        std::fs::write(output, template)?;

        println!("✓ Created configuration file: {}", output.display());

        if comprehensive {
            println!("📦 Generated comprehensive config with 15+ language configurations");
        } else if interactive {
            println!("🎯 Generated customized config based on your selections");
        } else if let Some(info) = detected_info {
            if !info.detected_languages.is_empty() {
                println!(
                    "🔍 Detected {} file types in your project:",
                    info.detected_languages.len()
                );
                for (lang, count) in &info.detected_languages {
                    println!("   {count} ({lang} files)");
                }
                println!(
                    "📝 Configured {} languages with appropriate settings",
                    info.configured_languages
                );
            } else {
                println!("📝 No supported files detected, generated basic template");
            }
            if info.total_files > 0 {
                println!("📊 Scanned {} files total", info.total_files);
            }
        } else {
            println!("📝 Generated smart config based on detected files in your project");
        }

        Ok(())
    }
}
