use crate::languages::config::LanguageConfig;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

pub struct LanguageRegistry {
    languages: HashMap<String, Arc<LanguageConfig>>,
    extension_map: HashMap<String, String>,
}

impl LanguageRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            languages: HashMap::new(),
            extension_map: HashMap::new(),
        };

        registry.register_default_languages();
        registry
    }

    fn register_default_languages(&mut self) {
        let configs = vec![
            LanguageConfig::rust(),
            LanguageConfig::python(),
            LanguageConfig::javascript(),
            LanguageConfig::typescript(),
            LanguageConfig::tsx(),
            LanguageConfig::go(),
            LanguageConfig::ruby(),
            LanguageConfig::php(),
            LanguageConfig::elixir(),
            LanguageConfig::toml(),
            LanguageConfig::csharp(),
            LanguageConfig::java(),
            LanguageConfig::c(),
            LanguageConfig::cpp(),
            LanguageConfig::json(),
            LanguageConfig::jsonc(),
            LanguageConfig::yaml(),
            LanguageConfig::hcl(),
            LanguageConfig::make(),
            LanguageConfig::shell(),
        ];

        for config in configs {
            self.register_language(config);
        }
    }

    pub fn register_language(&mut self, config: LanguageConfig) {
        let config = Arc::new(config);
        let name_lower = config.name.to_lowercase();

        for extension in &config.extensions {
            let normalized_ext = extension.trim_start_matches('.').to_lowercase();
            self.extension_map
                .insert(normalized_ext, name_lower.clone());
        }

        self.languages.insert(name_lower, config);
    }

    pub fn get_language(&self, name: &str) -> Option<&LanguageConfig> {
        self.languages.get(&name.to_lowercase()).map(Arc::as_ref)
    }

    #[must_use]
    pub fn get_language_arc(&self, name: &str) -> Option<Arc<LanguageConfig>> {
        self.languages.get(&name.to_lowercase()).cloned()
    }

    pub fn detect_language(&self, file_path: &Path) -> Option<&LanguageConfig> {
        let language_name = self.detect_language_name(file_path)?;
        self.languages.get(language_name).map(Arc::as_ref)
    }

    #[must_use]
    pub fn detect_language_arc(&self, file_path: &Path) -> Option<Arc<LanguageConfig>> {
        let language_name = self.detect_language_name(file_path)?;
        self.languages.get(language_name).cloned()
    }

    fn detect_language_name(&self, file_path: &Path) -> Option<&str> {
        let file_name = file_path.file_name()?.to_str()?;

        match file_name {
            "Makefile" | "makefile" | "GNUmakefile" => return Some("make"),
            _ => {}
        }

        if file_name.ends_with(".d.ts")
            || file_name.ends_with(".d.mts")
            || file_name.ends_with(".d.cts")
        {
            return Some("typescript");
        }

        match file_name {
            "bashrc" | ".bashrc" | "zshrc" | ".zshrc" | "zshenv" | ".zshenv" => {
                return Some("shell");
            }
            _ => {}
        }

        let extension = file_path.extension()?.to_str()?.to_lowercase();
        self.extension_map.get(&extension).map(String::as_str)
    }

    pub fn detect_language_by_extension(&self, extension: &str) -> Option<&LanguageConfig> {
        let normalized_ext = extension.trim_start_matches('.').to_lowercase();
        let language_name = self.extension_map.get(&normalized_ext)?;
        self.languages.get(language_name).map(Arc::as_ref)
    }

    pub fn get_supported_languages(&self) -> Vec<String> {
        self.languages.keys().cloned().collect()
    }

    pub fn get_supported_extensions(&self) -> Vec<String> {
        self.extension_map.keys().cloned().collect()
    }

    pub fn is_supported_extension(&self, extension: &str) -> bool {
        let normalized_ext = extension.trim_start_matches('.').to_lowercase();
        self.extension_map.contains_key(&normalized_ext)
    }

    pub fn is_supported_language(&self, name: &str) -> bool {
        self.languages.contains_key(&name.to_lowercase())
    }

    pub fn language_for_extension(&self, extension: &str) -> Option<String> {
        let normalized_ext = extension.trim_start_matches('.').to_lowercase();
        self.extension_map.get(&normalized_ext).cloned()
    }

    pub fn extensions_for_language(&self, name: &str) -> Option<Vec<String>> {
        self.get_language(name)
            .map(|config| config.extensions.clone())
    }

    pub fn get_all_languages(&self) -> impl Iterator<Item = (&String, &LanguageConfig)> {
        self.languages
            .iter()
            .map(|(name, config)| (name, config.as_ref()))
    }

    pub fn register_configured_languages(
        &mut self,
        config_languages: &std::collections::HashMap<String, crate::config::LanguageConfig>,
    ) {
        for config in config_languages.values() {
            let name_lower = config.name.to_lowercase();
            if let Some(existing_config) = self.languages.get(&name_lower) {
                let updated_config = LanguageConfig {
                    name: config.name.clone(),
                    extensions: config.extensions.clone(),
                    comment_types: config.comment_nodes.clone(),
                    doc_comment_types: config.doc_comment_nodes.clone(),
                    tree_sitter_lang: existing_config.tree_sitter_lang,
                };
                self.register_language(updated_config);
            } else {
                let language_config = LanguageConfig {
                    name: config.name.clone(),
                    extensions: config.extensions.clone(),
                    comment_types: config.comment_nodes.clone(),
                    doc_comment_types: config.doc_comment_nodes.clone(),
                    tree_sitter_lang: || unsafe {
                        tree_sitter::Language::from_raw(std::ptr::null())
                    },
                };
                self.register_language(language_config);
            }
        }
    }
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_registry_creation() {
        let registry = LanguageRegistry::new();
        assert!(!registry.languages.is_empty());
        assert!(!registry.extension_map.is_empty());
    }

    #[test]
    fn test_language_detection_by_path() {
        let registry = LanguageRegistry::new();

        let rust_file = PathBuf::from("src/main.rs");
        let detected = registry.detect_language(&rust_file);
        assert!(detected.is_some());
        assert_eq!(detected.unwrap().name, "rust");

        let python_file = PathBuf::from("script.py");
        let detected = registry.detect_language(&python_file);
        assert!(detected.is_some());
        assert_eq!(detected.unwrap().name, "python");

        let go_file = PathBuf::from("main.go");
        let detected = registry.detect_language(&go_file);
        assert!(detected.is_some());
        assert_eq!(detected.unwrap().name, "go");

        let unknown_file = PathBuf::from("file.unknown");
        let detected = registry.detect_language(&unknown_file);
        assert!(detected.is_none());
    }

    #[test]
    fn test_language_detection_by_extension() {
        let registry = LanguageRegistry::new();

        assert_eq!(
            registry.detect_language_by_extension("rs").unwrap().name,
            "rust"
        );
        assert_eq!(
            registry.detect_language_by_extension("py").unwrap().name,
            "python"
        );
        assert_eq!(
            registry.detect_language_by_extension("js").unwrap().name,
            "javascript"
        );
        assert_eq!(
            registry.detect_language_by_extension("ts").unwrap().name,
            "typescript"
        );

        assert_eq!(
            registry.detect_language_by_extension("sh").unwrap().name,
            "shell"
        );

        assert_eq!(
            registry.detect_language_by_extension("bash").unwrap().name,
            "shell"
        );

        assert_eq!(
            registry.detect_language_by_extension("zsh").unwrap().name,
            "shell"
        );

        assert_eq!(
            registry.detect_language_by_extension("go").unwrap().name,
            "go"
        );

        assert!(registry.detect_language_by_extension("unknown").is_none());
    }

    #[test]
    fn test_supported_languages() {
        let registry = LanguageRegistry::new();
        let languages = registry.get_supported_languages();

        assert!(languages.contains(&"rust".to_string()));
        assert!(languages.contains(&"python".to_string()));
        assert!(languages.contains(&"javascript".to_string()));
        assert!(languages.contains(&"typescript".to_string()));
        assert!(languages.contains(&"go".to_string()));
        assert!(languages.contains(&"java".to_string()));
        assert!(languages.contains(&"c".to_string()));
        assert!(languages.contains(&"cpp".to_string()));
        assert!(languages.contains(&"shell".to_string()));
    }

    #[test]
    fn test_supported_extensions() {
        let registry = LanguageRegistry::new();

        assert!(registry.is_supported_extension("rs"));
        assert!(registry.is_supported_extension("py"));
        assert!(registry.is_supported_extension("js"));
        assert!(registry.is_supported_extension("ts"));
        assert!(registry.is_supported_extension("go"));
        assert!(registry.is_supported_extension("java"));
        assert!(registry.is_supported_extension("c"));
        assert!(registry.is_supported_extension("cpp"));
        assert!(registry.is_supported_extension("sh"));
        assert!(registry.is_supported_extension("bash"));
        assert!(registry.is_supported_extension("zsh"));

        // C/C++ headers are supported, but we preserve important header guard comments.
        assert!(registry.is_supported_extension("h"));
        assert!(registry.is_supported_extension("hpp"));
        assert!(registry.is_supported_extension("hh"));
        assert!(registry.is_supported_extension("hxx"));
        assert!(registry.is_supported_extension("h++"));

        assert!(!registry.is_supported_extension("unknown"));
    }

    #[test]
    fn test_case_insensitive_extension_detection() {
        let registry = LanguageRegistry::new();

        assert_eq!(
            registry.detect_language_by_extension("RS").unwrap().name,
            "rust"
        );
        assert_eq!(
            registry.detect_language_by_extension("PY").unwrap().name,
            "python"
        );

        assert!(registry.is_supported_extension("RS"));
        assert!(registry.is_supported_extension("PY"));
    }

    #[test]
    fn test_language_registration() {
        let mut registry = LanguageRegistry::new();
        let initial_count = registry.languages.len();

        let custom_config = LanguageConfig::new(
            "custom",
            vec!["cst"],
            vec!["comment"],
            vec!["doc_comment"],
            || tree_sitter_rust::LANGUAGE.into(),
        );

        registry.register_language(custom_config);

        assert_eq!(registry.languages.len(), initial_count + 1);
        assert!(registry.is_supported_language("custom"));
        assert!(registry.is_supported_extension("cst"));
        assert_eq!(
            registry.language_for_extension("cst"),
            Some("custom".to_string())
        );
    }

    #[test]
    fn test_extensions_for_language() {
        let registry = LanguageRegistry::new();

        let rust_extensions = registry.extensions_for_language("rust").unwrap();
        assert_eq!(rust_extensions, vec!["rs"]);

        let go_extensions = registry.extensions_for_language("go").unwrap();
        assert_eq!(go_extensions, vec!["go"]);

        let python_extensions = registry.extensions_for_language("python").unwrap();
        assert!(python_extensions.contains(&"py".to_string()));
        assert!(python_extensions.contains(&"pyw".to_string()));
        assert!(python_extensions.contains(&"pyi".to_string()));

        assert!(registry.extensions_for_language("unknown").is_none());
    }

    #[test]
    fn test_get_all_languages() {
        let registry = LanguageRegistry::new();
        let all_languages: Vec<_> = registry.get_all_languages().collect();

        assert!(!all_languages.is_empty());
        assert!(all_languages.iter().any(|(name, _)| *name == "rust"));
        assert!(all_languages.iter().any(|(name, _)| *name == "python"));
    }
}
