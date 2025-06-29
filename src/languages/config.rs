use std::collections::HashSet;
use tree_sitter::Language;

#[derive(Debug, Clone)]
pub struct LanguageConfig {
    pub name: String,
    pub extensions: Vec<String>,
    pub comment_types: Vec<String>,
    pub doc_comment_types: Vec<String>,
    pub tree_sitter_lang: fn() -> Language,
}

impl LanguageConfig {
    pub fn new(
        name: &str,
        extensions: Vec<&str>,
        comment_types: Vec<&str>,
        doc_comment_types: Vec<&str>,
        tree_sitter_lang: fn() -> Language,
    ) -> Self {
        Self {
            name: name.to_string(),
            extensions: extensions.iter().map(|&s| s.to_string()).collect(),
            comment_types: comment_types.iter().map(|&s| s.to_string()).collect(),
            doc_comment_types: doc_comment_types.iter().map(|&s| s.to_string()).collect(),
            tree_sitter_lang,
        }
    }

    pub fn supports_extension(&self, extension: &str) -> bool {
        self.extensions.contains(&extension.to_lowercase())
    }

    pub fn is_comment_type(&self, node_type: &str) -> bool {
        self.comment_types.contains(&node_type.to_string())
    }

    pub fn is_doc_comment_type(&self, node_type: &str) -> bool {
        self.doc_comment_types.contains(&node_type.to_string())
    }

    pub fn get_comment_types(&self) -> &[String] {
        &self.comment_types
    }

    pub fn get_doc_comment_types(&self) -> &[String] {
        &self.doc_comment_types
    }

    pub fn tree_sitter_language(&self) -> Language {
        (self.tree_sitter_lang)()
    }

    pub fn get_all_comment_types(&self) -> HashSet<String> {
        let mut types = HashSet::new();
        types.extend(self.comment_types.iter().cloned());
        types.extend(self.doc_comment_types.iter().cloned());
        types
    }
}

// Language-specific configurations
impl LanguageConfig {
    pub fn rust() -> Self {
        Self::new(
            "rust",
            vec!["rs"],
            vec!["line_comment", "block_comment"],
            vec!["doc_comment", "inner_doc_comment", "outer_doc_comment"],
            || tree_sitter_rust::LANGUAGE.into(),
        )
    }

    pub fn python() -> Self {
        Self::new(
            "python",
            vec!["py", "pyw", "pyi", "pyx", "pxd"],
            vec!["comment"],
            vec!["string"], // Python uses strings for docstrings
            || tree_sitter_python::LANGUAGE.into(),
        )
    }

    pub fn javascript() -> Self {
        Self::new(
            "javascript",
            vec!["js", "jsx", "mjs", "cjs"],
            vec!["comment"],
            vec!["comment"], // JSDoc comments are still comments
            || tree_sitter_javascript::LANGUAGE.into(),
        )
    }

    pub fn typescript() -> Self {
        Self::new(
            "typescript",
            vec!["ts", "mts", "cts", "d.ts", "d.mts", "d.cts"],
            vec!["comment"],
            vec!["comment"], // TSDoc comments are still comments
            || tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        )
    }

    pub fn tsx() -> Self {
        Self::new(
            "tsx",
            vec!["tsx"],
            vec!["comment"],
            vec!["comment"], // TSDoc comments are still comments
            || tree_sitter_typescript::LANGUAGE_TSX.into(),
        )
    }

    pub fn go() -> Self {
        Self::new(
            "go",
            vec!["go"],
            vec!["comment"],
            vec!["comment"], // Go doc comments are regular comments
            || tree_sitter_go::LANGUAGE.into(),
        )
    }

    pub fn java() -> Self {
        Self::new(
            "java",
            vec!["java"],
            vec!["line_comment", "block_comment"],
            vec!["block_comment"], // Javadoc comments
            || tree_sitter_java::LANGUAGE.into(),
        )
    }

    pub fn c() -> Self {
        Self::new(
            "c",
            vec!["c", "h"],
            vec!["comment"],
            vec!["comment"], // Doxygen comments
            || tree_sitter_c::LANGUAGE.into(),
        )
    }

    pub fn cpp() -> Self {
        Self::new(
            "cpp",
            vec!["cpp", "cxx", "cc", "c++", "hpp", "hxx", "hh", "h++"],
            vec!["comment"],
            vec!["comment"], // Doxygen comments
            || tree_sitter_cpp::LANGUAGE.into(),
        )
    }

