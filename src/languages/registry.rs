use crate::languages::config::LanguageConfig;
use std::collections::HashMap;
use std::path::Path;

pub struct LanguageRegistry {
    languages: HashMap<String, LanguageConfig>,
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
            LanguageConfig::java(),
            LanguageConfig::c(),
            LanguageConfig::cpp(),
            LanguageConfig::ruby(),
            LanguageConfig::json(),
            LanguageConfig::jsonc(),
            LanguageConfig::yaml(),
            LanguageConfig::hcl(),
            LanguageConfig::make(),
            LanguageConfig::zig(),
        ];

        for config in configs {
            self.register_language(config);
        }
    }

    pub fn register_language(&mut self, config: LanguageConfig) {
        let name = config.name.clone();

        // Map all extensions to this language
        for extension in &config.extensions {
            self.extension_map
                .insert(extension.to_lowercase(), name.clone());
        }

        self.languages.insert(name, config);
    }

    pub fn get_language(&self, name: &str) -> Option<&LanguageConfig> {
        self.languages.get(name)
    }

    pub fn detect_language(&self, file_path: &Path) -> Option<&LanguageConfig> {
        let file_name = file_path.file_name()?.to_str()?;

        // Special handling for files without extensions
        match file_name {
            "Makefile" | "makefile" | "GNUmakefile" => return self.languages.get("make"),
            _ => {}
        }

        // Special handling for .d.ts files
        if file_name.ends_with(".d.ts")
            || file_name.ends_with(".d.mts")
            || file_name.ends_with(".d.cts")
        {
            return self.languages.get("typescript");
        }

        let extension = file_path.extension()?.to_str()?.to_lowercase();
        let language_name = self.extension_map.get(&extension)?;
        self.languages.get(language_name)
    }

    pub fn detect_language_by_extension(&self, extension: &str) -> Option<&LanguageConfig> {
        let language_name = self.extension_map.get(&extension.to_lowercase())?;
        self.languages.get(language_name)
    }

    pub fn get_supported_languages(&self) -> Vec<String> {
        self.languages.keys().cloned().collect()
    }

    pub fn get_supported_extensions(&self) -> Vec<String> {
        self.extension_map.keys().cloned().collect()
    }

    pub fn is_supported_extension(&self, extension: &str) -> bool {
        self.extension_map.contains_key(&extension.to_lowercase())
    }

    pub fn is_supported_language(&self, name: &str) -> bool {
        self.languages.contains_key(name)
    }

    pub fn language_for_extension(&self, extension: &str) -> Option<String> {
        self.extension_map.get(&extension.to_lowercase()).cloned()
    }

    pub fn extensions_for_language(&self, name: &str) -> Option<Vec<String>> {
        self.languages
            .get(name)
            .map(|config| config.extensions.clone())
    }

    pub fn get_all_languages(&self) -> impl Iterator<Item = (&String, &LanguageConfig)> {
        self.languages.iter()
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

        let zig_file = PathBuf::from("scratch.zig");
        let detected = registry.detect_language(&zig_file);
        assert!(detected.is_some());
        assert_eq!(detected.unwrap().name, "zig");

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
            registry.detect_language_by_extension("zig").unwrap().name,
            "zig"
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
        assert!(languages.contains(&"ruby".to_string()));
        assert!(languages.contains(&"zig".to_string()));
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
        assert!(registry.is_supported_extension("rb"));
        assert!(registry.is_supported_extension("zig"));

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

        assert_eq!(
            registry.detect_language_by_extension("ZIG").unwrap().name,
            "zig"
        );

        assert!(registry.is_supported_extension("RS"));
        assert!(registry.is_supported_extension("PY"));
    }

    #[test]
    fn test_language_registration() {
        let mut registry = LanguageRegistry::new();
        let initial_count = registry.languages.len();

        // Create a custom language config
        let custom_config = LanguageConfig::new(
            "custom",
            vec!["cst"],
            vec!["comment"],
            vec!["doc_comment"],
            || tree_sitter_rust::LANGUAGE.into(), // Just use rust parser for testing
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

        let zig_extensions = registry.extensions_for_language("zig").unwrap();
        assert_eq!(zig_extensions, vec!["zig"]);

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
        assert!(all_languages.iter().any(|(name, _)| *name == "zig"));
    }
}
