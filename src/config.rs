use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// Global settings that apply to all files
    #[serde(default)]
    pub global: GlobalConfig,

    /// Language-specific configurations
    #[serde(default)]
    pub languages: HashMap<String, LanguageConfig>,

    /// Pattern-based rules (e.g., "tests/**/*.py")
    #[serde(default)]
    pub patterns: HashMap<String, PatternConfig>,
}

/// Global configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// Whether to remove TODO comments
    #[serde(default = "default_false")]
    pub remove_todos: bool,

    /// Whether to remove FIXME comments
    #[serde(default = "default_false")]
    pub remove_fixme: bool,

    /// Whether to remove documentation comments
    #[serde(default = "default_false")]
    pub remove_docs: bool,

    /// Additional patterns to preserve
    #[serde(default)]
    pub preserve_patterns: Vec<String>,

    /// Whether to use default ignore patterns
    #[serde(default = "default_true")]
    pub use_default_ignores: bool,

    /// Whether to respect .gitignore files
    #[serde(default = "default_true")]
    pub respect_gitignore: bool,

    /// Whether to traverse git repositories
    #[serde(default = "default_false")]
    pub traverse_git_repos: bool,
}

/// Language-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageConfig {
    /// Display name for the language
    pub name: String,

    /// File extensions for this language
    pub extensions: Vec<String>,

    /// Tree-sitter comment node types
    pub comment_nodes: Vec<String>,

    /// Tree-sitter documentation comment node types
    #[serde(default)]
    pub doc_comment_nodes: Vec<String>,

    /// Language-specific preserve patterns
    #[serde(default)]
    pub preserve_patterns: Vec<String>,

    /// Override global remove_todos setting
    pub remove_todos: Option<bool>,

    /// Override global remove_fixme setting
    pub remove_fixme: Option<bool>,

    /// Override global remove_docs setting
    pub remove_docs: Option<bool>,

    /// Override global use_default_ignores setting
    pub use_default_ignores: Option<bool>,

    /// Path to custom parser library (for dynamic loading)
    pub parser_path: Option<PathBuf>,

    /// Git repository for grammar (future use)
    pub grammar_repo: Option<String>,
}

/// Pattern-based configuration (e.g., for specific file patterns)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternConfig {
    /// Whether to remove TODO comments
    pub remove_todos: Option<bool>,

    /// Whether to remove FIXME comments
    pub remove_fixme: Option<bool>,

    /// Whether to remove documentation comments
    pub remove_docs: Option<bool>,

    /// Additional patterns to preserve
    #[serde(default)]
    pub preserve_patterns: Vec<String>,

    /// Whether to use default ignore patterns
    pub use_default_ignores: Option<bool>,
}

/// Resolved configuration for a specific file
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub remove_todos: bool,
    pub remove_fixme: bool,
    pub remove_docs: bool,
    pub preserve_patterns: Vec<String>,
    pub use_default_ignores: bool,
    pub respect_gitignore: bool,
    pub traverse_git_repos: bool,
    pub language_config: Option<LanguageConfig>,
}

/// Configuration manager that handles nested configs and path resolution
#[derive(Debug)]
pub struct ConfigManager {
    /// All discovered configuration files with their paths
    configs: Vec<(PathBuf, Config)>,

    /// Pre-computed configurations for all paths
    path_configs: HashMap<PathBuf, ResolvedConfig>,

    /// Root directory for path resolution
    root_dir: PathBuf,
}

// Default value helpers
fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            remove_todos: false,
            remove_fixme: false,
            remove_docs: false,
            preserve_patterns: Vec::new(),
            use_default_ignores: true,
            respect_gitignore: true,
            traverse_git_repos: false,
        }
    }
}

