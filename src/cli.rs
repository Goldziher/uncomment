use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Usage examples and preservation notes shown under `--help`.
const AFTER_LONG_HELP: &str = "Examples:
  uncomment src/                     Remove comments from every file under src/
  uncomment src/ --dry-run --diff    Preview changes as a diff, write nothing
  uncomment main.rs --remove-doc     Also strip doc comments and docstrings
  uncomment . -j 0                   Process the whole tree using all CPU cores
  uncomment init                     Generate a .uncommentrc.toml for this project

Preserved by default: TODO, FIXME, HACK, XXX, NOSONAR, the ~keep marker, doc
comments, and linting directives (eslint-disable, clippy::, noqa, ...). Override
with the flags above or a .uncommentrc.toml (see `uncomment init`).";

#[derive(Parser, Debug)]
#[command(
    name = "uncomment",
    version,
    about = "Strip comments from source code — accurately, via tree-sitter.",
    long_about = "uncomment removes comments from source code using tree-sitter AST parsing, so it \
                  is 100% accurate and never touches comment-like text inside strings. It preserves \
                  what matters by default — TODO/FIXME, docs, and linting directives — across 300+ \
                  languages, with parallel processing and a safe dry-run mode.",
    styles = crate::ui::clap_styles(),
    after_long_help = AFTER_LONG_HELP
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
        #[arg(short, long, value_name = "FILE", default_value = ".uncommentrc.toml")]
        output: PathBuf,

        /// ~keep Overwrite existing file
        #[arg(short, long)]
        force: bool,

        /// ~keep Generate configuration for all supported languages
        #[arg(long, help = "Generate comprehensive config with all supported languages")]
        comprehensive: bool,

        /// ~keep Interactive mode to select languages
        #[arg(short, long, help = "Interactive mode to select languages and options")]
        interactive: bool,
    },
}

#[derive(Parser, Debug)]
pub struct ProcessArgs {
    /// ~keep Files or directories to process (supports glob patterns)
    #[arg(value_name = "PATH", help = "Files, directories, or glob patterns to process")]
    pub paths: Vec<String>,

    /// ~keep Remove TODO comments (normally preserved)
    #[arg(
        short = 'r',
        long,
        help = "Remove TODO comments (normally preserved)",
        help_heading = "Comment selection"
    )]
    pub remove_todo: bool,

    /// ~keep Remove FIXME comments (normally preserved)
    #[arg(
        short = 'f',
        long,
        help = "Remove FIXME comments (normally preserved)",
        help_heading = "Comment selection"
    )]
    pub remove_fixme: bool,

    /// ~keep Remove documentation comments (normally preserved)
    #[arg(
        short = 'd',
        long,
        help = "Remove documentation comments and docstrings",
        help_heading = "Comment selection"
    )]
    pub remove_doc: bool,

    /// ~keep Additional patterns to preserve (beyond defaults)
    #[arg(
        short = 'i',
        long = "ignore",
        value_name = "PATTERN",
        help = "Additional patterns to preserve (can be used multiple times)",
        help_heading = "Comment selection"
    )]
    pub ignore_patterns: Vec<String>,

    /// ~keep Disable automatic preservation of linting directives
    #[arg(
        long = "no-default-ignores",
        help = "Disable built-in preservation patterns (ESLint, Clippy, etc.)",
        help_heading = "Comment selection"
    )]
    pub no_default_ignores: bool,

    /// ~keep Show what would be changed without modifying files
    #[arg(
        short = 'n',
        long,
        help = "Show changes without modifying files",
        help_heading = "Output"
    )]
    pub dry_run: bool,

    /// ~keep Show line-by-line diffs in dry-run mode
    #[arg(
        long = "diff",
        help = "Show line-by-line diffs for modified files (only useful with --dry-run)",
        help_heading = "Output"
    )]
    pub diff: bool,

    /// ~keep Show detailed processing information
    #[arg(
        short = 'v',
        long,
        help = "Show detailed processing information",
        help_heading = "Output"
    )]
    pub verbose: bool,

    /// ~keep Ignore .gitignore rules when finding files
    #[arg(
        long = "no-gitignore",
        help = "Process files ignored by .gitignore",
        help_heading = "File selection"
    )]
    pub no_gitignore: bool,

    /// ~keep Process files in nested git repositories
    #[arg(
        long = "traverse-git-repos",
        help = "Traverse into other git repositories (useful for monorepos)",
        help_heading = "File selection"
    )]
    pub traverse_git_repos: bool,

    /// ~keep Number of parallel threads (0 = number of CPU cores)
    #[arg(
        short = 'j',
        long = "threads",
        value_name = "N",
        help = "Number of parallel threads (0 = auto-detect)",
        default_value = "1",
        help_heading = "Performance"
    )]
    pub threads: usize,

    /// ~keep Path to configuration file
    #[arg(
        short = 'c',
        long = "config",
        value_name = "FILE",
        help = "Path to configuration file (overrides automatic discovery)",
        help_heading = "File selection"
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
            show_diff: self.diff,
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
            let current_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let (template, info) = crate::config::Config::smart_template_with_info(&current_dir)?;
            (template, Some(info))
        };

        std::fs::write(output, template)?;

        use crate::ui;
        anstream::println!(
            "{} {} {}",
            ui::success(ui::CHECK),
            ui::success("Created configuration file:"),
            ui::path(output)
        );

        if comprehensive {
            anstream::println!(
                "{} Generated comprehensive config with 15+ language configurations",
                ui::dim(ui::BULLET)
            );
        } else if interactive {
            anstream::println!(
                "{} Generated customized config based on your selections",
                ui::dim(ui::BULLET)
            );
        } else if let Some(info) = detected_info {
            if !info.detected_languages.is_empty() {
                anstream::println!(
                    "{} Detected {} file types in your project:",
                    ui::dim(ui::BULLET),
                    ui::accent(info.detected_languages.len())
                );
                for (lang, count) in &info.detected_languages {
                    anstream::println!("    {}", ui::dim(format!("{count} ({lang} files)")));
                }
                anstream::println!(
                    "{} Configured {} languages with appropriate settings",
                    ui::dim(ui::BULLET),
                    ui::accent(info.configured_languages)
                );
            } else {
                anstream::println!(
                    "{} No supported files detected, generated basic template",
                    ui::dim(ui::BULLET)
                );
            }
            if info.total_files > 0 {
                anstream::println!(
                    "{} Scanned {} files total",
                    ui::dim(ui::BULLET),
                    ui::accent(info.total_files)
                );
            }
        } else {
            anstream::println!(
                "{} Generated smart config based on detected files in your project",
                ui::dim(ui::BULLET)
            );
        }

        Ok(())
    }
}
