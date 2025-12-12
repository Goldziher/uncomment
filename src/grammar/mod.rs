use crate::config::{GrammarConfig, GrammarSource};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use tree_sitter::Language;
use tree_sitter_loader::{CompileConfig, Config as LoaderConfig, Loader};

pub mod loader;

pub struct GrammarManager {
    loader: Loader,

    language_cache: HashMap<String, Language>,

    static_languages: HashMap<String, Language>,
}

impl GrammarManager {
    pub fn new() -> Result<Self> {
        let mut loader = Loader::new().context("Failed to create tree-sitter loader")?;

        #[cfg(debug_assertions)]
        loader.debug_build(true);

        let config = LoaderConfig::initial();
        loader
            .find_all_languages(&config)
            .context("Failed to initialize language configurations")?;

        let mut static_languages = HashMap::new();

        static_languages.insert("rust".to_string(), tree_sitter_rust::LANGUAGE.into());
        static_languages.insert("python".to_string(), tree_sitter_python::LANGUAGE.into());
        static_languages.insert(
            "javascript".to_string(),
            tree_sitter_javascript::LANGUAGE.into(),
        );
        static_languages.insert(
            "typescript".to_string(),
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        );
        static_languages.insert(
            "tsx".to_string(),
            tree_sitter_typescript::LANGUAGE_TSX.into(),
        );
        static_languages.insert("go".to_string(), tree_sitter_go::LANGUAGE.into());
        static_languages.insert("ruby".to_string(), tree_sitter_ruby::LANGUAGE.into());
        static_languages.insert("php".to_string(), tree_sitter_php::LANGUAGE_PHP.into());
        static_languages.insert("elixir".to_string(), tree_sitter_elixir::LANGUAGE.into());
        static_languages.insert("toml".to_string(), tree_sitter_toml_ng::LANGUAGE.into());
        static_languages.insert("csharp".to_string(), tree_sitter_c_sharp::LANGUAGE.into());
        static_languages.insert("java".to_string(), tree_sitter_java::LANGUAGE.into());
        static_languages.insert("c".to_string(), tree_sitter_c::LANGUAGE.into());
        static_languages.insert("cpp".to_string(), tree_sitter_cpp::LANGUAGE.into());
        static_languages.insert("json".to_string(), tree_sitter_json::LANGUAGE.into());
        static_languages.insert("yaml".to_string(), tree_sitter_yaml::LANGUAGE.into());
        static_languages.insert("hcl".to_string(), tree_sitter_hcl::LANGUAGE.into());
        static_languages.insert("make".to_string(), tree_sitter_make::LANGUAGE.into());
        static_languages.insert("shell".to_string(), tree_sitter_bash::LANGUAGE.into());

        Ok(Self {
            loader,
            language_cache: HashMap::new(),
            static_languages,
        })
    }

    pub fn get_language(
        &mut self,
        language_name: &str,
        grammar_config: &GrammarConfig,
    ) -> Result<Language> {
        if let Some(language) = self.language_cache.get(language_name) {
            return Ok(language.clone());
        }

        let language = match &grammar_config.source {
            GrammarSource::Builtin => self
                .get_builtin_language(language_name)
                .with_context(|| format!("Built-in language '{language_name}' not found"))?,
            GrammarSource::Git { url, branch, path } => self
                .load_git_language(language_name, url, branch.as_deref(), path.as_deref())
                .with_context(|| format!("Failed to load Git grammar for '{language_name}'"))?,
            GrammarSource::Local { path } => self
                .load_local_language(language_name, path)
                .with_context(|| format!("Failed to load local grammar for '{language_name}'"))?,
            GrammarSource::Library { path } => self
                .load_library_language(language_name, path)
                .with_context(|| format!("Failed to load library grammar for '{language_name}'"))?,
        };

        self.language_cache
            .insert(language_name.to_string(), language.clone());

        Ok(language)
    }

    fn get_builtin_language(&self, language_name: &str) -> Result<Language> {
        let language = self
            .static_languages
            .get(language_name)
            .ok_or_else(|| anyhow::anyhow!("No built-in language '{}'", language_name))?;

        Ok(language.clone())
    }

    fn load_git_language(
        &mut self,
        language_name: &str,
        url: &str,
        branch: Option<&str>,
        subpath: Option<&str>,
    ) -> Result<Language> {
        let git_loader =
            loader::GitGrammarLoader::new().context("Failed to create Git grammar loader")?;

        git_loader
            .load_git_grammar(language_name, url, branch, subpath)
            .with_context(|| {
                format!("Failed to load Git grammar for '{language_name}' from '{url}'")
            })
    }

    fn load_local_language(&mut self, _language_name: &str, path: &Path) -> Result<Language> {
        if !path.exists() {
            anyhow::bail!("Grammar path does not exist: {}", path.display());
        }

        let grammar_js = path.join("grammar.js");
        if grammar_js.exists() {
            let compile_config = CompileConfig::new(path, None, None);

            self.loader
                .load_language_at_path(compile_config)
                .with_context(|| {
                    format!("Failed to compile and load grammar from {}", path.display())
                })
        } else {
            let compile_config = CompileConfig::new(path, None, None);

            self.loader
                .load_language_at_path(compile_config)
                .with_context(|| format!("Failed to load language from {}", path.display()))
        }
    }

