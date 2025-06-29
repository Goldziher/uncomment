use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Helper function to prompt for boolean input
fn prompt_bool(prompt: &str, default: bool) -> Result<bool> {
    use std::io::{self, Write};

    print!("{} [{}]: ", prompt, if default { "Y/n" } else { "y/N" });
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    Ok(match input.as_str() {
        "y" | "yes" => true,
        "n" | "no" => false,
        "" => default, // Use default if user just presses enter
        _ => default,  // Use default for invalid input
    })
}

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

/// Tree-sitter grammar configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GrammarConfig {
    /// Grammar source type
    #[serde(default)]
    pub source: GrammarSource,

    /// Version or commit hash (for git sources)
    pub version: Option<String>,

    /// Local path to compiled grammar library
    pub library_path: Option<PathBuf>,

    /// Custom compilation flags
    #[serde(default)]
    pub compile_flags: Vec<String>,
}

/// Grammar source specification
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum GrammarSource {
    /// Use built-in static grammar (default)
    #[default]
    Builtin,

    /// Load from Git repository
    Git {
        /// Repository URL
        url: String,
        /// Branch or tag (defaults to main/master)
        branch: Option<String>,
        /// Subdirectory containing grammar.js (if not root)
        path: Option<String>,
    },

    /// Load from local directory
    Local {
        /// Path to directory containing grammar.js
        path: PathBuf,
    },

    /// Load pre-compiled library
    Library {
        /// Path to compiled .so/.dll/.dylib file
        path: PathBuf,
    },
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

    /// Tree-sitter grammar configuration
    #[serde(default)]
    pub grammar: GrammarConfig,
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
    pub grammar_config: Option<GrammarConfig>,
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

# Example: Add Ruby support (not included in builtins)
[languages.ruby]
name = "Ruby"
extensions = ["rb", "rbw", "gemspec", "rake"]
comment_nodes = ["comment"]
preserve_patterns = ["rubocop:", "frozen_string_literal:"]

[languages.ruby.grammar]
source = { type = "git", url = "https://github.com/tree-sitter/tree-sitter-ruby", branch = "master" }

# Example: Add Vue.js support
[languages.vue]
name = "Vue"
extensions = ["vue"]
comment_nodes = ["comment"]
preserve_patterns = ["eslint-", "@ts-", "prettier-ignore"]

[languages.vue.grammar]
source = { type = "git", url = "https://github.com/tree-sitter-grammars/tree-sitter-vue", branch = "main" }

# Example: Add Swift support
[languages.swift]
name = "Swift"
extensions = ["swift"]
comment_nodes = ["comment", "multiline_comment"]
preserve_patterns = ["MARK:", "TODO:", "FIXME:", "swiftlint:"]

[languages.swift.grammar]
source = { type = "git", url = "https://github.com/alex-pinkus/tree-sitter-swift", branch = "main" }

# Example: Use a local grammar directory
# [languages.custom]
# name = "Custom Language"
# extensions = ["cst"]
# comment_nodes = ["comment"]
#
# [languages.custom.grammar]
# source = { type = "local", path = "/path/to/grammar-dir" }

# Example: Use a pre-compiled grammar library
# [languages.proprietary]
# name = "Proprietary"
# extensions = ["prop"]
# comment_nodes = ["comment"]
#
# [languages.proprietary.grammar]
# source = { type = "library", path = "/usr/local/lib/libtree-sitter-proprietary.so" }

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

    /// Create a comprehensive configuration template with many supported languages
    pub fn comprehensive_template() -> String {
        r#"# Comprehensive Uncomment Configuration File
# Generated with all supported languages from tree-sitter-language-pack
# https://github.com/Goldziher/uncomment

[global]
# Global settings that apply to all files
remove_todos = false        # Remove TODO comments
remove_fixme = false        # Remove FIXME comments
remove_docs = false         # Remove documentation comments
preserve_patterns = [       # Additional patterns to preserve
    "HACK",
    "WORKAROUND",
    "NOTE",
    "XXX",
    "FIXME",
    "TODO"
]
use_default_ignores = true  # Use built-in ignore patterns
respect_gitignore = true    # Respect .gitignore files
traverse_git_repos = false # Traverse into nested git repos

# Language-specific configurations with custom grammars
# These languages use dynamic tree-sitter grammars from the internet

# Web Development Languages
[languages.vue]
name = "Vue"
extensions = ["vue"]
comment_nodes = ["comment"]
preserve_patterns = ["eslint-", "@ts-", "prettier-ignore"]

[languages.vue.grammar]
source = { type = "git", url = "https://github.com/tree-sitter-grammars/tree-sitter-vue", branch = "main" }

[languages.svelte]
name = "Svelte"
extensions = ["svelte"]
comment_nodes = ["comment"]
preserve_patterns = ["eslint-", "prettier-ignore"]

[languages.svelte.grammar]
source = { type = "git", url = "https://github.com/Himujjal/tree-sitter-svelte", branch = "master" }

[languages.astro]
name = "Astro"
extensions = ["astro"]
comment_nodes = ["comment"]
preserve_patterns = ["eslint-", "prettier-ignore"]

[languages.astro.grammar]
source = { type = "git", url = "https://github.com/virchau13/tree-sitter-astro", branch = "master" }

# Mobile Development
[languages.swift]
name = "Swift"
extensions = ["swift"]
comment_nodes = ["comment", "multiline_comment"]
preserve_patterns = ["MARK:", "TODO:", "FIXME:", "swiftlint:"]

[languages.swift.grammar]
source = { type = "git", url = "https://github.com/alex-pinkus/tree-sitter-swift", branch = "main" }

[languages.kotlin]
name = "Kotlin"
extensions = ["kt", "kts"]
comment_nodes = ["line_comment", "multiline_comment"]
preserve_patterns = ["@Suppress", "ktlint:"]

[languages.kotlin.grammar]
source = { type = "git", url = "https://github.com/fwcd/tree-sitter-kotlin" }

[languages.dart]
name = "Dart"
extensions = ["dart"]
comment_nodes = ["comment"]
preserve_patterns = ["ignore:", "ignore_for_file:"]

[languages.dart.grammar]
source = { type = "git", url = "https://github.com/UserNobody14/tree-sitter-dart", branch = "master" }

# Systems Programming
[languages.zig]
name = "Zig"
extensions = ["zig"]
comment_nodes = ["line_comment"]
preserve_patterns = ["zig fmt:"]

[languages.zig.grammar]
source = { type = "git", url = "https://github.com/maxxnino/tree-sitter-zig" }

[languages.nim]
name = "Nim"
extensions = ["nim", "nims"]
comment_nodes = ["comment"]
preserve_patterns = ["pragma:"]

[languages.nim.grammar]
source = { type = "git", url = "https://github.com/alaviss/tree-sitter-nim" }

# Functional Programming
[languages.haskell]
name = "Haskell"
extensions = ["hs", "lhs"]
comment_nodes = ["comment"]
preserve_patterns = ["LANGUAGE", "OPTIONS_GHC"]

[languages.haskell.grammar]
source = { type = "git", url = "https://github.com/tree-sitter/tree-sitter-haskell", branch = "master" }

[languages.elixir]
name = "Elixir"
extensions = ["ex", "exs"]
comment_nodes = ["comment"]
preserve_patterns = ["@doc", "@moduledoc"]

[languages.elixir.grammar]
source = { type = "git", url = "https://github.com/elixir-lang/tree-sitter-elixir" }

[languages.elm]
name = "Elm"
extensions = ["elm"]
comment_nodes = ["line_comment", "block_comment"]

[languages.elm.grammar]
source = { type = "git", url = "https://github.com/razzeee/tree-sitter-elm" }

[languages.clojure]
name = "Clojure"
extensions = ["clj", "cljs", "cljc", "edn"]
comment_nodes = ["comment"]

[languages.clojure.grammar]
source = { type = "git", url = "https://github.com/sogaiu/tree-sitter-clojure", branch = "master" }

# Data Science & ML
[languages.r]
name = "R"
extensions = ["r", "R"]
comment_nodes = ["comment"]
preserve_patterns = ["@param", "@return", "@export"]

[languages.r.grammar]
source = { type = "git", url = "https://github.com/r-lib/tree-sitter-r" }

[languages.julia]
name = "Julia"
extensions = ["jl"]
comment_nodes = ["comment"]
preserve_patterns = ["@doc", "@inline", "@noinline"]

[languages.julia.grammar]
source = { type = "git", url = "https://github.com/tree-sitter/tree-sitter-julia", branch = "master" }

# DevOps & Configuration
[languages.dockerfile]
name = "Dockerfile"
extensions = ["dockerfile"]
comment_nodes = ["comment"]

[languages.dockerfile.grammar]
source = { type = "git", url = "https://github.com/camdencheek/tree-sitter-dockerfile" }

[languages.nix]
name = "Nix"
extensions = ["nix"]
comment_nodes = ["comment"]

[languages.nix.grammar]
source = { type = "git", url = "https://github.com/nix-community/tree-sitter-nix", branch = "master" }

[languages.lua]
name = "Lua"
extensions = ["lua"]
comment_nodes = ["comment"]

[languages.lua.grammar]
source = { type = "git", url = "https://github.com/MunifTanjim/tree-sitter-lua" }

# Shell Scripting
[languages.fish]
name = "Fish"
extensions = ["fish"]
comment_nodes = ["comment"]

[languages.fish.grammar]
source = { type = "git", url = "https://github.com/ram02z/tree-sitter-fish", branch = "master" }

# Override built-in languages with custom settings
[languages.python]
name = "Python"
extensions = ["py", "pyw", "pyi"]
comment_nodes = ["comment"]
preserve_patterns = ["mypy:", "type:", "noqa:", "pragma:", "pylint:"]
remove_docs = false  # Keep docstrings by default

[languages.javascript]
name = "JavaScript"
extensions = ["js", "jsx", "mjs", "cjs"]
comment_nodes = ["comment"]
preserve_patterns = ["@ts-expect-error", "@ts-ignore", "webpack", "eslint-"]

[languages.typescript]
name = "TypeScript"
extensions = ["ts", "tsx", "mts", "cts", "d.ts"]
comment_nodes = ["comment"]
preserve_patterns = ["@ts-expect-error", "@ts-ignore", "eslint-"]

[languages.rust]
name = "Rust"
extensions = ["rs"]
comment_nodes = ["line_comment", "block_comment"]
doc_comment_nodes = ["doc_comment"]
preserve_patterns = ["clippy:", "allow", "deny", "warn"]
remove_docs = false  # Keep doc comments by default

# Pattern-based rules for different file types
[patterns."tests/**/*.py"]
# More aggressive with test files
remove_docs = true
remove_todos = true

[patterns."src/**/*.spec.ts"]
# TypeScript test files
remove_docs = true
remove_todos = true

[patterns."**/*.generated.*"]
# Be aggressive with generated files
remove_docs = true
remove_todos = true
preserve_patterns = []

[patterns."docs/**/*"]
# Preserve everything in documentation
remove_docs = false
remove_todos = false
remove_fixme = false
"#
        .to_string()
    }

    /// Create a smart configuration template based on detected files
    pub fn smart_template<P: AsRef<Path>>(project_dir: P) -> Result<String> {
        use walkdir::WalkDir;

        let mut detected_languages = HashMap::new();
        let mut file_count = 0;

        // Define supported extensions
        let supported_extensions = [
            "py", "pyw", "pyi", "pyx", "pxd", // Python
            "js", "jsx", "mjs", "cjs", // JavaScript
            "ts", "tsx", "mts", "cts",  // TypeScript
            "rs",   // Rust
            "go",   // Go
            "java", // Java
            "c", "h", // C
            "cpp", "cc", "cxx", "hpp", "hxx", "hh", // C++
            "rb", // Ruby
            "yml", "yaml", // YAML
            "hcl", "tf", "tfvars", // HCL/Terraform
            "vue", "svelte", "astro", // Web frameworks
            "swift", "kt", "kts", "dart", // Mobile
            "zig", "nim", "hs", "lhs", // Systems/functional
            "ex", "exs", "elm", "clj", "cljs", "cljc", "edn", // Functional
            "r", "jl", // Data science
            "nix", "lua", "fish", // Scripting/config
        ];

        // Walk the project directory to detect languages
        for entry in WalkDir::new(project_dir.as_ref())
            .max_depth(3) // Don't go too deep to avoid slow scanning
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                if let Some(ext) = entry.path().extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    // Only count supported extensions
                    if supported_extensions.contains(&ext_str.as_str()) {
                        *detected_languages.entry(ext_str).or_insert(0) += 1;
                        file_count += 1;
                    }
                }

                // Special handling for files without extensions
                if let Some(filename) = entry.path().file_name() {
                    let filename_str = filename.to_string_lossy().to_lowercase();
                    if filename_str == "dockerfile" {
                        *detected_languages
                            .entry("dockerfile".to_string())
                            .or_insert(0) += 1;
                        file_count += 1;
                    } else if filename_str == "makefile" || filename_str.ends_with(".mk") {
                        *detected_languages.entry("make".to_string()).or_insert(0) += 1;
                        file_count += 1;
                    }
                }
            }
        }

        if file_count == 0 {
            // No files detected, return basic template
            return Ok(Self::template());
        }

        // Generate config based on detected languages
        let mut config = String::from(
            r#"# Smart Uncomment Configuration
# Generated based on detected files in your project
# https://github.com/Goldziher/uncomment

[global]
remove_todos = false
remove_fixme = false
remove_docs = false
preserve_patterns = ["HACK", "WORKAROUND", "NOTE"]
use_default_ignores = true
respect_gitignore = true
traverse_git_repos = false

# Detected languages in your project:
"#,
        );

        // Language mapping from extensions to our configurations
        let language_configs = Self::get_language_mappings();

        for (ext, count) in &detected_languages {
            if *count > 0 {
                config.push_str(&format!("# Found {} {} files\n", count, ext));
            }
        }
        config.push('\n');

        // Add language configurations for detected languages
        for ext in detected_languages.keys() {
            if let Some(lang_config) = language_configs.get(ext) {
                config.push_str(lang_config);
                config.push('\n');
            }
        }

        // Add common pattern-based rules
        config.push_str(
            r#"
# Pattern-based rules
[patterns."tests/**/*"]
# More aggressive with test files
remove_todos = true

[patterns."**/*.spec.*"]
# Test specification files
remove_docs = true
remove_todos = true

[patterns."**/*.generated.*"]
# Generated files
remove_docs = true
remove_todos = true
preserve_patterns = []
"#,
        );

        Ok(config)
    }

    /// Create an interactive configuration template
    pub fn interactive_template() -> Result<String> {
        use std::io::{self, Write};

        println!("🚀 Welcome to Uncomment Interactive Configuration!");
        println!("I'll help you create a customized configuration file.\n");

        // Ask about global settings
        let remove_todos = prompt_bool("Remove TODO comments by default? (y/n)", false)?;
        let remove_fixme = prompt_bool("Remove FIXME comments by default? (y/n)", false)?;
        let remove_docs = prompt_bool("Remove documentation comments by default? (y/n)", false)?;

        println!("\n📋 Available languages with grammar support:");
        let available_languages = vec![
            ("vue", "Vue.js single-file components"),
            ("svelte", "Svelte components"),
            ("swift", "Swift (iOS/macOS development)"),
            ("kotlin", "Kotlin (Android/JVM development)"),
            ("dart", "Dart (Flutter development)"),
            ("zig", "Zig systems language"),
            ("haskell", "Haskell functional language"),
            ("elixir", "Elixir/Phoenix development"),
            ("r", "R statistical computing"),
            ("julia", "Julia scientific computing"),
            ("nix", "Nix package manager"),
            ("lua", "Lua scripting"),
        ];

        for (i, (name, desc)) in available_languages.iter().enumerate() {
            println!("  {}. {} - {}", i + 1, name, desc);
        }

        println!("\nSelect languages to include (comma-separated numbers, or 'all' for all, or 'skip' to skip):");
        print!("> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        let mut selected_languages = Vec::new();

        if input == "all" {
            selected_languages = available_languages.iter().map(|(name, _)| *name).collect();
        } else if input != "skip" {
            for num_str in input.split(',') {
                if let Ok(num) = num_str.trim().parse::<usize>() {
                    if num > 0 && num <= available_languages.len() {
                        selected_languages.push(available_languages[num - 1].0);
                    }
                }
            }
        }

        // Generate the configuration
        let mut config = format!(
            r#"# Interactive Uncomment Configuration
# Generated through interactive setup
# https://github.com/Goldziher/uncomment

[global]
remove_todos = {}
remove_fixme = {}
remove_docs = {}
preserve_patterns = ["HACK", "WORKAROUND", "NOTE"]
use_default_ignores = true
respect_gitignore = true
traverse_git_repos = false

"#,
            remove_todos, remove_fixme, remove_docs
        );

        // Add selected language configurations
        let language_configs = Self::get_extended_language_mappings();
        for lang in &selected_languages {
            if let Some(lang_config) = language_configs.get(*lang) {
                config.push_str(lang_config);
                config.push('\n');
            }
        }

        if !selected_languages.is_empty() {
            println!(
                "\n✅ Generated configuration with {} languages!",
                selected_languages.len()
            );
        }

        Ok(config)
    }

    /// Get language mappings for common extensions
    fn get_language_mappings() -> std::collections::HashMap<String, &'static str> {
        let mut map = std::collections::HashMap::new();

        map.insert(
            "py".to_string(),
            r#"[languages.python]
name = "Python"
extensions = ["py", "pyw", "pyi"]
comment_nodes = ["comment"]
preserve_patterns = ["mypy:", "type:", "noqa:", "pragma:", "pylint:"]
remove_docs = false"#,
        );

        map.insert(
            "js".to_string(),
            r#"[languages.javascript]
name = "JavaScript"
extensions = ["js", "jsx", "mjs", "cjs"]
comment_nodes = ["comment"]
preserve_patterns = ["@ts-expect-error", "@ts-ignore", "webpack", "eslint-"]"#,
        );

        map.insert(
            "ts".to_string(),
            r#"[languages.typescript]
name = "TypeScript"
extensions = ["ts", "tsx", "mts", "cts", "d.ts"]
comment_nodes = ["comment"]
preserve_patterns = ["@ts-expect-error", "@ts-ignore", "eslint-"]"#,
        );

        map.insert(
            "rs".to_string(),
            r#"[languages.rust]
name = "Rust"
extensions = ["rs"]
comment_nodes = ["line_comment", "block_comment"]
doc_comment_nodes = ["doc_comment"]
preserve_patterns = ["clippy:", "allow", "deny", "warn"]
remove_docs = false"#,
        );

        map.insert(
            "go".to_string(),
            r#"[languages.go]
name = "Go"
extensions = ["go"]
comment_nodes = ["comment"]
preserve_patterns = ["go:generate", "nolint"]"#,
        );

        map.insert(
            "java".to_string(),
            r#"[languages.java]
name = "Java"
extensions = ["java"]
comment_nodes = ["line_comment", "block_comment"]
doc_comment_nodes = ["doc_comment"]
preserve_patterns = ["@SuppressWarnings", "@Override"]
remove_docs = false"#,
        );

        map.insert("vue".to_string(), r#"[languages.vue]
name = "Vue"
extensions = ["vue"]
comment_nodes = ["comment"]
preserve_patterns = ["eslint-", "@ts-", "prettier-ignore"]

[languages.vue.grammar]
source = { type = "git", url = "https://github.com/tree-sitter-grammars/tree-sitter-vue", branch = "main" }"#);

        map.insert(
            "dockerfile".to_string(),
            r#"[languages.dockerfile]
name = "Dockerfile"
extensions = ["dockerfile"]
comment_nodes = ["comment"]

[languages.dockerfile.grammar]
source = { type = "git", url = "https://github.com/camdencheek/tree-sitter-dockerfile" }"#,
        );

        map.insert("swift".to_string(), r#"[languages.swift]
name = "Swift"
extensions = ["swift"]
comment_nodes = ["comment", "multiline_comment"]
preserve_patterns = ["MARK:", "TODO:", "FIXME:", "swiftlint:"]

[languages.swift.grammar]
source = { type = "git", url = "https://github.com/alex-pinkus/tree-sitter-swift", branch = "main" }"#);

        map
    }

    /// Get extended language mappings for interactive mode
    fn get_extended_language_mappings() -> std::collections::HashMap<&'static str, &'static str> {
        let mut map = std::collections::HashMap::new();

        map.insert("vue", r#"[languages.vue]
name = "Vue"
extensions = ["vue"]
comment_nodes = ["comment"]
preserve_patterns = ["eslint-", "@ts-", "prettier-ignore"]

[languages.vue.grammar]
source = { type = "git", url = "https://github.com/tree-sitter-grammars/tree-sitter-vue", branch = "main" }"#);

        map.insert("svelte", r#"[languages.svelte]
name = "Svelte"
extensions = ["svelte"]
comment_nodes = ["comment"]
preserve_patterns = ["eslint-", "prettier-ignore"]

[languages.svelte.grammar]
source = { type = "git", url = "https://github.com/Himujjal/tree-sitter-svelte", branch = "master" }"#);

        map.insert("swift", r#"[languages.swift]
name = "Swift"
extensions = ["swift"]
comment_nodes = ["comment", "multiline_comment"]
preserve_patterns = ["MARK:", "TODO:", "FIXME:", "swiftlint:"]

[languages.swift.grammar]
source = { type = "git", url = "https://github.com/alex-pinkus/tree-sitter-swift", branch = "main" }"#);

        map.insert(
            "kotlin",
            r#"[languages.kotlin]
name = "Kotlin"
extensions = ["kt", "kts"]
comment_nodes = ["line_comment", "multiline_comment"]
preserve_patterns = ["@Suppress", "ktlint:"]

[languages.kotlin.grammar]
source = { type = "git", url = "https://github.com/fwcd/tree-sitter-kotlin" }"#,
        );

        map.insert("dart", r#"[languages.dart]
name = "Dart"
extensions = ["dart"]
comment_nodes = ["comment"]
preserve_patterns = ["ignore:", "ignore_for_file:"]

[languages.dart.grammar]
source = { type = "git", url = "https://github.com/UserNobody14/tree-sitter-dart", branch = "master" }"#);

        map.insert(
            "zig",
            r#"[languages.zig]
name = "Zig"
extensions = ["zig"]
comment_nodes = ["line_comment"]
preserve_patterns = ["zig fmt:"]

[languages.zig.grammar]
source = { type = "git", url = "https://github.com/maxxnino/tree-sitter-zig" }"#,
        );

        map.insert("haskell", r#"[languages.haskell]
name = "Haskell"
extensions = ["hs", "lhs"]
comment_nodes = ["comment"]
preserve_patterns = ["LANGUAGE", "OPTIONS_GHC"]

[languages.haskell.grammar]
source = { type = "git", url = "https://github.com/tree-sitter/tree-sitter-haskell", branch = "master" }"#);

        map.insert(
            "elixir",
            r#"[languages.elixir]
name = "Elixir"
extensions = ["ex", "exs"]
comment_nodes = ["comment"]
preserve_patterns = ["@doc", "@moduledoc"]

[languages.elixir.grammar]
source = { type = "git", url = "https://github.com/elixir-lang/tree-sitter-elixir" }"#,
        );

        map.insert(
            "r",
            r#"[languages.r]
name = "R"
extensions = ["r", "R"]
comment_nodes = ["comment"]
preserve_patterns = ["@param", "@return", "@export"]

[languages.r.grammar]
source = { type = "git", url = "https://github.com/r-lib/tree-sitter-r" }"#,
        );

        map.insert("julia", r#"[languages.julia]
name = "Julia"
extensions = ["jl"]
comment_nodes = ["comment"]
preserve_patterns = ["@doc", "@inline", "@noinline"]

[languages.julia.grammar]
source = { type = "git", url = "https://github.com/tree-sitter/tree-sitter-julia", branch = "master" }"#);

        map.insert("nix", r#"[languages.nix]
name = "Nix"
extensions = ["nix"]
comment_nodes = ["comment"]

[languages.nix.grammar]
source = { type = "git", url = "https://github.com/nix-community/tree-sitter-nix", branch = "master" }"#);

        map.insert(
            "lua",
            r#"[languages.lua]
name = "Lua"
extensions = ["lua"]
comment_nodes = ["comment"]

[languages.lua.grammar]
source = { type = "git", url = "https://github.com/MunifTanjim/tree-sitter-lua" }"#,
        );

        map
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
                                "Warning: Failed to load config file {}: {e}",
                                path.display()
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
                        eprintln!("Warning: Failed to load global config: {e}");
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
            grammar_config: None,  // Will be set when language is determined
        }
    }

    /// Get the resolved configuration for a specific file path
    pub fn get_config_for_file<P: AsRef<Path>>(&self, file_path: P) -> ResolvedConfig {
        let file_path = file_path.as_ref();

        // Convert to absolute path if it's relative
        let absolute_file_path = if file_path.is_absolute() {
            file_path.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_default().join(file_path)
        };

        let dir_path = absolute_file_path.parent().unwrap_or(&absolute_file_path);

        self.path_configs
            .get(dir_path)
            .cloned()
            .unwrap_or_else(|| self.resolve_config_for_path(dir_path))
    }

    /// Get the resolved configuration for a specific file with language-specific overrides
    pub fn get_config_for_file_with_language<P: AsRef<Path>>(
        &self,
        file_path: P,
        language_name: &str,
    ) -> ResolvedConfig {
        let mut config = self.get_config_for_file(file_path);

        // Apply language-specific overrides
        if let Some(lang_config) = self.get_language_config(language_name) {
            // Override global settings with language-specific ones
            if let Some(remove_todos) = lang_config.remove_todos {
                config.remove_todos = remove_todos;
            }
            if let Some(remove_fixme) = lang_config.remove_fixme {
                config.remove_fixme = remove_fixme;
            }
            if let Some(remove_docs) = lang_config.remove_docs {
                config.remove_docs = remove_docs;
            }
            if let Some(use_default_ignores) = lang_config.use_default_ignores {
                config.use_default_ignores = use_default_ignores;
            }

            // Merge preserve patterns
            config
                .preserve_patterns
                .extend(lang_config.preserve_patterns.clone());
            config.preserve_patterns.sort();
            config.preserve_patterns.dedup();

            // Set the language config and grammar config
            config.grammar_config = Some(lang_config.grammar.clone());
            config.language_config = Some(lang_config);
        }

        config
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
                grammar: GrammarConfig::default(),
            },
        );

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_grammar_config_integration() {
        let mut config = Config::default();

        // Add a language with custom grammar configuration
        let language_config = LanguageConfig {
            name: "test_lang".to_string(),
            extensions: vec!["test".to_string()],
            comment_nodes: vec!["comment".to_string()],
            doc_comment_nodes: vec!["doc_comment".to_string()],
            preserve_patterns: vec![],
            remove_todos: None,
            remove_fixme: None,
            remove_docs: None,
            use_default_ignores: None,
            grammar: GrammarConfig {
                source: GrammarSource::Git {
                    url: "https://github.com/test/test-grammar".to_string(),
                    branch: Some("main".to_string()),
                    path: None,
                },
                version: Some("1.0.0".to_string()),
                library_path: None,
                compile_flags: vec!["--optimize".to_string()],
            },
        };

        config
            .languages
            .insert("test_lang".to_string(), language_config);

        // Validate the configuration
        assert!(config.validate().is_ok());

        // Test that we can access the grammar configuration
        let lang_config = config.languages.get("test_lang").unwrap();
        assert!(matches!(
            lang_config.grammar.source,
            GrammarSource::Git { .. }
        ));
        assert_eq!(lang_config.grammar.version, Some("1.0.0".to_string()));
        assert_eq!(lang_config.grammar.compile_flags, vec!["--optimize"]);
    }

    #[test]
    fn test_grammar_config_defaults() {
        let default_config = GrammarConfig::default();
        assert!(matches!(default_config.source, GrammarSource::Builtin));
        assert!(default_config.version.is_none());
        assert!(default_config.library_path.is_none());
        assert!(default_config.compile_flags.is_empty());
    }

    #[test]
    fn test_grammar_source_serialization() {
        // Test Git source
        let git_source = GrammarSource::Git {
            url: "https://github.com/test/grammar".to_string(),
            branch: Some("main".to_string()),
            path: Some("grammar".to_string()),
        };

        let serialized = toml::to_string(&git_source).unwrap();
        let deserialized: GrammarSource = toml::from_str(&serialized).unwrap();
        assert!(matches!(deserialized, GrammarSource::Git { .. }));

        // Test Local source
        let local_source = GrammarSource::Local {
            path: "/path/to/grammar".into(),
        };

        let serialized = toml::to_string(&local_source).unwrap();
        let deserialized: GrammarSource = toml::from_str(&serialized).unwrap();
        assert!(matches!(deserialized, GrammarSource::Local { .. }));

        // Test Library source
        let library_source = GrammarSource::Library {
            path: "/path/to/lib.so".into(),
        };

        let serialized = toml::to_string(&library_source).unwrap();
        let deserialized: GrammarSource = toml::from_str(&serialized).unwrap();
        assert!(matches!(deserialized, GrammarSource::Library { .. }));

        // Test Builtin source
        let builtin_source = GrammarSource::Builtin;
        let serialized = toml::to_string(&builtin_source).unwrap();
        let deserialized: GrammarSource = toml::from_str(&serialized).unwrap();
        assert!(matches!(deserialized, GrammarSource::Builtin));
    }

    #[test]
    fn test_config_manager_with_grammar_configs() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let config_path = temp_dir.path().join("uncomment.toml");

        // Test a simpler approach - inline grammar config
        let config_content = r#"
[global]
remove_docs = false

[languages.swift]
name = "Swift"
extensions = ["swift"]
comment_nodes = ["comment"]
doc_comment_nodes = ["doc_comment"]
remove_docs = true

[languages.swift.grammar]
source = { type = "git", url = "https://github.com/alex-pinkus/tree-sitter-swift", branch = "main" }
version = "1.0.0"
"#;

        std::fs::write(&config_path, config_content).unwrap();

        let manager = ConfigManager::new(temp_dir.path()).unwrap();

        // Test grammar config resolution
        let resolved =
            manager.get_config_for_file_with_language(temp_dir.path().join("test.swift"), "swift");

        assert!(resolved.grammar_config.is_some());
        let grammar_config = resolved.grammar_config.unwrap();
        assert!(matches!(grammar_config.source, GrammarSource::Git { .. }));
        assert_eq!(grammar_config.version, Some("1.0.0".to_string()));
        assert!(resolved.remove_docs); // Language-specific override

        // Test language without grammar config falls back to default
        let resolved_default =
            manager.get_config_for_file_with_language(temp_dir.path().join("test.rs"), "rust");

        assert!(resolved_default.grammar_config.is_none());
    }

    #[test]
    fn test_resolved_config_grammar_integration() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let manager = ConfigManager::new(temp_dir.path()).unwrap();

        // Test that resolved config includes grammar_config field
        let resolved = manager.get_config_for_file(temp_dir.path().join("test.rs"));
        assert!(resolved.grammar_config.is_none()); // No language-specific config

        // The grammar_config field should be properly initialized
        let _: Option<GrammarConfig> = resolved.grammar_config;
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
