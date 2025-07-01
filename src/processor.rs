use crate::ast::visitor::{CommentInfo, CommentVisitor};
use crate::config::{ConfigManager, ResolvedConfig};
use crate::grammar::GrammarManager;
use crate::languages::registry::LanguageRegistry;
use crate::rules::preservation::PreservationRule;
use anyhow::{Context, Result};
use std::path::Path;
use tree_sitter::Parser;

#[derive(Debug, Clone)]
pub struct ProcessingOptions {
    pub remove_todo: bool,
    pub remove_fixme: bool,
    pub remove_doc: bool,
    pub custom_preserve_patterns: Vec<String>,
    pub use_default_ignores: bool,
    pub dry_run: bool,
    pub respect_gitignore: bool,
    pub traverse_git_repos: bool,
}

pub struct Processor {
    parser: Parser,
    registry: LanguageRegistry,
    grammar_manager: GrammarManager,
}

impl Default for Processor {
    fn default() -> Self {
        Self::new()
    }
}

impl Processor {
    pub fn new() -> Self {
        Self {
            parser: Parser::new(),
            registry: LanguageRegistry::new(),
            grammar_manager: GrammarManager::new().expect("Failed to initialize GrammarManager"),
        }
    }

    /// Create a new processor with configured languages from a configuration manager
    pub fn new_with_config(config_manager: &ConfigManager) -> Self {
        let mut registry = LanguageRegistry::new();

        // Register configured languages
        let all_languages = config_manager.get_all_languages();
        registry.register_configured_languages(&all_languages);

        Self {
            parser: Parser::new(),
            registry,
            grammar_manager: GrammarManager::new().expect("Failed to initialize GrammarManager"),
        }
    }

    /// Process a single file with configuration manager
    pub fn process_file_with_config(
        &mut self,
        path: &Path,
        config_manager: &ConfigManager,
    ) -> Result<ProcessedFile> {
        // Read file content
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;

        // Get language configuration first
        let language_config = self
            .registry
            .detect_language(path)
            .with_context(|| format!("Unsupported file type: {}", path.display()))?
            .clone();

        // Get the language name for configuration lookup
        let language_name = language_config.name.to_lowercase();

        // Get resolved configuration for this file with language-specific overrides
        let resolved_config =
            config_manager.get_config_for_file_with_language(path, &language_name);

        // Process the content
        let (processed_content, comments_removed) =
            self.process_content_with_config(&content, &language_config, &resolved_config)?;

        Ok(ProcessedFile {
            path: path.to_path_buf(),
            original_content: content,
            processed_content,
            modified: false, // Will be set by the caller after comparison
            comments_removed,
        })
    }

    /// Process a single file (legacy method for backward compatibility)
    pub fn process_file(
        &mut self,
        path: &Path,
        options: &ProcessingOptions,
    ) -> Result<ProcessedFile> {
        // Read file content
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;

        // Get language configuration
        let language_config = self
            .registry
            .detect_language(path)
            .with_context(|| format!("Unsupported file type: {}", path.display()))?
            .clone();

        // Process the content
        let (processed_content, comments_removed) =
            self.process_content(&content, &language_config, options)?;

        Ok(ProcessedFile {
            path: path.to_path_buf(),
            original_content: content,
            processed_content,
            modified: false, // Will be set by the caller after comparison
            comments_removed,
        })
    }

    /// Process file content with resolved configuration
    fn process_content_with_config(
        &mut self,
        content: &str,
        language_config: &crate::languages::config::LanguageConfig,
        resolved_config: &ResolvedConfig,
    ) -> Result<(String, usize)> {
        // Determine which language to use - dynamic or static
        let language = if let Some(grammar_config) = &resolved_config.grammar_config {
            // Use dynamic grammar loading
            self.grammar_manager
                .get_language(&language_config.name, grammar_config)
                .with_context(|| {
                    format!(
                        "Failed to load dynamic grammar for {}",
                        language_config.name
                    )
                })?
        } else {
            // Use static built-in language
            language_config.tree_sitter_language()
        };

        self.parser
            .set_language(&language)
            .context("Failed to set parser language")?;

        // Parse the content
        let tree = self
            .parser
            .parse(content, None)
            .context("Failed to parse source code")?;

        // Create preservation rules based on resolved config
        let preservation_rules = self.create_preservation_rules_from_config(resolved_config);

        // Collect comments using the language-aware visitor
        let mut visitor = CommentVisitor::new_with_language(
            content,
            &preservation_rules,
            language_config.comment_types.clone(),
            language_config.doc_comment_types.clone(),
            language_config.name.clone(),
        );
        visitor.visit_node(tree.root_node());

        let comments_to_remove = visitor.get_comments_to_remove();
        let comments_removed = comments_to_remove.len();

        // Generate output by removing comments
        let output = self.remove_comments_from_content(content, &comments_to_remove);

        Ok((output, comments_removed))
    }

