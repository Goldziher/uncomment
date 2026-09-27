use crate::lint::config::LintTable;
use crate::paths;
use ahash::{AHashMap, AHashSet};
use anyhow::{Context, Result};
use globset::{GlobBuilder, GlobMatcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Once, RwLock};

const CONFIG_FILE_NAMES: [&str; 2] = [".uncommentrc.toml", "uncomment.toml"];

#[derive(Debug, Clone)]
pub struct DetectionInfo {
    pub detected_languages: HashMap<String, usize>,
    pub configured_languages: usize,
    pub total_files: usize,
}

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
        "" => default,
        _ => default,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default = "GlobalConfig::unspecified")]
    pub global: GlobalConfig,

    #[serde(default)]
    pub languages: HashMap<String, LanguageConfig>,

    #[serde(default)]
    pub patterns: HashMap<String, PatternConfig>,

    /// `[lint]` as written, or `None` when the file had no such section. Layering has to tell those
    /// apart, so the table keeps every key optional; see [`LintTable::layer_over`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lint: Option<LintTable>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GlobalConfig {
    /// Whether to remove TODO comments
    pub remove_todos: bool,

    /// Whether to remove FIXME comments
    pub remove_fixme: bool,

    pub remove_docs: bool,

    pub preserve_patterns: Vec<String>,

    pub use_default_ignores: bool,

    pub respect_gitignore: bool,

    pub traverse_git_repos: bool,

    /// Which flags the config file actually contained. `None` marks a value assembled in
    /// code, where every field is deliberate.
    #[serde(skip)]
    specified: Option<SpecifiedFlags>,
}

/// The `[global]` keys one config file set, so that layering can tell "key absent" from
/// "key set to the default value". Without this a nested config carrying only
/// `[patterns]` deserializes to all-defaults and erases the outer config's settings.
#[derive(Debug, Clone, Copy, Default)]
struct SpecifiedFlags {
    remove_todos: Option<bool>,
    remove_fixme: Option<bool>,
    remove_docs: Option<bool>,
    use_default_ignores: Option<bool>,
    respect_gitignore: Option<bool>,
    traverse_git_repos: Option<bool>,
}

/// The wire form of `[global]`: every flag optional, so an absent key stays absent.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GlobalConfigFile {
    remove_todos: Option<bool>,
    remove_fixme: Option<bool>,
    remove_docs: Option<bool>,
    #[serde(default)]
    preserve_patterns: Vec<String>,
    use_default_ignores: Option<bool>,
    respect_gitignore: Option<bool>,
    traverse_git_repos: Option<bool>,
}

impl<'de> Deserialize<'de> for GlobalConfig {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let file = GlobalConfigFile::deserialize(deserializer)?;
        let defaults = Self::default();

        Ok(Self {
            remove_todos: file.remove_todos.unwrap_or(defaults.remove_todos),
            remove_fixme: file.remove_fixme.unwrap_or(defaults.remove_fixme),
            remove_docs: file.remove_docs.unwrap_or(defaults.remove_docs),
            preserve_patterns: file.preserve_patterns,
            use_default_ignores: file.use_default_ignores.unwrap_or(defaults.use_default_ignores),
            respect_gitignore: file.respect_gitignore.unwrap_or(defaults.respect_gitignore),
            traverse_git_repos: file.traverse_git_repos.unwrap_or(defaults.traverse_git_repos),
            specified: Some(SpecifiedFlags {
                remove_todos: file.remove_todos,
                remove_fixme: file.remove_fixme,
                remove_docs: file.remove_docs,
                use_default_ignores: file.use_default_ignores,
                respect_gitignore: file.respect_gitignore,
                traverse_git_repos: file.traverse_git_repos,
            }),
        })
    }
}

impl GlobalConfig {
    /// A `[global]` section the file did not contain: nothing is specified, so every flag
    /// is inherited from the enclosing layer.
    fn unspecified() -> Self {
        Self {
            specified: Some(SpecifiedFlags::default()),
            ..Self::default()
        }
    }