    fn load_library_language(&mut self, _language_name: &str, path: &Path) -> Result<Language> {
        if !path.exists() {
            anyhow::bail!("Library path does not exist: {}", path.display());
        }

        use libloading::{Library, Symbol};

        unsafe {
            let lib = Library::new(path)
                .with_context(|| format!("Failed to load library from {}", path.display()))?;

            let symbol_names = ["tree_sitter_language", "tree_sitter", "language"];

            for symbol_name in &symbol_names {
                if let Ok(func) = lib.get::<Symbol<
                    unsafe extern "C" fn() -> *const tree_sitter::ffi::TSLanguage,
                >>(symbol_name.as_bytes())
                {
                    let ts_language_ptr = func();
                    let language = Language::from_raw(ts_language_ptr);

                    std::mem::forget(lib);

                    return Ok(language);
                }
            }

            anyhow::bail!(
                "No valid tree-sitter language function found in library {}",
                path.display()
            );
        }
    }

    #[cfg(test)]
    pub fn builtin_languages(&self) -> Vec<String> {
        self.static_languages.keys().cloned().collect()
    }

    #[cfg(test)]
    pub fn clear_cache(&mut self) {
        self.language_cache.clear();
    }
}

impl Default for GrammarManager {
    fn default() -> Self {
        Self::new().expect("Failed to create default GrammarManager")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_grammar_manager_creation() {
        let manager = GrammarManager::new();
        assert!(manager.is_ok());
    }

    #[test]
    fn test_builtin_languages() {
        let manager = GrammarManager::new().unwrap();
        let languages = manager.builtin_languages();

        assert!(languages.contains(&"rust".to_string()));
        assert!(languages.contains(&"python".to_string()));
        assert!(languages.contains(&"javascript".to_string()));
        assert!(languages.contains(&"typescript".to_string()));
        assert!(languages.contains(&"go".to_string()));
        assert!(languages.contains(&"ruby".to_string()));
        assert!(languages.contains(&"php".to_string()));
        assert!(languages.contains(&"elixir".to_string()));
        assert!(languages.contains(&"toml".to_string()));
        assert!(languages.contains(&"csharp".to_string()));
        assert!(languages.contains(&"java".to_string()));
        assert!(languages.contains(&"c".to_string()));
        assert!(languages.contains(&"cpp".to_string()));
        assert!(languages.contains(&"json".to_string()));
        assert!(languages.contains(&"yaml".to_string()));
        assert!(languages.contains(&"hcl".to_string()));
        assert!(languages.contains(&"make".to_string()));
        assert!(languages.contains(&"shell".to_string()));
    }

    #[test]
    fn test_get_builtin_language() {
        let mut manager = GrammarManager::new().unwrap();
        let config = GrammarConfig::default();

        let rust_lang = manager.get_language("rust", &config);
        assert!(rust_lang.is_ok());

        let python_lang = manager.get_language("python", &config);
        assert!(python_lang.is_ok());

        let js_lang = manager.get_language("javascript", &config);
        assert!(js_lang.is_ok());
    }

    #[test]
    fn test_nonexistent_builtin_language() {
        let mut manager = GrammarManager::new().unwrap();
        let config = GrammarConfig::default();

        let result = manager.get_language("nonexistent", &config);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Built-in language 'nonexistent' not found")
        );
    }

    #[test]
    fn test_language_caching() {
        let mut manager = GrammarManager::new().unwrap();
        let config = GrammarConfig::default();

        let rust_lang1 = manager.get_language("rust", &config).unwrap();
        let rust_lang2 = manager.get_language("rust", &config).unwrap();

        assert_eq!(rust_lang1.abi_version(), rust_lang2.abi_version());

        manager.clear_cache();
        let rust_lang3 = manager.get_language("rust", &config).unwrap();
        assert_eq!(rust_lang1.abi_version(), rust_lang3.abi_version());
    }

    #[test]
    fn test_local_grammar_invalid_path() {
        let mut manager = GrammarManager::new().unwrap();
        let config = GrammarConfig {
            source: GrammarSource::Local {
                path: "/nonexistent/path".into(),
            },
            ..Default::default()
        };

        let result = manager.get_language("test", &config);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("Grammar path does not exist")
                || error_msg.contains("Failed to load local grammar")
        );
    }

    #[test]
    fn test_local_grammar_no_grammar_js() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = GrammarManager::new().unwrap();
        let config = GrammarConfig {
            source: GrammarSource::Local {
                path: temp_dir.path().to_path_buf(),
            },
            ..Default::default()
        };

        let result = manager.get_language("test", &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_library_grammar_invalid_path() {
        let mut manager = GrammarManager::new().unwrap();
        let config = GrammarConfig {
            source: GrammarSource::Library {
                path: "/nonexistent/library.so".into(),
            },
            ..Default::default()
        };

        let result = manager.get_language("test", &config);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("Library path does not exist")
                || error_msg.contains("Failed to load library grammar")
        );
    }

    #[test]
    fn test_git_grammar_configuration() {
        let mut manager = GrammarManager::new().unwrap();
        let config = GrammarConfig {
            source: GrammarSource::Git {
                url: "https://github.com/tree-sitter/tree-sitter-rust".to_string(),
                branch: Some("master".to_string()),
                path: None,
            },
            ..Default::default()
        };

        let result = manager.get_language("rust", &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_grammar_source_defaults() {
        let default_source = GrammarSource::default();
        assert!(matches!(default_source, GrammarSource::Builtin));

        let default_config = GrammarConfig::default();
        assert!(matches!(default_config.source, GrammarSource::Builtin));
        assert!(default_config.version.is_none());
        assert!(default_config.library_path.is_none());
        assert!(default_config.compile_flags.is_empty());
    }

    #[test]
    fn test_all_builtin_languages_loadable() {
        let mut manager = GrammarManager::new().unwrap();
        let config = GrammarConfig::default();
        let languages = manager.builtin_languages();

        for language in languages {
            let result = manager.get_language(&language, &config);
            assert!(
                result.is_ok(),
                "Failed to load builtin language: {language}"
            );
        }
    }
}