    /// Process file content and return (processed_content, comments_removed_count)
    fn process_content(
        &mut self,
        content: &str,
        language_config: &crate::languages::config::LanguageConfig,
        options: &ProcessingOptions,
    ) -> Result<(String, usize)> {
        // Set the parser language
        let language = language_config.tree_sitter_language();
        self.parser
            .set_language(&language)
            .context("Failed to set parser language")?;

        // Parse the content
        let tree = self
            .parser
            .parse(content, None)
            .context("Failed to parse source code")?;

        // Create preservation rules based on options
        let preservation_rules = self.create_preservation_rules(options);

        // Collect comments using the language-aware visitor
        let mut visitor = CommentVisitor::new_with_language(
            content,
            &preservation_rules,
            language_config.comment_types.clone(),
            language_config.doc_comment_types.clone(),
            language_config.name.clone(),
        );
        visitor.visit_node(tree.root_node());

        let comments_to_remove = visitor.get_comments_to_remove();
        let comments_removed = comments_to_remove.len();

        // Generate output by removing comments
        let output = self.remove_comments_from_content(content, &comments_to_remove);

        Ok((output, comments_removed))
    }

    fn create_preservation_rules_from_config(
        &self,
        config: &ResolvedConfig,
    ) -> Vec<PreservationRule> {
        let mut rules = Vec::new();

        // Always preserve ~keep
        rules.push(PreservationRule::pattern("~keep"));

        // Preserve TODO/FIXME unless explicitly removed
        if !config.remove_todos {
            rules.push(PreservationRule::pattern("TODO"));
            rules.push(PreservationRule::pattern("todo"));
        }
        if !config.remove_fixme {
            rules.push(PreservationRule::pattern("FIXME"));
            rules.push(PreservationRule::pattern("fixme"));
        }

        // Preserve documentation unless explicitly removed
        if !config.remove_docs {
            rules.push(PreservationRule::documentation());
        }

        // Add configured patterns
        for pattern in &config.preserve_patterns {
            rules.push(PreservationRule::pattern(pattern));
        }

        // Add default ignores if enabled
        if config.use_default_ignores {
            let mut comprehensive_rules = PreservationRule::comprehensive_rules();

            // Remove TODO/FIXME rules if they should be removed according to config
            if config.remove_todos {
                comprehensive_rules
                    .retain(|rule| !rule.pattern_matches("TODO") && !rule.pattern_matches("todo"));
            }
            if config.remove_fixme {
                comprehensive_rules.retain(|rule| {
                    !rule.pattern_matches("FIXME") && !rule.pattern_matches("fixme")
                });
            }

            // Remove documentation rules if docs should be removed
            if config.remove_docs {
                comprehensive_rules.retain(|rule| !matches!(rule, PreservationRule::Documentation));
            }

            rules.extend(comprehensive_rules);
        }

        rules
    }

    fn create_preservation_rules(&self, options: &ProcessingOptions) -> Vec<PreservationRule> {
        let mut rules = Vec::new();

        // Always preserve ~keep
        rules.push(PreservationRule::pattern("~keep"));

        // Preserve TODO/FIXME unless explicitly removed
        if !options.remove_todo {
            rules.push(PreservationRule::pattern("TODO"));
            rules.push(PreservationRule::pattern("todo"));
        }
        if !options.remove_fixme {
            rules.push(PreservationRule::pattern("FIXME"));
            rules.push(PreservationRule::pattern("fixme"));
        }

        // Preserve documentation unless explicitly removed
        if !options.remove_doc {
            rules.push(PreservationRule::documentation());
        }

        // Add custom patterns
        for pattern in &options.custom_preserve_patterns {
            rules.push(PreservationRule::pattern(pattern));
        }

        // Add default ignores if enabled
        if options.use_default_ignores {
            rules.extend(PreservationRule::comprehensive_rules());
        }

        rules
    }

