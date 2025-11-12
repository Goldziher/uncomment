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

    pub fn new_with_config(config_manager: &ConfigManager) -> Self {
        let mut registry = LanguageRegistry::new();

        let all_languages = config_manager.get_all_languages();
        registry.register_configured_languages(&all_languages);

        Self {
            parser: Parser::new(),
            registry,
            grammar_manager: GrammarManager::new().expect("Failed to initialize GrammarManager"),
        }
    }

    pub fn process_file_with_config(
        &mut self,
        path: &Path,
        config_manager: &ConfigManager,
        cli_overrides: Option<&ProcessingOptions>,
    ) -> Result<ProcessedFile> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;

        let language_config = self
            .registry
            .detect_language(path)
            .with_context(|| format!("Unsupported file type: {}", path.display()))?
            .clone();

        let language_name = language_config.name.to_lowercase();

        let mut resolved_config =
            config_manager.get_config_for_file_with_language(path, &language_name);

        if let Some(overrides) = cli_overrides {
            if overrides.remove_doc {
                resolved_config.remove_docs = true;
            }
            if overrides.remove_todo {
                resolved_config.remove_todos = true;
            }
            if overrides.remove_fixme {
                resolved_config.remove_fixme = true;
            }
            if !overrides.custom_preserve_patterns.is_empty() {
                resolved_config
                    .preserve_patterns
                    .extend(overrides.custom_preserve_patterns.clone());
            }
            resolved_config.respect_gitignore = overrides.respect_gitignore;
            resolved_config.traverse_git_repos = overrides.traverse_git_repos;
        }

        let (processed_content, comments_removed) =
            self.process_content_with_config(&content, &language_config, &resolved_config)?;

        Ok(ProcessedFile {
            path: path.to_path_buf(),
            original_content: content,
            processed_content,
            modified: false,
            comments_removed,
        })
    }

    fn process_content_with_config(
        &mut self,
        content: &str,
        language_config: &crate::languages::config::LanguageConfig,
        resolved_config: &ResolvedConfig,
    ) -> Result<(String, usize)> {
        let language = if let Some(grammar_config) = &resolved_config.grammar_config {
            self.grammar_manager
                .get_language(&language_config.name, grammar_config)
                .with_context(|| {
                    format!(
                        "Failed to load dynamic grammar for {}",
                        language_config.name
                    )
                })?
        } else {
            language_config.tree_sitter_language()
        };

        self.parser
            .set_language(&language)
            .context("Failed to set parser language")?;

        let tree = self
            .parser
            .parse(content, None)
            .context("Failed to parse source code")?;

        let preservation_rules = self.create_preservation_rules_from_config(resolved_config);

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

        if !config.remove_docs {
            rules.push(PreservationRule::documentation());
        }

        for pattern in &config.preserve_patterns {
            rules.push(PreservationRule::pattern(pattern));
        }

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

            if config.remove_docs {
                comprehensive_rules.retain(|rule| !matches!(rule, PreservationRule::Documentation));
            }

            rules.extend(comprehensive_rules);
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

        let mut bytes = content.as_bytes().to_vec();

        let mut deduped = comments_to_remove.to_vec();
        deduped.sort_by(|a, b| {
            a.start_byte
                .cmp(&b.start_byte)
                .then(b.end_byte.cmp(&a.end_byte))
        });

        let mut filtered: Vec<CommentInfo> = Vec::new();
        for comment in deduped {
            if let Some(previous) = filtered.last()
                && comment.start_byte >= previous.start_byte
                && comment.end_byte <= previous.end_byte
            {
                continue;
            }
            filtered.push(comment);
        }

        let mut sorted_comments = filtered;
        sorted_comments.sort_by(|a, b| b.start_byte.cmp(&a.start_byte));

        for comment in sorted_comments {
            if comment.start_byte >= bytes.len() {
                continue;
            }
            let start = comment.start_byte;
            let end = comment.end_byte.min(bytes.len());
            if start >= end {
                continue;
            }

            let mut line_start = start;
            while line_start > 0 && bytes[line_start - 1] != b'\n' {
                line_start -= 1;
            }

            let mut line_end = end;
            while line_end < bytes.len() && bytes[line_end] != b'\n' {
                line_end += 1;
            }
            if line_end < bytes.len() {
                line_end += 1;
            }

            let before = &bytes[line_start..start];
            let after = &bytes[end..line_end];
            let before_ws = before.iter().all(|b| b.is_ascii_whitespace());
            let after_ws = after.iter().all(|b| b.is_ascii_whitespace());

            if before_ws && after_ws {
                bytes.drain(line_start..line_end);
            } else {
                bytes.drain(start..end);
            }
        }

        String::from_utf8(bytes).unwrap_or_default()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ResolvedConfig;
    use crate::languages::config::LanguageConfig;

    fn default_resolved_config() -> ResolvedConfig {
        ResolvedConfig {
            remove_todos: false,
            remove_fixme: false,
            remove_docs: false,
            preserve_patterns: Vec::new(),
            use_default_ignores: true,
            respect_gitignore: true,
            traverse_git_repos: false,
            language_config: None,
            grammar_config: None,
        }
    }

    fn process_rust(source: &str) -> String {
        let mut processor = Processor::new();
        let language_config = LanguageConfig::rust();
        let resolved_config = default_resolved_config();
        let (output, _) = processor
            .process_content_with_config(source, &language_config, &resolved_config)
            .expect("processing rust source");
        output
    }

    #[test]
    fn preserves_strings_matching_comment_text() {
        let source = r#"fn main() {
    let pattern = "// comment";
    println!("{}", pattern); // comment
}
"#;

        let processed = process_rust(source);

        assert!(processed.contains("\"// comment\""));
        assert!(!processed.contains("; // comment"));
    }

    #[test]
    fn preserves_macro_invocations_with_comment_like_strings() {
        let source = r#"macro_rules! announce {
    ($msg:expr) => {{
        println!("{}", $msg); // keep
    }};
}

fn main() {
    announce!("// keep");
}
"#;

        let processed = process_rust(source);

        assert!(processed.contains("announce!(\"// keep\");"));
        assert!(!processed.contains("// keep\n"));
    }

    #[test]
    fn preserves_attributes_when_removing_doc_comments() {
        let source = r#"#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Create smart config
    #[command(about = "Create a template configuration file")]
    Init,
}
"#;

        let mut processor = Processor::new();
        let language_config = LanguageConfig::rust();
        let mut config = default_resolved_config();
        config.remove_docs = true;

        let (processed, _) = processor
            .process_content_with_config(source, &language_config, &config)
            .expect("process doc comments");

        assert!(processed.contains("#[command(about = \"Create a template configuration file\")]"));
        assert!(!processed.contains("Create smart config"));
    }
}
