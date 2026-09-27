//! The on-disk shape of a config file, and how one layer merges into the next.
//!
//! Discovery — which files are read, in what order — is `super::manager`; the `uncomment init`
//! generators are `super::templates`.

use crate::lint::config::LintTable;
use anyhow::{Context, Result};
use globset::{GlobBuilder, GlobMatcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

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

    /// Path globs no subcommand collects a file from. See [`super::ExcludeSet`] for how one is
    /// anchored and matched.
    pub exclude: Vec<String>,

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
    #[serde(default)]
    exclude: Vec<String>,
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
            exclude: file.exclude,
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

    /// Whole filenames this language claims, which is the only way to reach an extensionless file
    /// such as `BUILD`. An entry ending in `.*` claims every name starting with the part before the
    /// `*`. Matched case-sensitively, and before any extension rule.
    #[serde(default)]
    pub filenames: Vec<String>,

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
///
/// Shared with `[global] exclude` so that one glob dialect covers the whole config file. The caller
/// attaches the context, because the key a broken glob is reported against differs.
pub(super) fn compile_path_glob(pattern: &str) -> Result<GlobMatcher> {
    GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .map(|glob| glob.compile_matcher())
        .map_err(anyhow::Error::from)
}

fn compile_pattern_glob(pattern: &str) -> Result<GlobMatcher> {
    compile_path_glob(pattern).with_context(|| format!("Invalid glob in [patterns.\"{pattern}\"]"))
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
pub(super) struct LoadedConfig {
    pub(super) dir: PathBuf,
    /// The file it was read from, for error messages. `None` for a config assembled in code.
    pub(super) path: Option<PathBuf>,
    pub(super) config: Config,
    patterns: Vec<CompiledPattern>,
}

impl LoadedConfig {
    pub(super) fn new(dir: PathBuf, path: Option<PathBuf>, config: Config) -> Result<Self> {
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

    pub(super) fn apply_patterns(&self, file_path: &Path, resolved: &mut ResolvedConfig) {
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

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            remove_todos: false,
            remove_fixme: false,
            remove_docs: false,
            preserve_patterns: Vec::new(),
            exclude: Vec::new(),
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

            // Either list makes the declaration reachable; a language with neither claims no file at
            // all, so the whole section would be inert.
            if lang_config.extensions.is_empty() && lang_config.filenames.is_empty() {
                return Err(anyhow::anyhow!(
                    "Language '{}' has no file extensions or filenames",
                    lang_name
                ));
            }

            if lang_config.comment_nodes.is_empty() {
                return Err(anyhow::anyhow!("Language '{}' has no comment node types", lang_name));
            }
        }

        super::exclude::validate(&self.global.exclude, super::exclude::CONFIG_KEY)?;

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

        // A union, like `preserve_patterns`: an inner config adds paths the project will not touch
        // and cannot take back one an outer config already excluded.
        let mut exclude = merged.global.exclude.clone();
        exclude.extend(other.global.exclude.iter().cloned());
        exclude.sort();
        exclude.dedup();
        merged.global.exclude = exclude;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let mut config = Config::default();

        assert!(config.validate().is_ok());

        config.languages.insert(
            "test".to_string(),
            LanguageConfig {
                name: "".to_string(),
                extensions: vec![".test".to_string()],
                filenames: vec![],
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