    fn remove_comments_from_content(
        &self,
        content: &str,
        comments_to_remove: &[CommentInfo],
    ) -> String {
        if comments_to_remove.is_empty() {
            return content.to_string();
        }

        // Convert byte positions to char positions once, before any modifications
        let char_positions: Vec<usize> = content
            .char_indices()
            .map(|(byte_pos, _)| byte_pos)
            .collect();
        let total_chars = content.chars().count();

        // Helper function to convert byte position to char position
        let byte_to_char = |byte_pos: usize| -> usize {
            match char_positions.binary_search(&byte_pos) {
                Ok(char_pos) => char_pos,
                Err(char_pos) => char_pos.min(total_chars),
            }
        };

        let mut chars: Vec<char> = content.chars().collect();

        // Sort comments by start position in reverse order to avoid offset issues
        let mut sorted_comments = comments_to_remove.to_vec();
        sorted_comments.sort_by(|a, b| b.start_byte.cmp(&a.start_byte));

        for comment in sorted_comments {
            let start_char = byte_to_char(comment.start_byte);
            let end_char = byte_to_char(comment.end_byte);

            // Handle inline vs standalone comments
            if self.is_inline_comment(content, &comment) {
                // For inline comments, just remove the comment part
                if start_char < chars.len() && end_char <= chars.len() && start_char <= end_char {
                    chars.drain(start_char..end_char);
                }
            } else {
                // For standalone comments, remove the entire line
                let line_start = self.find_line_start(content, comment.start_byte);
                let line_end = self.find_line_end(content, comment.end_byte);

                let line_start_char = byte_to_char(line_start);
                let line_end_char = byte_to_char(line_end);

                if line_start_char < chars.len()
                    && line_end_char <= chars.len()
                    && line_start_char <= line_end_char
                {
                    chars.drain(line_start_char..line_end_char);
                }
            }
        }

        chars.into_iter().collect()
    }

    fn is_inline_comment(&self, content: &str, comment: &CommentInfo) -> bool {
        let lines: Vec<&str> = content.lines().collect();
        if comment.start_row < lines.len() {
            let line = lines[comment.start_row];
            let before_comment = &line[..comment.start_byte.min(line.len())];
            !before_comment.trim().is_empty()
        } else {
            false
        }
    }

    fn find_line_start(&self, content: &str, byte_pos: usize) -> usize {
        content[..byte_pos]
            .rfind('\n')
            .map(|pos| pos + 1)
            .unwrap_or(0)
    }

    fn find_line_end(&self, content: &str, byte_pos: usize) -> usize {
        content[byte_pos..]
            .find('\n')
            .map(|pos| byte_pos + pos + 1)
            .unwrap_or(content.len())
    }
}

#[derive(Debug)]
pub struct ProcessedFile {
    pub path: std::path::PathBuf,
    pub original_content: String,
    pub processed_content: String,
    pub modified: bool,
    pub comments_removed: usize,
}

// Simple output writer
pub struct OutputWriter {
    dry_run: bool,
    verbose: bool,
}

impl OutputWriter {
    pub fn new(dry_run: bool, verbose: bool) -> Self {
        Self { dry_run, verbose }
    }

    pub fn write_file(&self, processed_file: &ProcessedFile) -> Result<()> {
        let modified = processed_file.original_content != processed_file.processed_content;

        if !modified {
            if self.verbose {
                println!("✓ No changes needed: {}", processed_file.path.display());
            }
            return Ok(());
        }

        if self.dry_run {
            println!("[DRY RUN] Would modify: {}", processed_file.path.display());
            if self.verbose {
                println!("  Removed {} comment(s)", processed_file.comments_removed);
            }
            self.show_diff(processed_file)?;
        } else {
            std::fs::write(&processed_file.path, &processed_file.processed_content).with_context(
                || format!("Failed to write file: {}", processed_file.path.display()),
            )?;

            if self.verbose {
                println!(
                    "✓ Modified: {} (removed {} comment(s))",
                    processed_file.path.display(),
                    processed_file.comments_removed
                );
            } else {
                println!("Modified: {}", processed_file.path.display());
            }
        }

        Ok(())
    }

    fn show_diff(&self, processed_file: &ProcessedFile) -> Result<()> {
        println!("\n--- {}", processed_file.path.display());
        println!("+++ {} (processed)", processed_file.path.display());

        let original_lines: Vec<&str> = processed_file.original_content.lines().collect();
        let processed_lines: Vec<&str> = processed_file.processed_content.lines().collect();

        let max_lines = original_lines.len().max(processed_lines.len());

        for i in 0..max_lines {
            let original_line = original_lines.get(i).copied().unwrap_or("");
            let processed_line = processed_lines.get(i).copied().unwrap_or("");

            if original_line != processed_line {
                if i < original_lines.len() && i >= processed_lines.len() {
                    println!("-{original_line}");
                } else if i >= original_lines.len() && i < processed_lines.len() {
                    println!("+{processed_line}");
                } else if original_line != processed_line {
                    println!("-{original_line}");
                    println!("+{processed_line}");
                }
            }
        }

        Ok(())
    }

    pub fn print_summary(&self, total_files: usize, modified_files: usize) {
        if self.dry_run {
            println!(
                "\n[DRY RUN] Summary: {total_files} files processed, {modified_files} would be modified"
            );
        } else {
            println!("\nSummary: {total_files} files processed, {modified_files} modified");
        }

        if total_files > 0 && modified_files == 0 {
            println!("All files were already comment-free or only contained preserved comments.");
        }
    }
}