    /// The flags this layer contributes to a merge.
    fn specified(&self) -> SpecifiedFlags {
        self.specified.unwrap_or(SpecifiedFlags {
            remove_todos: Some(self.remove_todos),
            remove_fixme: Some(self.remove_fixme),
            remove_docs: Some(self.remove_docs),
            use_default_ignores: Some(self.use_default_ignores),
            respect_gitignore: Some(self.respect_gitignore),
            traverse_git_repos: Some(self.traverse_git_repos),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageConfig {
    pub name: String,

    pub extensions: Vec<String>,

    pub comment_nodes: Vec<String>,

    #[serde(default)]
    pub doc_comment_nodes: Vec<String>,

    #[serde(default)]
    pub preserve_patterns: Vec<String>,

    /// Override global remove_todos setting
    pub remove_todos: Option<bool>,

    /// Override global remove_fixme setting
    pub remove_fixme: Option<bool>,

    pub remove_docs: Option<bool>,

    pub use_default_ignores: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatternConfig {
    /// Whether to remove TODO comments
    pub remove_todos: Option<bool>,

    /// Whether to remove FIXME comments
    pub remove_fixme: Option<bool>,

    pub remove_docs: Option<bool>,

    /// `None` inherits the patterns already in force, `Some([])` clears them, and a
    /// non-empty list extends them.
    pub preserve_patterns: Option<Vec<String>>,

    pub use_default_ignores: Option<bool>,
}

impl PatternConfig {
    fn apply_to(&self, resolved: &mut ResolvedConfig) {
        if let Some(value) = self.remove_todos {
            resolved.remove_todos = value;
        }
        if let Some(value) = self.remove_fixme {
            resolved.remove_fixme = value;
        }
        if let Some(value) = self.remove_docs {
            resolved.remove_docs = value;
        }
        if let Some(value) = self.use_default_ignores {
            resolved.use_default_ignores = value;
        }
        match &self.preserve_patterns {
            None => {}
            Some(patterns) if patterns.is_empty() => resolved.preserve_patterns.clear(),
            Some(patterns) => {
                resolved.preserve_patterns.extend(patterns.iter().cloned());
                resolved.preserve_patterns.sort();
                resolved.preserve_patterns.dedup();
            }
        }
    }
}

/// `literal_separator` keeps `*` from crossing a `/`, so `src/*.py` matches
/// `src/main.py` but not `src/inner/main.py`.
fn compile_pattern_glob(pattern: &str) -> Result<GlobMatcher> {
    GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .map(|glob| glob.compile_matcher())
        .with_context(|| format!("Invalid glob in [patterns.\"{pattern}\"]"))
}

#[derive(Debug)]
struct CompiledPattern {
    pattern: String,
    matcher: GlobMatcher,
    depth: usize,
    config: PatternConfig,
}

/// A parsed config file together with the directory its `[patterns]` globs are
/// anchored at, and those globs compiled once up front.
#[derive(Debug)]
struct LoadedConfig {
    dir: PathBuf,
    /// The file it was read from, for error messages. `None` for a config assembled in code.
    path: Option<PathBuf>,
    config: Config,
    patterns: Vec<CompiledPattern>,
}

impl LoadedConfig {
    fn new(dir: PathBuf, path: Option<PathBuf>, config: Config) -> Result<Self> {
        let mut patterns = Vec::with_capacity(config.patterns.len());
        for (pattern, pattern_config) in &config.patterns {
            patterns.push(CompiledPattern {
                matcher: compile_pattern_glob(pattern)?,
                depth: pattern.split('/').filter(|part| !part.is_empty()).count(),
                pattern: pattern.clone(),
                config: pattern_config.clone(),
            });
        }

        // `Config::patterns` is a HashMap, so its iteration order is randomised per
        // process. Impose a total order instead: broader globs first, ties broken by
        // the glob text, last match wins.
        patterns.sort_by(|left, right| {
            left.depth
                .cmp(&right.depth)
                .then_with(|| left.pattern.cmp(&right.pattern))
        });

        Ok(Self {
            dir,
            path,
            config,
            patterns,
        })
    }

    fn apply_patterns(&self, file_path: &Path, resolved: &mut ResolvedConfig) {
        let Ok(relative) = file_path.strip_prefix(&self.dir) else {
            return;
        };

        for pattern in &self.patterns {
            if pattern.matcher.is_match(relative) {
                pattern.config.apply_to(resolved);
            }
        }
    }
}

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

#[derive(Debug)]
pub struct ConfigManager {
    /// The user-level config, merged beneath every project config.
    global_config: Option<Arc<LoadedConfig>>,

    /// The root directory and its ancestors up to the git root, outermost first.
    ancestor_configs: Vec<Arc<LoadedConfig>>,

    /// Configs below the root that may also declare custom languages, deepest first.
    /// Empty until [`ConfigManager::discover_language_sources`] is called, because
    /// finding them costs a walk and nothing else about resolution needs one.
    descendant_language_configs: Vec<Arc<LoadedConfig>>,

    /// An explicit `--config` file, which replaces directory discovery entirely.
    forced_config: Option<Arc<LoadedConfig>>,

    /// Memoised "is there a config file in this directory?", filled on first use.
    dir_configs: RwLock<AHashMap<PathBuf, Option<Arc<LoadedConfig>>>>,

    /// Memoised per-file resolution results.
    file_configs: RwLock<AHashMap<PathBuf, ResolvedConfig>>,

    /// The highest directory the upward search may reach.
    ceiling: PathBuf,

    /// The directory the requested paths are resolved against, and the floor of the
    /// eagerly loaded ancestor chain.
    root_dir: PathBuf,

    current_dir: PathBuf,

    lazy_language_warning: Once,

    /// First failure from a lazily discovered config, readable through
    /// `deferred_config_error`.
    deferred_error: RwLock<Option<String>>,
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
            specified: None,
        }
    }
}

impl Config {
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

    pub fn validate(&self) -> Result<()> {
        for (lang_name, lang_config) in &self.languages {
            if lang_config.name.is_empty() {
                return Err(anyhow::anyhow!("Language '{}' has empty name", lang_name));
            }

            if lang_config.extensions.is_empty() {
                return Err(anyhow::anyhow!("Language '{}' has no file extensions", lang_name));
            }

            if lang_config.comment_nodes.is_empty() {
                return Err(anyhow::anyhow!("Language '{}' has no comment node types", lang_name));
            }
        }

        let mut pattern_names: Vec<&String> = self.patterns.keys().collect();
        pattern_names.sort();
        for pattern in pattern_names {
            compile_pattern_glob(pattern)?;
        }

        // Each file's own `[lint]` table, judged on its own keys, so the caller's context names the
        // file that carries a broken pattern rather than whichever file the layered result came from.
        if let Some(lint) = &self.lint {
            lint.validate()?;
        }

        Ok(())
    }

    pub fn template_clean() -> String {
        r#"[global]
remove_todos = false
remove_fixme = false
remove_docs = false
preserve_patterns = ["HACK", "WORKAROUND", "NOTE"]
use_default_ignores = true
respect_gitignore = true
traverse_git_repos = false

[languages.python]
name = "Python"
extensions = [".py", ".pyw", ".pyi"]
comment_nodes = ["comment"]
preserve_patterns = ["mypy:", "type:", "noqa:", "pragma:"]
remove_docs = true

[languages.javascript]
name = "JavaScript"
extensions = [".js", ".jsx", ".mjs", ".cjs"]
comment_nodes = ["comment"]
preserve_patterns = ["@ts-expect-error", "@ts-ignore", "webpack"]

[languages.typescript]
name = "TypeScript"
extensions = [".ts", ".tsx", ".mts", ".cts", ".d.ts"]
comment_nodes = ["comment"]
preserve_patterns = ["@ts-expect-error", "@ts-ignore"]

[languages.ruby]
name = "Ruby"
extensions = ["rb", "rbw", "gemspec", "rake"]
comment_nodes = ["comment"]
preserve_patterns = ["rubocop:", "frozen_string_literal:"]

[patterns."tests/**/*"]
remove_todos = true

[patterns."**/*.spec.*"]
remove_docs = true
remove_todos = true

[patterns."**/*.generated.*"]
remove_docs = true
remove_todos = true
preserve_patterns = []
"#
        .to_string()
    }

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

# Example: Add Vue.js support
[languages.vue]
name = "Vue"
extensions = [".vue"]
comment_nodes = ["comment"]
preserve_patterns = ["eslint-", "@ts-", "prettier-ignore"]

# Example: Add Swift support
[languages.swift]
name = "Swift"
extensions = [".swift"]
comment_nodes = ["comment", "multiline_comment"]
preserve_patterns = ["MARK:", "TODO:", "FIXME:", "swiftlint:"]

# Example: Add Objective-C support
[languages.objc]
name = "ObjC"
extensions = [".m"]
comment_nodes = ["comment"]
doc_comment_nodes = ["comment"]
preserve_patterns = ["MARK:", "TODO:", "FIXME:"]

# [languages.custom]
# name = "Custom Language"
# extensions = ["cst"]
# comment_nodes = ["comment"]
#
# [languages.proprietary]
# name = "Proprietary"
# extensions = ["prop"]
# comment_nodes = ["comment"]
#
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

    pub fn comprehensive_template_clean() -> String {
        r#"[global]
remove_todos = false
remove_fixme = false
remove_docs = false
preserve_patterns = ["HACK", "WORKAROUND", "NOTE", "XXX", "FIXME", "TODO"]
use_default_ignores = true
respect_gitignore = true
traverse_git_repos = false

[languages.vue]
name = "Vue"
extensions = [".vue"]
comment_nodes = ["comment", "template_element"]
preserve_patterns = ["eslint-", "prettier-", "vue-", "@vue/"]

[languages.svelte]
name = "Svelte"
extensions = [".svelte"]
comment_nodes = ["comment", "text"]
preserve_patterns = ["eslint-", "prettier-", "svelte-"]

[languages.astro]
name = "Astro"
extensions = [".astro"]
comment_nodes = ["comment", "frontmatter"]
preserve_patterns = ["astro-", "eslint-"]

[languages.swift]
name = "Swift"
extensions = [".swift"]
comment_nodes = ["comment", "multiline_comment"]
preserve_patterns = ["swiftlint:", "TODO:", "FIXME:"]

[languages.objc]
name = "ObjC"
extensions = [".m"]
comment_nodes = ["comment"]
doc_comment_nodes = ["comment"]
preserve_patterns = ["MARK:", "TODO:", "FIXME:"]

[languages.kotlin]
name = "Kotlin"
extensions = [".kt", ".kts"]
comment_nodes = ["line_comment", "block_comment"]
preserve_patterns = ["ktlint:", "TODO:", "FIXME:"]

[languages.dart]
name = "Dart"
extensions = [".dart"]
comment_nodes = ["comment", "documentation_comment"]
preserve_patterns = ["ignore:", "TODO:", "FIXME:"]

[languages.zig]
name = "Zig"
extensions = [".zig"]
comment_nodes = ["line_comment", "doc_comment"]
preserve_patterns = ["TODO:", "FIXME:", "NOTE:"]

[languages.elixir]
name = "Elixir"
extensions = [".ex", ".exs"]
comment_nodes = ["comment"]
preserve_patterns = ["credo:", "dialyzer:", "TODO:", "FIXME:"]

[languages.haskell]
name = "Haskell"
extensions = [".hs", ".lhs"]
comment_nodes = ["comment"]
preserve_patterns = ["hlint:", "TODO:", "FIXME:"]

[languages.julia]
name = "Julia"
extensions = [".jl"]
comment_nodes = ["comment"]
preserve_patterns = ["TODO:", "FIXME:", "NOTE:"]

[languages.r]
name = "R"
extensions = [".r", ".R"]
comment_nodes = ["comment"]
preserve_patterns = ["TODO:", "FIXME:", "NOTE:"]

[languages.lua]
name = "Lua"
extensions = [".lua"]
comment_nodes = ["comment"]
preserve_patterns = ["TODO:", "FIXME:", "NOTE:"]

[languages.nix]
name = "Nix"
extensions = [".nix"]
comment_nodes = ["comment"]
preserve_patterns = ["TODO:", "FIXME:", "NOTE:"]

[patterns."tests/**/*"]
remove_todos = true

[patterns."**/*.spec.*"]
remove_docs = true
remove_todos = true

[patterns."**/*.test.*"]
remove_docs = true
remove_todos = true

[patterns."**/*.generated.*"]
remove_docs = true
remove_todos = true
preserve_patterns = []

[patterns."**/dist/**/*"]
remove_docs = true
remove_todos = true
preserve_patterns = []
"#
        .to_string()
    }

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

# Language-specific configurations

# Web Development Languages
[languages.vue]
name = "Vue"
extensions = [".vue"]
comment_nodes = ["comment"]
preserve_patterns = ["eslint-", "@ts-", "prettier-ignore"]

[languages.svelte]
name = "Svelte"
extensions = [".svelte"]
comment_nodes = ["comment"]
preserve_patterns = ["eslint-", "prettier-ignore"]

[languages.astro]
name = "Astro"
extensions = [".astro"]
comment_nodes = ["comment"]
preserve_patterns = ["eslint-", "prettier-ignore"]

# Mobile Development
[languages.swift]
name = "Swift"
extensions = [".swift"]
comment_nodes = ["comment", "multiline_comment"]
preserve_patterns = ["MARK:", "TODO:", "FIXME:", "swiftlint:"]

[languages.objc]
name = "ObjC"
extensions = [".m"]
comment_nodes = ["comment"]
doc_comment_nodes = ["comment"]
preserve_patterns = ["MARK:", "TODO:", "FIXME:"]

[languages.kotlin]
name = "Kotlin"
extensions = [".kt", ".kts"]
comment_nodes = ["line_comment", "block_comment"]
preserve_patterns = ["@Suppress", "ktlint:"]

[languages.dart]
name = "Dart"
extensions = [".dart"]
comment_nodes = ["comment"]
preserve_patterns = ["ignore:", "ignore_for_file:"]

# Systems Programming
[languages.zig]
name = "Zig"
extensions = [".zig"]
comment_nodes = ["line_comment"]
preserve_patterns = ["zig fmt:"]

[languages.nim]
name = "Nim"
extensions = ["nim", "nims"]
comment_nodes = ["comment"]
preserve_patterns = ["pragma:"]

# Functional Programming
[languages.haskell]
name = "Haskell"
extensions = [".hs", ".lhs"]
comment_nodes = ["comment"]
preserve_patterns = ["LANGUAGE", "OPTIONS_GHC"]

[languages.elixir]
name = "Elixir"
extensions = [".ex", ".exs"]
comment_nodes = ["comment"]
preserve_patterns = ["@doc", "@moduledoc"]

[languages.elm]
name = "Elm"
extensions = ["elm"]
comment_nodes = ["line_comment", "block_comment"]

[languages.clojure]
name = "Clojure"
extensions = ["clj", "cljs", "cljc", "edn"]
comment_nodes = ["comment"]

# Data Science & ML
[languages.r]
name = "R"
extensions = [".r", ".R"]
comment_nodes = ["comment"]
preserve_patterns = ["@param", "@return", "@export"]

[languages.julia]
name = "Julia"
extensions = [".jl"]
comment_nodes = ["comment"]
preserve_patterns = ["@doc", "@inline", "@noinline"]

# DevOps & Configuration
[languages.dockerfile]
name = "Dockerfile"
extensions = ["dockerfile"]
comment_nodes = ["comment"]

[languages.nix]
name = "Nix"
extensions = [".nix"]
comment_nodes = ["comment"]

[languages.lua]
name = "Lua"
extensions = [".lua"]
comment_nodes = ["comment"]

# Shell Scripting
[languages.fish]
name = "Fish"
extensions = ["fish"]
comment_nodes = ["comment"]

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

    pub fn smart_template<P: AsRef<Path>>(project_dir: P) -> Result<String> {
        use walkdir::WalkDir;

        let mut detected_languages = HashMap::new();
        let mut file_count = 0;

        let supported_extensions = [
            "py", "pyw", "pyi", "pyx", "pxd", "js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts", "rs", "go", "java",
            "c", "h", "cpp", "cc", "cxx", "hpp", "hxx", "hh", "rb", "yml", "yaml", "hcl", "tf", "tfvars", "vue",
            "svelte", "astro", "swift", "m", "kt", "kts", "dart", "zig", "nim", "hs", "lhs", "ex", "exs", "elm", "clj",
            "cljs", "cljc", "edn", "r", "jl", "nix", "lua", "fish", "html", "htm", "xhtml", "css", "xml", "xsd", "xsl",
            "xslt", "svg", "sql", "ps1", "psm1", "psd1", "proto", "ini", "cfg", "conf",
        ];

        for entry in WalkDir::new(project_dir.as_ref())
            .max_depth(3)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                if let Some(ext) = entry.path().extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    if supported_extensions.contains(&ext_str.as_str()) {
                        *detected_languages.entry(ext_str).or_insert(0) += 1;
                        file_count += 1;
                    }
                }

                if let Some(filename) = entry.path().file_name() {
                    let filename_str = filename.to_string_lossy().to_lowercase();
                    if filename_str == "dockerfile" {
                        *detected_languages.entry("dockerfile".to_string()).or_insert(0) += 1;
                        file_count += 1;
                    } else if filename_str == "makefile" || filename_str.ends_with(".mk") {
                        *detected_languages.entry("make".to_string()).or_insert(0) += 1;
                        file_count += 1;
                    }
                }
            }
        }

        if file_count == 0 {
            return Ok(Self::template());
        }

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

        let language_configs = Self::get_language_mappings();

        for (ext, count) in &detected_languages {
            if *count > 0 {
                config.push_str(&format!("# Found {count} {ext} files\n"));
            }
        }
        config.push('\n');

        let mut configured_keys = AHashSet::new();
        for ext in detected_languages.keys() {
            let lookup_key = match ext.as_str() {
                "py" | "pyw" | "pyi" | "pyx" | "pxd" => "py",
                "js" | "jsx" | "mjs" | "cjs" => "js",
                "ts" | "tsx" | "mts" | "cts" => "ts",
                "swift" => "swift",
                "m" => "objc",
                "kt" | "kts" => "kt",
                "hs" | "lhs" => "hs",
                "html" | "htm" | "xhtml" => "html",
                "xml" | "xsd" | "xsl" | "xslt" | "svg" => "xml",
                "ps1" | "psm1" | "psd1" => "ps1",
                "ini" | "cfg" | "conf" => "ini",
                other => other,
            };

            if configured_keys.insert(lookup_key)
                && let Some(lang_config) = language_configs.get(lookup_key)
            {
                config.push_str(lang_config);
                config.push('\n');
            }
        }

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

    pub fn smart_template_with_info<P: AsRef<Path>>(project_dir: P) -> Result<(String, DetectionInfo)> {
        use walkdir::WalkDir;

        let mut detected_languages = HashMap::new();
        let mut file_count = 0;
        let mut total_files = 0;

        let supported_extensions = [
            "py", "pyw", "pyi", "pyx", "pxd", "js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts", "rs", "go", "java",
            "c", "h", "cpp", "cc", "cxx", "hpp", "hxx", "hh", "rb", "yml", "yaml", "hcl", "tf", "tfvars", "vue",
            "svelte", "astro", "swift", "m", "kt", "kts", "dart", "zig", "nim", "hs", "lhs", "ex", "exs", "elm", "clj",
            "cljs", "cljc", "edn", "r", "jl", "nix", "lua", "fish", "html", "htm", "xhtml", "css", "xml", "xsd", "xsl",
            "xslt", "svg", "sql", "ps1", "psm1", "psd1", "proto", "ini", "cfg", "conf",
        ];

        for entry in WalkDir::new(project_dir.as_ref())
            .max_depth(3)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                total_files += 1;

                if let Some(ext) = entry.path().extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    if supported_extensions.contains(&ext_str.as_str()) {
                        let lang_name = match ext_str.as_str() {
                            "py" | "pyw" | "pyi" | "pyx" | "pxd" => "Python",
                            "js" | "jsx" | "mjs" | "cjs" => "JavaScript",
                            "ts" | "tsx" | "mts" | "cts" => "TypeScript",
                            "rs" => "Rust",
                            "go" => "Go",
                            "java" => "Java",
                            "c" | "h" => "C",
                            "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" => "C++",
                            "rb" => "Ruby",
                            "yml" | "yaml" => "YAML",
                            "hcl" | "tf" | "tfvars" => "HCL/Terraform",
                            "vue" => "Vue",
                            "svelte" => "Svelte",
                            "astro" => "Astro",
                            "swift" => "Swift",
                            "m" => "Objective-C",
                            "kt" | "kts" => "Kotlin",
                            "dart" => "Dart",
                            "zig" => "Zig",
                            "nim" => "Nim",
                            "hs" | "lhs" => "Haskell",
                            "ex" | "exs" => "Elixir",
                            "elm" => "Elm",
                            "clj" | "cljs" | "cljc" | "edn" => "Clojure",
                            "r" => "R",
                            "jl" => "Julia",
                            "nix" => "Nix",
                            "lua" => "Lua",
                            "fish" => "Fish",
                            "html" | "htm" | "xhtml" => "HTML",
                            "css" => "CSS",
                            "xml" | "xsd" | "xsl" | "xslt" | "svg" => "XML",
                            "sql" => "SQL",
                            "ps1" | "psm1" | "psd1" => "PowerShell",
                            "proto" => "Proto",
                            "ini" | "cfg" | "conf" => "INI",
                            _ => &ext_str,
                        };
                        *detected_languages.entry(lang_name.to_string()).or_insert(0) += 1;
                        file_count += 1;
                    }
                }

                if let Some(filename) = entry.path().file_name() {
                    let filename_str = filename.to_string_lossy().to_lowercase();
                    if filename_str == "dockerfile" {
                        *detected_languages.entry("Docker".to_string()).or_insert(0) += 1;
                        file_count += 1;
                    } else if filename_str == "makefile" || filename_str.ends_with(".mk") {
                        *detected_languages.entry("Makefile".to_string()).or_insert(0) += 1;
                        file_count += 1;
                    }
                }
            }
        }

        if file_count == 0 {
            let detection_info = DetectionInfo {
                detected_languages: HashMap::new(),
                configured_languages: 0,
                total_files,
            };
            return Ok((Self::template_clean(), detection_info));
        }

        let mut config = String::from(
            r#"[global]
remove_todos = false
remove_fixme = false
remove_docs = false
preserve_patterns = ["HACK", "WORKAROUND", "NOTE"]
use_default_ignores = true
respect_gitignore = true
traverse_git_repos = false

"#,
        );

        let language_configs = Self::get_language_mappings();
        let mut configured_languages = 0;

        for lang_name in detected_languages.keys() {
            let lookup_key = match lang_name.as_str() {
                "Python" => "py",
                "JavaScript" => "js",
                "TypeScript" => "ts",
                "Rust" => "rs",
                "Go" => "go",
                "Java" => "java",
                "C" => "c",
                "C++" => "cpp",
                "Ruby" => "rb",
                "YAML" => "yml",
                "HCL/Terraform" => "hcl",
                "Vue" => "vue",
                "Svelte" => "svelte",
                "Astro" => "astro",
                "Swift" => "swift",
                "Objective-C" => "objc",
                "Kotlin" => "kt",
                "Dart" => "dart",
                "Zig" => "zig",
                "Nim" => "nim",
                "Haskell" => "hs",
                "Elixir" => "ex",
                "Elm" => "elm",
                "Clojure" => "clj",
                "R" => "r",
                "Julia" => "jl",
                "Nix" => "nix",
                "Lua" => "lua",
                "Fish" => "fish",
                "HTML" => "html",
                "CSS" => "css",
                "XML" => "xml",
                "SQL" => "sql",
                "PowerShell" => "ps1",
                "Proto" => "proto",
                "INI" => "ini",
                "Docker" => "dockerfile",
                "Makefile" => "make",
                _ => continue,
            };

            if let Some(lang_config) = language_configs.get(lookup_key) {
                config.push_str(lang_config);
                config.push_str("\n\n");
                configured_languages += 1;
            }
        }

        if configured_languages > 0 {
            config.push_str(
                r#"[patterns."tests/**/*"]
remove_todos = true

[patterns."**/*.spec.*"]
remove_docs = true
remove_todos = true

[patterns."**/*.generated.*"]
remove_docs = true
remove_todos = true
preserve_patterns = []
"#,
            );
        }

        let detection_info = DetectionInfo {
            detected_languages,
            configured_languages,
            total_files,
        };

        Ok((config, detection_info))
    }

    pub fn interactive_template_clean() -> Result<String> {
        use std::io::{self, Write};

        println!("🚀 Welcome to Uncomment Interactive Configuration!");
        println!("I'll help you create a customized configuration file.\n");

        let remove_todos = prompt_bool("Remove TODO comments by default? (y/n)", false)?;
        let remove_fixme = prompt_bool("Remove FIXME comments by default? (y/n)", false)?;
        let remove_docs = prompt_bool("Remove documentation comments by default? (y/n)", false)?;

        println!("\n📋 Available languages with grammar support:");
        let available_languages = vec![
            ("vue", "Vue.js single-file components"),
            ("svelte", "Svelte components"),
            ("swift", "Swift (iOS/macOS development)"),
            ("objc", "Objective-C (iOS/macOS development)"),
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
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        let mut selected_languages = Vec::new();

        if input == "all" {
            selected_languages = available_languages.iter().map(|(name, _)| *name).collect();
        } else if input != "skip" {
            for num_str in input.split(',') {
                if let Ok(num) = num_str.trim().parse::<usize>()
                    && num > 0
                    && num <= available_languages.len()
                {
                    selected_languages.push(available_languages[num - 1].0);
                }
            }
        }

        let mut config = format!(
            r#"[global]
remove_todos = {remove_todos}
remove_fixme = {remove_fixme}
remove_docs = {remove_docs}
preserve_patterns = ["HACK", "WORKAROUND", "NOTE"]
use_default_ignores = true
respect_gitignore = true
traverse_git_repos = false

"#
        );

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

    pub fn interactive_template() -> Result<String> {
        use std::io::{self, Write};

        println!("🚀 Welcome to Uncomment Interactive Configuration!");
        println!("I'll help you create a customized configuration file.\n");

        let remove_todos = prompt_bool("Remove TODO comments by default? (y/n)", false)?;
        let remove_fixme = prompt_bool("Remove FIXME comments by default? (y/n)", false)?;
        let remove_docs = prompt_bool("Remove documentation comments by default? (y/n)", false)?;

        println!("\n📋 Available languages with grammar support:");
        let available_languages = vec![
            ("vue", "Vue.js single-file components"),
            ("svelte", "Svelte components"),
            ("swift", "Swift (iOS/macOS development)"),
            ("objc", "Objective-C (iOS/macOS development)"),
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
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        let mut selected_languages = Vec::new();

        if input == "all" {
            selected_languages = available_languages.iter().map(|(name, _)| *name).collect();
        } else if input != "skip" {
            for num_str in input.split(',') {
                if let Ok(num) = num_str.trim().parse::<usize>()
                    && num > 0
                    && num <= available_languages.len()
                {
                    selected_languages.push(available_languages[num - 1].0);
                }
            }
        }

        let mut config = format!(
            r#"# Interactive Uncomment Configuration
# Generated through interactive setup
# https://github.com/Goldziher/uncomment

[global]
remove_todos = {remove_todos}
remove_fixme = {remove_fixme}
remove_docs = {remove_docs}
preserve_patterns = ["HACK", "WORKAROUND", "NOTE"]
use_default_ignores = true
respect_gitignore = true
traverse_git_repos = false

"#
        );

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

    fn get_language_mappings() -> std::collections::HashMap<String, &'static str> {
        let mut map = std::collections::HashMap::new();

        map.insert(
            "py".to_string(),
            r#"[languages.python]
name = "Python"
extensions = [".py", ".pyw", ".pyi"]
comment_nodes = ["comment"]
preserve_patterns = ["mypy:", "type:", "noqa:", "pragma:", "pylint:"]
remove_docs = false"#,
        );

        map.insert(
            "js".to_string(),
            r#"[languages.javascript]
name = "JavaScript"
extensions = [".js", ".jsx", ".mjs", ".cjs"]
comment_nodes = ["comment"]
preserve_patterns = ["@ts-expect-error", "@ts-ignore", "webpack", "eslint-"]"#,
        );

        map.insert(
            "ts".to_string(),
            r#"[languages.typescript]
name = "TypeScript"
extensions = [".ts", ".tsx", ".mts", ".cts", ".d.ts"]
comment_nodes = ["comment"]
preserve_patterns = ["@ts-expect-error", "@ts-ignore", "eslint-"]"#,
        );

        map.insert(
            "rs".to_string(),
            r#"[languages.rust]
name = "Rust"
extensions = [".rs"]
comment_nodes = ["line_comment", "block_comment"]
doc_comment_nodes = ["doc_comment"]
preserve_patterns = ["clippy:", "allow", "deny", "warn"]
remove_docs = false"#,
        );

        map.insert(
            "go".to_string(),
            r##"[languages.go]
name = "Go"
extensions = [".go"]
comment_nodes = ["comment"]
preserve_patterns = ["go:build", "go:generate", "go:embed", "go:cgo", "+build", "nolint", "#cgo", "#include"]"##,
        );

        map.insert(
            "rb".to_string(),
            r#"[languages.ruby]
name = "Ruby"
extensions = [".rb", ".rbw", "gemspec", "rake"]
comment_nodes = ["comment"]
preserve_patterns = ["rubocop:", "frozen_string_literal:"]
remove_docs = false"#,
        );

        map.insert(
            "php".to_string(),
            r#"[languages.php]
name = "PHP"
extensions = [".php", ".phtml"]
comment_nodes = ["comment"]
preserve_patterns = []
remove_docs = false"#,
        );

        map.insert(
            "ex".to_string(),
            r#"[languages.elixir]
name = "Elixir"
extensions = [".ex", ".exs"]
comment_nodes = ["comment"]
preserve_patterns = []
remove_docs = false"#,
        );

        map.insert(
            "toml".to_string(),
            r#"[languages.toml]
name = "TOML"
extensions = [".toml"]
comment_nodes = ["comment"]
preserve_patterns = []
remove_docs = false"#,
        );

        map.insert(
            "cs".to_string(),
            r#"[languages.csharp]
name = "CSharp"
extensions = [".cs"]
comment_nodes = ["comment"]
preserve_patterns = []
remove_docs = false"#,
        );

        map.insert(
            "java".to_string(),
            r#"[languages.java]
name = "Java"
extensions = [".java"]
comment_nodes = ["line_comment", "block_comment"]
doc_comment_nodes = ["doc_comment"]
preserve_patterns = ["@SuppressWarnings", "@Override"]
remove_docs = false"#,
        );

        map.insert(
            "vue".to_string(),
            r#"[languages.vue]
name = "Vue"
extensions = [".vue"]
comment_nodes = ["comment"]
preserve_patterns = ["eslint-", "@ts-", "prettier-ignore"]"#,
        );

        map.insert(
            "dockerfile".to_string(),
            r#"[languages.dockerfile]
name = "Dockerfile"
extensions = ["dockerfile"]
comment_nodes = ["comment"]"#,
        );

        map.insert(
            "swift".to_string(),
            r#"[languages.swift]
name = "Swift"
extensions = [".swift"]
comment_nodes = ["comment", "multiline_comment"]
preserve_patterns = ["MARK:", "TODO:", "FIXME:", "swiftlint:"]"#,
        );

        map.insert(
            "objc".to_string(),
            r#"[languages.objc]
name = "ObjC"
extensions = [".m"]
comment_nodes = ["comment"]
doc_comment_nodes = ["comment"]
preserve_patterns = ["MARK:", "TODO:", "FIXME:"]"#,
        );

        map.insert(
            "kt".to_string(),
            r#"[languages.kotlin]
name = "Kotlin"
extensions = [".kt", ".kts"]
comment_nodes = ["line_comment", "block_comment"]
preserve_patterns = ["@Suppress", "ktlint:"]"#,
        );

        map.insert(
            "hs".to_string(),
            r#"[languages.haskell]
name = "Haskell"
extensions = [".hs", ".lhs"]
comment_nodes = ["comment"]
preserve_patterns = ["LANGUAGE", "OPTIONS_GHC"]"#,
        );

        map.insert(
            "html".to_string(),
            r#"[languages.html]
name = "HTML"
extensions = [".html", ".htm", ".xhtml"]
comment_nodes = ["comment"]"#,
        );

        map.insert(
            "css".to_string(),
            r#"[languages.css]
name = "CSS"
extensions = [".css"]
comment_nodes = ["comment"]"#,
        );

        map.insert(
            "xml".to_string(),
            r#"[languages.xml]
name = "XML"
extensions = [".xml", ".xsd", ".xsl", ".xslt", ".svg"]
comment_nodes = ["Comment"]"#,
        );

        map.insert(
            "sql".to_string(),
            r#"[languages.sql]
name = "SQL"
extensions = [".sql"]
comment_nodes = ["comment"]"#,
        );

        map.insert(
            "lua".to_string(),
            r#"[languages.lua]
name = "Lua"
extensions = [".lua"]
comment_nodes = ["comment"]"#,
        );

        map.insert(
            "nix".to_string(),
            r#"[languages.nix]
name = "Nix"
extensions = [".nix"]
comment_nodes = ["comment"]"#,
        );

        map.insert(
            "ps1".to_string(),
            r#"[languages.powershell]
name = "PowerShell"
extensions = [".ps1", ".psm1", ".psd1"]
comment_nodes = ["comment"]"#,
        );

        map.insert(
            "proto".to_string(),
            r#"[languages.proto]
name = "Proto"
extensions = [".proto"]
comment_nodes = ["comment"]"#,
        );

        map.insert(
            "ini".to_string(),
            r#"[languages.ini]
name = "INI"
extensions = [".ini", ".cfg", ".conf"]
comment_nodes = ["comment"]"#,
        );

        map
    }

    fn get_extended_language_mappings() -> std::collections::HashMap<&'static str, &'static str> {
        let mut map = std::collections::HashMap::new();

        map.insert(
            "vue",
            r#"[languages.vue]
name = "Vue"
extensions = [".vue"]
comment_nodes = ["comment"]
preserve_patterns = ["eslint-", "@ts-", "prettier-ignore"]"#,
        );

        map.insert(
            "svelte",
            r#"[languages.svelte]
name = "Svelte"
extensions = [".svelte"]
comment_nodes = ["comment"]
preserve_patterns = ["eslint-", "prettier-ignore"]"#,
        );

        map.insert(
            "swift",
            r#"[languages.swift]
name = "Swift"
extensions = [".swift"]
comment_nodes = ["comment", "multiline_comment"]
preserve_patterns = ["MARK:", "TODO:", "FIXME:", "swiftlint:"]"#,
        );

        map.insert(
            "objc",
            r#"[languages.objc]
name = "ObjC"
extensions = [".m"]
comment_nodes = ["comment"]
doc_comment_nodes = ["comment"]
preserve_patterns = ["MARK:", "TODO:", "FIXME:"]"#,
        );

        map.insert(
            "kotlin",
            r#"[languages.kotlin]
name = "Kotlin"
extensions = [".kt", ".kts"]
comment_nodes = ["line_comment", "block_comment"]
preserve_patterns = ["@Suppress", "ktlint:"]"#,
        );

        map.insert(
            "dart",
            r#"[languages.dart]
name = "Dart"
extensions = [".dart"]
comment_nodes = ["comment"]
preserve_patterns = ["ignore:", "ignore_for_file:"]"#,
        );

        map.insert(
            "zig",
            r#"[languages.zig]
name = "Zig"
extensions = [".zig"]
comment_nodes = ["line_comment"]
preserve_patterns = ["zig fmt:"]"#,
        );

        map.insert(
            "haskell",
            r#"[languages.haskell]
name = "Haskell"
extensions = [".hs", ".lhs"]
comment_nodes = ["comment"]
preserve_patterns = ["LANGUAGE", "OPTIONS_GHC"]"#,
        );

        map.insert(
            "elixir",
            r#"[languages.elixir]
name = "Elixir"
extensions = [".ex", ".exs"]
comment_nodes = ["comment"]
preserve_patterns = ["@doc", "@moduledoc"]"#,
        );

        map.insert(
            "r",
            r#"[languages.r]
name = "R"
extensions = [".r", ".R"]
comment_nodes = ["comment"]
preserve_patterns = ["@param", "@return", "@export"]"#,
        );

        map.insert(
            "julia",
            r#"[languages.julia]
name = "Julia"
extensions = [".jl"]
comment_nodes = ["comment"]
preserve_patterns = ["@doc", "@inline", "@noinline"]"#,
        );

        map.insert(
            "nix",
            r#"[languages.nix]
name = "Nix"
extensions = [".nix"]
comment_nodes = ["comment"]"#,
        );

        map.insert(
            "lua",
            r#"[languages.lua]
name = "Lua"
extensions = [".lua"]
comment_nodes = ["comment"]"#,
        );

        map
    }

    /// Layer `other` on top of `self`. A `[global]` key `other`'s file did not contain
    /// keeps `self`'s value, so a config carrying only `[patterns]` changes no globals.
    pub fn merge_with(&self, other: &Config) -> Config {
        let mut merged = self.clone();

        let specified = other.global.specified();
        let overrides = [
            (&mut merged.global.remove_todos, specified.remove_todos),
            (&mut merged.global.remove_fixme, specified.remove_fixme),
            (&mut merged.global.remove_docs, specified.remove_docs),
            (&mut merged.global.use_default_ignores, specified.use_default_ignores),
            (&mut merged.global.respect_gitignore, specified.respect_gitignore),
            (&mut merged.global.traverse_git_repos, specified.traverse_git_repos),
        ];
        for (target, value) in overrides {
            if let Some(value) = value {
                *target = value;
            }
        }
        merged.global.specified = None;

        let mut patterns = merged.global.preserve_patterns.clone();
        patterns.extend(other.global.preserve_patterns.iter().cloned());
        patterns.sort();
        patterns.dedup();
        merged.global.preserve_patterns = patterns;

        merged.languages.extend(
            other
                .languages
                .iter()
                .map(|(name, config)| (name.clone(), config.clone())),
        );
        merged.patterns.extend(
            other
                .patterns
                .iter()
                .map(|(pattern, config)| (pattern.clone(), config.clone())),
        );

        // Key by key, like `[global]`: a nested `[lint]` amends the table above it instead of
        // replacing it, so naming one rule does not silently reset `enabled` or the tag vocabulary.
        merged.lint = match (&merged.lint, &other.lint) {
            (base, None) => base.clone(),
            (None, Some(over)) => Some(over.clone()),
            (Some(base), Some(over)) => Some(over.layer_over(base)),
        };

        merged
    }
}

impl ConfigManager {
    pub fn new<P: AsRef<Path>>(root_dir: P) -> Result<Self> {
        let current_dir = std::env::current_dir().context("Failed to get current directory")?;
        let root_dir = paths::absolute_normalized(&current_dir, root_dir.as_ref());
        // Bounding the upward search at the git root keeps configuration outside the
        // repository from applying.
        let ceiling = paths::find_repo_root(&root_dir).unwrap_or_else(|| root_dir.clone());

        // Only the ancestor chain can ever apply to `root_dir` itself, and it is
        // bounded by path depth rather than by tree size. Everything below the root
        // is discovered lazily, on the first file that needs it.
        let mut ancestor_configs = Vec::new();
        let mut dir_configs = AHashMap::new();
        let mut dir = Some(root_dir.as_path());
        while let Some(current) = dir {
            let loaded = Self::load_dir_config(current)?;
            if let Some(loaded) = &loaded {
                ancestor_configs.push(loaded.clone());
            }
            dir_configs.insert(current.to_path_buf(), loaded);

            if current == ceiling {
                break;
            }
            dir = current.parent();
        }
        ancestor_configs.reverse();

        Ok(Self {
            global_config: Self::load_global_config(),
            ancestor_configs,
            descendant_language_configs: Vec::new(),
            forced_config: None,
            dir_configs: RwLock::new(dir_configs),
            file_configs: RwLock::new(AHashMap::new()),
            ceiling,
            root_dir,
            current_dir,
            lazy_language_warning: Once::new(),
            deferred_error: RwLock::new(None),
        })
    }

    pub fn from_single_config<P: AsRef<Path>>(root_dir: P, config: Config) -> Result<Self> {
        Self::forced(root_dir, None, config)
    }

    /// One explicit config file — `--config` — read here so that errors naming it stay available to
    /// every reader of the resulting manager, `[lint]` included.
    pub fn from_config_file<P: AsRef<Path>, Q: AsRef<Path>>(root_dir: P, path: Q) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let config = Config::from_file(&path)?;
        Self::forced(root_dir, Some(path), config)
    }

    fn forced<P: AsRef<Path>>(root_dir: P, path: Option<PathBuf>, config: Config) -> Result<Self> {
        let current_dir = std::env::current_dir().context("Failed to get current directory")?;
        let root_dir = paths::absolute_normalized(&current_dir, root_dir.as_ref());
        let loaded = Arc::new(LoadedConfig::new(root_dir.clone(), path, config)?);

        Ok(Self {
            global_config: None,
            ancestor_configs: vec![loaded.clone()],
            descendant_language_configs: Vec::new(),
            forced_config: Some(loaded),
            dir_configs: RwLock::new(AHashMap::new()),
            file_configs: RwLock::new(AHashMap::new()),
            ceiling: root_dir.clone(),
            root_dir,
            current_dir,
            lazy_language_warning: Once::new(),
            deferred_error: RwLock::new(None),
        })
    }

    fn global_config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join("uncomment").join("config.toml"))
    }

    fn load_global_config() -> Option<Arc<LoadedConfig>> {
        let path = Self::global_config_path()?;
        if !path.exists() {
            return None;
        }

        let dir = path.parent()?.to_path_buf();
        match Config::from_file(&path).and_then(|config| LoadedConfig::new(dir, Some(path.clone()), config)) {
            Ok(loaded) => Some(Arc::new(loaded)),
            Err(e) => {
                eprintln!("Warning: Failed to load global config: {e}");
                None
            }
        }
    }

    /// The config governing `dir`, or `None` when the directory has none.
    ///
    /// A config file that exists but cannot be loaded is an error. uncomment rewrites
    /// files in place, so a config it cannot understand must stop the run instead of
    /// degrading to built-in defaults — which would empty `preserve_patterns`.
    fn load_dir_config(dir: &Path) -> Result<Option<Arc<LoadedConfig>>> {
        // The first name that *exists* wins whether or not it parses; otherwise a typo in
        // `.uncommentrc.toml` would silently promote `uncomment.toml` to authoritative.
        let Some(path) = CONFIG_FILE_NAMES
            .iter()
            .map(|file_name| dir.join(file_name))
            .find(|path| path.is_file())
        else {
            return Ok(None);
        };

        let config = Config::from_file(&path)?;
        let loaded = LoadedConfig::new(dir.to_path_buf(), Some(path.clone()), config)
            .with_context(|| format!("Invalid configuration in: {}", path.display()))?;

        Ok(Some(Arc::new(loaded)))
    }

    /// The first failure from a lazily discovered config, once resolution has run.
    ///
    /// `get_config_for_file` is called per file from the parallel processing pass and
    /// cannot return an error, so a caller that rewrites files must check this before
    /// reporting success. Configs on the ancestor chain fail `ConfigManager::new`
    /// outright and never land here.
    pub fn deferred_config_error(&self) -> Option<String> {
        self.deferred_error.read().ok().and_then(|slot| slot.clone())
    }

    fn record_deferred_error(&self, error: anyhow::Error) {
        let message = format!("{error:#}");
        if let Ok(mut slot) = self.deferred_error.write()
            && slot.is_none()
        {
            eprintln!("error: {message}");
            *slot = Some(message);
        }
    }

    fn dir_config(&self, dir: &Path) -> Option<Arc<LoadedConfig>> {
        if let Ok(cache) = self.dir_configs.read()
            && let Some(entry) = cache.get(dir)
        {
            return entry.clone();
        }

        let loaded = match Self::load_dir_config(dir) {
            Ok(loaded) => loaded,
            Err(error) => {
                self.record_deferred_error(error);
                None
            }
        };

        if let Some(loaded) = &loaded
            && !loaded.config.languages.is_empty()
            && !self.is_language_source(&loaded.dir)
        {
            let path = loaded.dir.display().to_string();
            self.lazy_language_warning.call_once(|| {
                eprintln!(
                    "Warning: [languages] in the config under {path} is ignored; the language registry was \
                     already built when that config was reached."
                );
            });
        }

        if let Ok(mut cache) = self.dir_configs.write() {
            cache.insert(dir.to_path_buf(), loaded.clone());
        }

        loaded
    }

    /// Whether `dir`'s config already contributed its `[languages]` to the registry.
    fn is_language_source(&self, dir: &Path) -> bool {
        self.language_sources().any(|loaded| loaded.dir == dir)
    }

    /// The config files that apply to `dir`, outermost first. `dir` must already be
    /// normalized: the walk below is lexical and a stray `..` would climb past the
    /// ceiling.
    fn config_chain(&self, dir: &Path) -> Vec<Arc<LoadedConfig>> {
        if let Some(forced) = &self.forced_config {
            return vec![forced.clone()];
        }

        let mut chain = Vec::new();
        let mut current = Some(dir);
        while let Some(candidate) = current {
            if !paths::is_ancestor_of(&self.ceiling, candidate) {
                break;
            }
            if let Some(loaded) = self.dir_config(candidate) {
                chain.push(loaded);
            }
            current = candidate.parent();
        }
        chain.reverse();

        chain
    }

    /// Resolve the effective configuration for one file.
    ///
    /// Layers are applied outermost first: the user-level config, then every
    /// `.uncommentrc.toml`/`uncomment.toml` from the search ceiling down to the
    /// file's own directory.
    ///
    /// `[patterns."<glob>"]` sections are applied last. For a **discovered** config each
    /// glob is matched against the file path relative to the directory holding that
    /// config file, so a nested config's globs are anchored at that nested directory
    /// rather than at the invocation root. A config forced with `--config` is anchored at
    /// the **invocation directory** instead of at wherever the file itself lives, so
    /// `--config ../shared/uncomment.toml` with a `src/**` glob means `src/**` under the
    /// current directory.
    ///
    /// Within one config file the globs are applied in a fixed order — fewer path
    /// components first, ties broken by the glob text — and the last match wins, which
    /// keeps the result independent of `HashMap` iteration order.
    fn resolve_config_for_file(&self, file_path: &Path) -> ResolvedConfig {
        let dir = file_path.parent().unwrap_or(file_path);
        let chain = self.config_chain(dir);

        let mut base_config = Config::default();
        if let Some(global) = &self.global_config {
            base_config = base_config.merge_with(&global.config);
        }
        for loaded in &chain {
            base_config = base_config.merge_with(&loaded.config);
        }

        let mut resolved = ResolvedConfig {
            remove_todos: base_config.global.remove_todos,
            remove_fixme: base_config.global.remove_fixme,
            remove_docs: base_config.global.remove_docs,
            preserve_patterns: base_config.global.preserve_patterns,
            use_default_ignores: base_config.global.use_default_ignores,
            respect_gitignore: base_config.global.respect_gitignore,
            traverse_git_repos: base_config.global.traverse_git_repos,
            language_config: None,
        };

        for loaded in &chain {
            loaded.apply_patterns(file_path, &mut resolved);
        }

        resolved
    }

    pub fn get_config_for_file<P: AsRef<Path>>(&self, file_path: P) -> ResolvedConfig {
        // Normalizing up front makes the cache key canonical and, more importantly, keeps
        // a `..` in the path from reaching a directory the walk must not see.
        let file_path = paths::absolute_normalized(&self.current_dir, file_path.as_ref());

        if let Ok(cache) = self.file_configs.read()
            && let Some(cached) = cache.get(&file_path)
        {
            return cached.clone();
        }

        let resolved = self.resolve_config_for_file(&file_path);
        if let Ok(mut cache) = self.file_configs.write() {
            cache.insert(file_path, resolved.clone());
        }

        resolved
    }

    /// The `[lint]` table in force for one file, layered exactly like the rest of the configuration:
    /// the user-level config first, then every config file from the search ceiling down to the file's
    /// own directory, each one amending the last key by key.
    ///
    /// The second element is the innermost config file that contributed a `[lint]` section, which is
    /// what a table-level mistake is reported against. `None` there means no file did — the built-in
    /// defaults — or that the config came from code rather than from a file.
    ///
    /// Infallible, like [`Self::get_config_for_file`]: a config file below the invocation directory is
    /// discovered here, and a rejected one is recorded for [`Self::deferred_config_error`] rather than
    /// returned. A caller that may rewrite files has to check that before acting on the result.
    pub fn lint_table_for_file<P: AsRef<Path>>(&self, file_path: P) -> (Option<LintTable>, Option<PathBuf>) {
        let file_path = paths::absolute_normalized(&self.current_dir, file_path.as_ref());
        let dir = file_path.parent().unwrap_or(&file_path);

        let mut table: Option<LintTable> = None;
        let mut source: Option<PathBuf> = None;
        for loaded in self.global_config.iter().chain(self.config_chain(dir).iter()) {
            let Some(next) = &loaded.config.lint else {
                continue;
            };
            table = Some(match &table {
                Some(base) => next.layer_over(base),
                None => next.clone(),
            });
            if let Some(path) = &loaded.path {
                source = Some(path.clone());
            }
        }

        (table, source)
    }

    pub fn get_config_for_file_with_language<P: AsRef<Path>>(
        &self,
        file_path: P,
        language_name: &str,
    ) -> ResolvedConfig {
        let mut config = self.get_config_for_file(file_path);

        if let Some(lang_config) = self.get_language_config(language_name) {
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

            config
                .preserve_patterns
                .extend(lang_config.preserve_patterns.iter().cloned());
            config.preserve_patterns.sort();
            config.preserve_patterns.dedup();

            config.language_config = Some(lang_config);
        }

        config
    }

    /// Extend the set of configs that may declare custom languages to cover `paths`.
    ///
    /// A custom language has to be known *before* file collection. Collection keeps only the files
    /// whose extension some registry recognises, so a file named solely by a `[languages]` section
    /// is discarded before anything reads that section, and the declaration does nothing at all.
    /// The ancestor chain is loaded eagerly and so is available in time; a config below the
    /// invocation directory is otherwise reached only per file, long after collection.
    ///
    /// This is what separates the two halves of a config file. Language *declarations* are gathered
    /// here, up front, because one registry serves the whole run; every behavioural setting —
    /// `remove_todos`, `preserve_patterns`, `[patterns]`, `[lint]` — stays lazily resolved per
    /// file. That keeps the cost bounded by the number of config files rather than by the number of
    /// directories: the sweep never descends further than collection itself will, prunes `.git`,
    /// honours the same ignore rules, and opens only files named like a config.
    ///
    /// The result is that the configs contributing languages are exactly the ones per-file
    /// resolution could return for the requested paths. Within that set the deepest declaration
    /// wins, and because there is a single registry for the run a language declared in a
    /// subdirectory is recognised for the whole of it.
    ///
    /// A config that cannot be parsed is skipped rather than reported here: it is reported, against
    /// its own path, when resolution reaches it — see [`Self::deferred_config_error`]. Sweeping is
    /// speculative, and a broken config in a subtree the run never touches must not fail the run.
    ///
    /// `--config` replaces discovery entirely, so this does nothing for a manager built from one.
    pub fn discover_language_sources(&mut self, paths: &[String], respect_gitignore: bool) {
        if self.forced_config.is_some() {
            return;
        }

        let mut seen: AHashSet<PathBuf> = self.ancestor_configs.iter().map(|loaded| loaded.dir.clone()).collect();
        let mut found: Vec<Arc<LoadedConfig>> = Vec::new();

        for root in Self::language_scan_roots(&self.root_dir, paths) {
            // A requested path may sit below the root directory, so the configs between the two are
            // not on the eagerly loaded chain either. This walk is bounded by path depth.
            let mut current = Some(root.as_path());
            while let Some(dir) = current {
                if !paths::is_ancestor_of(&self.ceiling, dir) {
                    break;
                }
                self.take_language_source(dir, &mut seen, &mut found);
                current = dir.parent();
            }

            let walker = ignore::WalkBuilder::new(&root)
                .hidden(false)
                .git_ignore(respect_gitignore)
                .git_global(respect_gitignore)
                .git_exclude(respect_gitignore)
                .parents(respect_gitignore)
                .require_git(false)
                .filter_entry(|entry| entry.file_name() != std::ffi::OsStr::new(".git"))
                .build();

            for entry in walker.flatten() {
                let is_config_name = entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| CONFIG_FILE_NAMES.contains(&name));
                if !is_config_name || !entry.file_type().is_some_and(|kind| kind.is_file()) {
                    continue;
                }

                let Some(dir) = entry.path().parent() else {
                    continue;
                };
                if paths::is_ancestor_of(&self.ceiling, dir) {
                    self.take_language_source(dir, &mut seen, &mut found);
                }
            }
        }

        // Deepest first, so that `language_sources` stays innermost-first and the closest
        // declaration still wins. Ties are broken by path so the order is not filesystem-dependent.
        found.sort_by(|left, right| {
            right
                .dir
                .components()
                .count()
                .cmp(&left.dir.components().count())
                .then_with(|| left.dir.cmp(&right.dir))
        });

        self.descendant_language_configs = found;
    }

    /// Load `dir`'s config into `found` unless the directory has already been accounted for.
    fn take_language_source(&self, dir: &Path, seen: &mut AHashSet<PathBuf>, found: &mut Vec<Arc<LoadedConfig>>) {
        if !seen.insert(dir.to_path_buf()) {
            return;
        }
        if let Ok(Some(loaded)) = Self::load_dir_config(dir) {
            found.push(loaded);
        }
    }

    /// The directory subtrees the requested paths can reach.
    ///
    /// A path is taken down to its literal prefix — everything before the first component holding a
    /// glob metacharacter — because the rest only filters what the walk finds. A file's own
    /// directory stands in for it. Roots contained by another root are dropped so no subtree is
    /// walked twice.
    fn language_scan_roots(root_dir: &Path, paths: &[String]) -> Vec<PathBuf> {
        let mut roots: Vec<PathBuf> = Vec::with_capacity(paths.len());
        for pattern in paths {
            // Taken apart with `Path::components` rather than by splitting on `/`: `"/a/b"` split on
            // `/` yields a leading empty segment that collects back into the *relative* `a/b`, which
            // would then be joined onto the root directory and sweep the wrong subtree entirely.
            let literal: PathBuf = Path::new(pattern)
                .components()
                .take_while(|part| {
                    !part
                        .as_os_str()
                        .to_string_lossy()
                        .contains(['*', '?', '[', ']', '{', '}'])
                })
                .collect();
            let mut root = paths::absolute_normalized(root_dir, &literal);
            if root.is_file()
                && let Some(parent) = root.parent()
            {
                root = parent.to_path_buf();
            }
            roots.push(root);
        }

        // Sorted, an enclosing directory always precedes the ones it contains.
        roots.sort();
        roots.dedup();

        let mut minimal: Vec<PathBuf> = Vec::with_capacity(roots.len());
        for root in roots {
            if !minimal.iter().any(|kept| paths::is_ancestor_of(kept, &root)) {
                minimal.push(root);
            }
        }

        minimal
    }

    /// The configs allowed to declare custom languages, innermost first so the
    /// closest one wins. A config below the root is included only once
    /// [`Self::discover_language_sources`] has been told the run will visit it; one
    /// merely stumbled upon during per-file resolution is still excluded, because by
    /// then the registry has been built and the file has already been collected or
    /// dropped.
    fn language_sources(&self) -> impl DoubleEndedIterator<Item = &Arc<LoadedConfig>> {
        self.descendant_language_configs
            .iter()
            .chain(self.ancestor_configs.iter().rev())
            .chain(self.global_config.iter())
    }

    pub fn get_language_config(&self, language_name: &str) -> Option<LanguageConfig> {
        for loaded in self.language_sources() {
            if let Some(lang_config) = loaded.config.languages.get(language_name) {
                return Some(lang_config.clone());
            }

            if let Some((_, lang_config)) = loaded
                .config
                .languages
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(language_name))
            {
                return Some(lang_config.clone());
            }
        }
        None
    }

    pub fn get_all_languages(&self) -> HashMap<String, LanguageConfig> {
        let mut languages = HashMap::new();

        for loaded in self.language_sources().rev() {
            for (name, lang_config) in &loaded.config.languages {
                languages.insert(name.clone(), lang_config.clone());
            }
        }

        languages
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An absolute input path must name itself as the subtree to sweep for `[languages]`, not a
    /// directory of the same name under the invocation directory. Splitting on `/` loses the root
    /// component, and the relative remainder would then be joined onto the root directory — so the
    /// sweep would look in a place that almost never exists and find no config at all.
    #[test]
    fn a_scan_root_from_an_absolute_path_stays_absolute() {
        let roots = ConfigManager::language_scan_roots(Path::new("/repo"), &["/elsewhere/pkg".to_string()]);
        assert_eq!(roots, vec![PathBuf::from("/elsewhere/pkg")]);
    }

    #[test]
    fn a_scan_root_stops_at_the_first_glob_component_and_drops_nested_roots() {
        let roots = ConfigManager::language_scan_roots(
            Path::new("/repo"),
            &[
                "src/**/*.rs".to_string(),
                "src/nested".to_string(),
                "other".to_string(),
                ".".to_string(),
            ],
        );
        // `.` normalizes to the root itself, which contains every other root.
        assert_eq!(roots, vec![PathBuf::from("/repo")]);

        let roots = ConfigManager::language_scan_roots(
            Path::new("/repo"),
            &["src/**/*.rs".to_string(), "src/nested".to_string(), "other".to_string()],
        );
        assert_eq!(roots, vec![PathBuf::from("/repo/other"), PathBuf::from("/repo/src")]);
    }

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

        assert!(config.validate().is_ok());

        config.languages.insert(
            "test".to_string(),
            LanguageConfig {
                name: "".to_string(),
                extensions: vec![".test".to_string()],
                comment_nodes: vec!["comment".to_string()],
                doc_comment_nodes: vec![],
                preserve_patterns: vec![],
                remove_todos: None,
                remove_fixme: None,
                remove_docs: None,
                use_default_ignores: None,
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

    /// A config built in code has no record of a file, so every flag on it is deliberate.
    #[test]
    fn a_programmatic_config_still_overrides_every_global() {
        let base = Config::default();
        let mut explicit = Config::default();
        explicit.global.use_default_ignores = false;
        explicit.global.respect_gitignore = false;

        let merged = base.merge_with(&explicit);
        assert!(!merged.global.use_default_ignores);
        assert!(!merged.global.respect_gitignore);
    }

    #[test]
    fn merging_a_parsed_config_without_a_global_section_keeps_the_outer_values() {
        let base = Config {
            global: GlobalConfig {
                remove_docs: true,
                respect_gitignore: false,
                ..Default::default()
            },
            ..Default::default()
        };

        let patterns_only: Config = toml::from_str("[patterns.\"*.py\"]\nremove_todos = true\n").unwrap();
        let partial_global: Config = toml::from_str("[global]\nremove_todos = true\n").unwrap();

        for (label, layer) in [("patterns only", patterns_only), ("partial [global]", partial_global)] {
            let merged = base.merge_with(&layer);
            assert!(merged.global.remove_docs, "{label} must not reset remove_docs");
            assert!(
                !merged.global.respect_gitignore,
                "{label} must not reset respect_gitignore"
            );
        }
    }
}