impl Config {
    /// Load configuration from a TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config file: {}", path.as_ref().display()))?;

        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.as_ref().display()))?;

        config
            .validate()
            .with_context(|| format!("Invalid configuration in: {}", path.as_ref().display()))?;

        Ok(config)
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        // Validate language configurations
        for (lang_name, lang_config) in &self.languages {
            if lang_config.name.is_empty() {
                return Err(anyhow::anyhow!("Language '{}' has empty name", lang_name));
            }

            if lang_config.extensions.is_empty() {
                return Err(anyhow::anyhow!(
                    "Language '{}' has no file extensions",
                    lang_name
                ));
            }

            if lang_config.comment_nodes.is_empty() {
                return Err(anyhow::anyhow!(
                    "Language '{}' has no comment node types",
                    lang_name
                ));
            }

            // Validate parser path if specified
            if let Some(ref parser_path) = lang_config.parser_path {
                if !parser_path.exists() {
                    return Err(anyhow::anyhow!(
                        "Parser path for language '{}' does not exist: {}",
                        lang_name,
                        parser_path.display()
                    ));
                }
            }
        }

        Ok(())
    }

    /// Create a template configuration
    pub fn template() -> String {
        r#"# Uncomment Configuration File
# https://github.com/Goldziher/uncomment

[global]
# Global settings that apply to all files
remove_todos = false        # Remove TODO comments
remove_fixme = false        # Remove FIXME comments
remove_docs = false         # Remove documentation comments
preserve_patterns = [       # Additional patterns to preserve
    "HACK",
    "WORKAROUND",
    "NOTE"
]
use_default_ignores = true  # Use built-in ignore patterns
respect_gitignore = true    # Respect .gitignore files
traverse_git_repos = false # Traverse into nested git repos

# Language-specific overrides (for built-in languages)
# These extend/override the built-in language configurations

# Override settings for Python files
[languages.python]
name = "Python"
extensions = [".py", ".pyw", ".pyi"]
comment_nodes = ["comment"]
preserve_patterns = ["mypy:", "type:", "noqa:", "pragma:"]
remove_docs = true  # Remove docstrings in Python

# Override settings for JavaScript files
[languages.javascript]
name = "JavaScript"
extensions = [".js", ".jsx", ".mjs", ".cjs"]
comment_nodes = ["comment"]
preserve_patterns = ["@ts-expect-error", "@ts-ignore", "webpack"]

# Override settings for TypeScript files
[languages.typescript]
name = "TypeScript"
extensions = [".ts", ".tsx", ".mts", ".cts", ".d.ts"]
comment_nodes = ["comment"]
preserve_patterns = ["@ts-expect-error", "@ts-ignore"]

# Custom language example (fully user-defined)
[languages.my_custom_lang]
name = "MyCustomLang"
extensions = [".mycl", ".custom"]
comment_nodes = ["comment", "line_comment"]
doc_comment_nodes = ["doc_comment"]
preserve_patterns = ["CUSTOM_TODO"]
# parser_path = "/path/to/libtree-sitter-mycustomlang.so"

# Pattern-based rules for specific file patterns
[patterns."tests/**/*.py"]
# Apply different rules to test files
remove_docs = true
remove_todos = true

[patterns."src/**/*.spec.ts"]
# Apply different rules to TypeScript test files
remove_docs = true
remove_todos = true

[patterns."**/*.generated.*"]
# Be more aggressive with generated files
remove_docs = true
remove_todos = true
preserve_patterns = []
"#
        .to_string()
    }

    /// Merge this config with another, giving precedence to the other config
    pub fn merge_with(&self, other: &Config) -> Config {
        let mut merged = self.clone();

        // Merge global settings (other takes precedence for non-default values)
        merged.global.remove_todos = other.global.remove_todos;
        merged.global.remove_fixme = other.global.remove_fixme;
        merged.global.remove_docs = other.global.remove_docs;
        merged.global.use_default_ignores = other.global.use_default_ignores;
        merged.global.respect_gitignore = other.global.respect_gitignore;
        merged.global.traverse_git_repos = other.global.traverse_git_repos;

        // Merge preserve patterns (combine both)
        let mut patterns = merged.global.preserve_patterns.clone();
        patterns.extend(other.global.preserve_patterns.clone());
        patterns.sort();
        patterns.dedup();
        merged.global.preserve_patterns = patterns;

        // Merge language configurations (other takes precedence)
        for (name, config) in &other.languages {
            merged.languages.insert(name.clone(), config.clone());
        }

        // Merge pattern configurations (other takes precedence)
        for (pattern, config) in &other.patterns {
            merged.patterns.insert(pattern.clone(), config.clone());
        }

        merged
    }
}

impl ConfigManager {
    /// Create a new ConfigManager by discovering all config files in the tree
    pub fn new<P: AsRef<Path>>(root_dir: P) -> Result<Self> {
        let root_dir = root_dir.as_ref().to_path_buf();
        let configs = Self::discover_configs(&root_dir)?;

        let mut manager = Self {
            configs,
            path_configs: HashMap::new(),
            root_dir,
        };

        manager.precompute_configs()?;
        Ok(manager)
    }

    /// Create a ConfigManager from a single config file
    pub fn from_single_config<P: AsRef<Path>>(root_dir: P, config: Config) -> Result<Self> {
        let root_dir = root_dir.as_ref().to_path_buf();
        let configs = vec![(root_dir.clone(), config)];

        let mut manager = Self {
            configs,
            path_configs: HashMap::new(),
            root_dir,
        };

        manager.precompute_configs()?;
        Ok(manager)
    }

    /// Discover all configuration files in the directory tree
    fn discover_configs(root_dir: &Path) -> Result<Vec<(PathBuf, Config)>> {
        let mut configs = Vec::new();

        // Walk the directory tree looking for config files
        for entry in walkdir::WalkDir::new(root_dir) {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                if matches!(file_name, ".uncommentrc.toml" | "uncomment.toml") {
                    match Config::from_file(path) {
                        Ok(config) => {
                            configs.push((path.to_path_buf(), config));
                        }
                        Err(e) => {
                            eprintln!(
                                "Warning: Failed to load config file {}: {}",
                                path.display(),
                                e
                            );
                        }
                    }
                }
            }
        }

        // Also check global config
        if let Some(global_config_path) = Self::global_config_path() {
            if global_config_path.exists() {
                match Config::from_file(&global_config_path) {
                    Ok(config) => {
                        configs.push((global_config_path, config));
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to load global config: {}", e);
                    }
                }
            }
        }