    pub fn ruby() -> Self {
        Self::new(
            "ruby",
            vec!["rb", "rbw"],
            vec!["comment"],
            vec!["comment"], // YARD documentation comments
            || tree_sitter_ruby::LANGUAGE.into(),
        )
    }

    pub fn json() -> Self {
        Self::new(
            "json",
            vec!["json"],
            vec![], // JSON doesn't support comments officially
            vec![],
            || tree_sitter_json::LANGUAGE.into(),
        )
    }

    pub fn jsonc() -> Self {
        Self::new(
            "jsonc",
            vec!["jsonc"],
            vec!["comment"], // JSON with Comments
            vec![],
            || tree_sitter_json::LANGUAGE.into(), // Uses same parser as JSON
        )
    }

    pub fn yaml() -> Self {
        Self::new("yaml", vec!["yaml", "yml"], vec!["comment"], vec![], || {
            tree_sitter_yaml::LANGUAGE.into()
        })
    }

    pub fn hcl() -> Self {
        Self::new(
            "hcl",
            vec!["hcl", "tf", "tfvars"],
            vec!["comment"], // Both # and // style comments
            vec![],
            || tree_sitter_hcl::LANGUAGE.into(),
        )
    }

    pub fn make() -> Self {
        Self::new(
            "make",
            vec!["mk"], // Will need special handling for Makefile (no extension)
            vec!["comment"],
            vec![],
            || tree_sitter_make::LANGUAGE.into(),
        )
    }

    pub fn shell() -> Self {
        Self::new(
            "shell",
            vec!["sh", "bash", "zsh"],
            vec!["comment"],
            vec!["comment"],
            || tree_sitter_bash::LANGUAGE.into(),
        )
    }

    pub fn haskell() -> Self {
        Self::new(
            "haskell",
            vec!["hs"],
            vec!["comment"],
            vec!["haddock"],
            || tree_sitter_haskell::LANGUAGE.into(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_config_creation() {
        let config = LanguageConfig::rust();
        assert_eq!(config.name, "rust");
        assert!(config.supports_extension("rs"));
        assert!(!config.supports_extension("py"));
        assert!(config.is_comment_type("line_comment"));
        assert!(config.is_doc_comment_type("doc_comment"));
    }

    #[test]
    fn test_extension_support() {
        let rust_config = LanguageConfig::rust();
        assert!(rust_config.supports_extension("rs"));
        assert!(rust_config.supports_extension("RS")); // Case insensitive

        let python_config = LanguageConfig::python();
        assert!(python_config.supports_extension("py"));
        assert!(python_config.supports_extension("pyw"));
        assert!(python_config.supports_extension("pyi"));
    }

    #[test]
    fn test_comment_type_detection() {
        let rust_config = LanguageConfig::rust();
        assert!(rust_config.is_comment_type("line_comment"));
        assert!(rust_config.is_comment_type("block_comment"));
        assert!(!rust_config.is_comment_type("function"));

        assert!(rust_config.is_doc_comment_type("doc_comment"));
        assert!(!rust_config.is_doc_comment_type("line_comment"));
    }

    #[test]
    fn test_all_comment_types() {
        let rust_config = LanguageConfig::rust();
        let all_types = rust_config.get_all_comment_types();
        assert!(all_types.contains("line_comment"));
        assert!(all_types.contains("block_comment"));
        assert!(all_types.contains("doc_comment"));
        assert!(all_types.contains("inner_doc_comment"));
        assert!(all_types.contains("outer_doc_comment"));
    }

    #[test]
    fn test_language_specific_configs() {
        let languages = vec![
            LanguageConfig::rust(),
            LanguageConfig::python(),
            LanguageConfig::javascript(),
            LanguageConfig::typescript(),
            LanguageConfig::go(),
            LanguageConfig::java(),
            LanguageConfig::c(),
            LanguageConfig::cpp(),
            LanguageConfig::ruby(),
            LanguageConfig::shell(),
            LanguageConfig::haskell(),
        ];

        for lang in languages {
            assert!(!lang.name.is_empty());
            assert!(!lang.extensions.is_empty());
            assert!(!lang.comment_types.is_empty());
        }
    }
}