        // Sort by path depth (deeper configs override shallower ones)
        configs.sort_by_key(|(path, _)| path.components().count());

        Ok(configs)
    }

    /// Get the global configuration file path
    fn global_config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join("uncomment").join("config.toml"))
    }

    /// Pre-compute resolved configurations for all possible paths
    fn precompute_configs(&mut self) -> Result<()> {
        // For each directory that could contain files, compute the resolved config
        let mut dirs_to_process = vec![self.root_dir.clone()];

        // Collect all directories from our walk
        for entry in walkdir::WalkDir::new(&self.root_dir) {
            let entry = entry?;
            if entry.path().is_dir() {
                dirs_to_process.push(entry.path().to_path_buf());
            }
        }

        for dir_path in dirs_to_process {
            let resolved = self.resolve_config_for_path(&dir_path);
            self.path_configs.insert(dir_path, resolved);
        }

        Ok(())
    }

    /// Resolve configuration for a specific path by merging all applicable configs
    fn resolve_config_for_path(&self, path: &Path) -> ResolvedConfig {
        let mut base_config = Config::default();

        // Start with global config if it exists
        if let Some((_, global_config)) = self
            .configs
            .iter()
            .find(|(config_path, _)| Self::global_config_path() == Some(config_path.clone()))
        {
            base_config = base_config.merge_with(global_config);
        }

        // Apply configs from root to the specific path (nearest wins)
        let mut current_path = path;
        let mut applicable_configs = Vec::new();

        loop {
            // Check if there's a config file in this directory
            for (config_path, config) in &self.configs {
                if let Some(config_dir) = config_path.parent() {
                    if config_dir == current_path {
                        applicable_configs.push(config);
                    }
                }
            }

            // Move up to parent directory
            if let Some(parent) = current_path.parent() {
                current_path = parent;
            } else {
                break;
            }
        }

        // Apply configs from outermost to innermost (innermost wins)
        applicable_configs.reverse();
        for config in applicable_configs {
            base_config = base_config.merge_with(config);
        }

        // Convert to resolved config
        ResolvedConfig {
            remove_todos: base_config.global.remove_todos,
            remove_fixme: base_config.global.remove_fixme,
            remove_docs: base_config.global.remove_docs,
            preserve_patterns: base_config.global.preserve_patterns,
            use_default_ignores: base_config.global.use_default_ignores,
            respect_gitignore: base_config.global.respect_gitignore,
            traverse_git_repos: base_config.global.traverse_git_repos,
            language_config: None, // Will be set when language is determined
        }
    }

    /// Get the resolved configuration for a specific file path
    pub fn get_config_for_file<P: AsRef<Path>>(&self, file_path: P) -> ResolvedConfig {
        let file_path = file_path.as_ref();
        let dir_path = file_path.parent().unwrap_or(file_path);

        self.path_configs
            .get(dir_path)
            .cloned()
            .unwrap_or_else(|| self.resolve_config_for_path(dir_path))
    }

    /// Get language configuration by name
    pub fn get_language_config(&self, language_name: &str) -> Option<LanguageConfig> {
        // Check all configs for this language (nearest config wins)
        for (_, config) in self.configs.iter().rev() {
            if let Some(lang_config) = config.languages.get(language_name) {
                return Some(lang_config.clone());
            }
        }
        None
    }

    /// Get all configured languages
    pub fn get_all_languages(&self) -> HashMap<String, LanguageConfig> {
        let mut languages = HashMap::new();

        // Merge all language configs (later ones override earlier ones)
        for (_, config) in &self.configs {
            for (name, lang_config) in &config.languages {
                languages.insert(name.clone(), lang_config.clone());
            }
        }

        languages
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_template() {
        let template = Config::template();
        assert!(template.contains("[global]"));
        assert!(template.contains("[languages.python]"));
        assert!(template.contains("[patterns."));
    }

    #[test]
    fn test_config_validation() {
        let mut config = Config::default();

        // Valid config should pass
        assert!(config.validate().is_ok());

        // Invalid language config should fail
        config.languages.insert(
            "test".to_string(),
            LanguageConfig {
                name: "".to_string(), // Empty name should fail
                extensions: vec![".test".to_string()],
                comment_nodes: vec!["comment".to_string()],
                doc_comment_nodes: vec![],
                preserve_patterns: vec![],
                remove_todos: None,
                remove_fixme: None,
                remove_docs: None,
                use_default_ignores: None,
                parser_path: None,
                grammar_repo: None,
            },
        );

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_merging() {
        let base = Config {
            global: GlobalConfig {
                remove_todos: false,
                preserve_patterns: vec!["TODO".to_string()],
                ..Default::default()
            },
            ..Default::default()
        };

        let override_config = Config {
            global: GlobalConfig {
                remove_todos: true,
                preserve_patterns: vec!["FIXME".to_string()],
                ..Default::default()
            },
            ..Default::default()
        };

        let merged = base.merge_with(&override_config);
        assert!(merged.global.remove_todos);
        assert_eq!(merged.global.preserve_patterns, vec!["FIXME", "TODO"]);
    }
}
