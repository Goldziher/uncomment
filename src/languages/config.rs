use ahash::AHashSet;

#[derive(Debug, Clone)]
pub struct LanguageConfig {
    pub name: String,
    pub extensions: Vec<String>,
    pub comment_types: Vec<String>,
    pub doc_comment_types: Vec<String>,
    pub tslp_name: String,
}

impl LanguageConfig {
    pub fn new(
        name: &str,
        extensions: Vec<&str>,
        comment_types: Vec<&str>,
        doc_comment_types: Vec<&str>,
        tslp_name: &str,
    ) -> Self {
        Self {
            name: name.to_string(),
            extensions: extensions.iter().map(|&s| s.to_string()).collect(),
            comment_types: comment_types.iter().map(|&s| s.to_string()).collect(),
            doc_comment_types: doc_comment_types.iter().map(|&s| s.to_string()).collect(),
            tslp_name: tslp_name.to_string(),
        }
    }

    pub fn supports_extension(&self, extension: &str) -> bool {
        self.extensions
            .iter()
            .any(|configured| configured.eq_ignore_ascii_case(extension))
    }

    pub fn is_comment_type(&self, node_type: &str) -> bool {
        self.comment_types
            .iter()
            .any(|configured| configured == node_type)
    }

    pub fn is_doc_comment_type(&self, node_type: &str) -> bool {
        self.doc_comment_types
            .iter()
            .any(|configured| configured == node_type)
    }

    pub fn get_comment_types(&self) -> &[String] {
        &self.comment_types
    }

    pub fn get_doc_comment_types(&self) -> &[String] {
        &self.doc_comment_types
    }

    pub fn get_all_comment_types(&self) -> AHashSet<String> {
        let mut types = AHashSet::new();
        types.extend(self.comment_types.iter().cloned());
        types.extend(self.doc_comment_types.iter().cloned());
        types
    }
}

impl LanguageConfig {
    pub fn rust() -> Self {
        Self::new(
            "rust",
            vec!["rs"],
            vec!["line_comment", "block_comment"],
            vec!["doc_comment", "inner_doc_comment", "outer_doc_comment"],
            "rust",
        )
    }

    pub fn python() -> Self {
        Self::new(
            "python",
            vec!["py", "pyw", "pyi", "pyx", "pxd"],
            vec!["comment"],
            vec!["string"],
            "python",
        )
    }

    pub fn javascript() -> Self {
        Self::new(
            "javascript",
            vec!["js", "jsx", "mjs", "cjs"],
            vec!["comment"],
            vec!["comment"],
            "javascript",
        )
    }

    pub fn typescript() -> Self {
        Self::new(
            "typescript",
            vec!["ts", "mts", "cts", "d.ts", "d.mts", "d.cts"],
            vec!["comment"],
            vec!["comment"],
            "typescript",
        )
    }

    pub fn tsx() -> Self {
        Self::new("tsx", vec!["tsx"], vec!["comment"], vec!["comment"], "tsx")
    }

    pub fn go() -> Self {
        Self::new("go", vec!["go"], vec!["comment"], vec!["comment"], "go")
    }

    pub fn ruby() -> Self {
        Self::new(
            "ruby",
            vec!["rb", "rbw", "gemspec", "rake"],
            vec!["comment"],
            vec![],
            "ruby",
        )
    }

    pub fn php() -> Self {
        Self::new("php", vec!["php", "phtml"], vec!["comment"], vec![], "php")
    }

    pub fn elixir() -> Self {
        Self::new(
            "elixir",
            vec!["ex", "exs"],
            vec!["comment"],
            vec![],
            "elixir",
        )
    }

    pub fn toml() -> Self {
        Self::new("toml", vec!["toml"], vec!["comment"], vec![], "toml")
    }

    pub fn csharp() -> Self {
        Self::new("csharp", vec!["cs"], vec!["comment"], vec![], "c_sharp")
    }

    pub fn java() -> Self {
        Self::new(
            "java",
            vec!["java"],
            vec!["line_comment", "block_comment"],
            vec!["block_comment"],
            "java",
        )
    }

    pub fn c() -> Self {
        Self::new("c", vec!["c", "h"], vec!["comment"], vec!["comment"], "c")
    }

    pub fn cpp() -> Self {
        Self::new(
            "cpp",
            vec!["cpp", "cxx", "cc", "c++", "hpp", "hxx", "hh", "h++"],
            vec!["comment"],
            vec!["comment"],
            "cpp",
        )
    }

    pub fn json() -> Self {
        Self::new("json", vec!["json"], vec![], vec![], "json")
    }

    pub fn jsonc() -> Self {
        Self::new("jsonc", vec!["jsonc"], vec!["comment"], vec![], "json")
    }

    pub fn yaml() -> Self {
        Self::new("yaml", vec!["yaml", "yml"], vec!["comment"], vec![], "yaml")
    }

    pub fn hcl() -> Self {
        Self::new(
            "hcl",
            vec!["hcl", "tf", "tfvars"],
            vec!["comment"],
            vec![],
            "hcl",
        )
    }

    pub fn make() -> Self {
        Self::new("make", vec!["mk"], vec!["comment"], vec![], "make")
    }

    pub fn shell() -> Self {
        Self::new(
            "shell",
            vec!["sh", "bash", "zsh"],
            vec!["comment"],
            vec!["comment"],
            "bash",
        )
    }

    pub fn haskell() -> Self {
        Self::new(
            "haskell",
            vec!["hs", "lhs"],
            vec!["comment"],
            vec![],
            "haskell",
        )
    }

    pub fn html() -> Self {
        Self::new(
            "html",
            vec!["html", "htm", "xhtml"],
            vec!["comment"],
            vec![],
            "html",
        )
    }

    pub fn css() -> Self {
        Self::new("css", vec!["css"], vec!["comment"], vec![], "css")
    }

    pub fn xml() -> Self {
        Self::new(
            "xml",
            vec!["xml", "xsd", "xsl", "xslt", "svg"],
            vec!["Comment"],
            vec![],
            "xml",
        )
    }

    pub fn sql() -> Self {
        Self::new("sql", vec!["sql"], vec!["comment"], vec![], "sql")
    }

    pub fn kotlin() -> Self {
        Self::new(
            "kotlin",
            vec!["kt", "kts"],
            vec!["line_comment", "block_comment"],
            vec![],
            "kotlin",
        )
    }

    pub fn swift() -> Self {
        Self::new(
            "swift",
            vec!["swift"],
            vec!["comment", "multiline_comment"],
            vec![],
            "swift",
        )
    }

    pub fn lua() -> Self {
        Self::new("lua", vec!["lua"], vec!["comment"], vec![], "lua")
    }

    pub fn nix() -> Self {
        Self::new("nix", vec!["nix"], vec!["comment"], vec![], "nix")
    }

    pub fn powershell() -> Self {
        Self::new(
            "powershell",
            vec!["ps1", "psm1", "psd1"],
            vec!["comment"],
            vec![],
            "powershell",
        )
    }

    pub fn proto() -> Self {
        Self::new("proto", vec!["proto"], vec!["comment"], vec![], "proto")
    }

    pub fn ini() -> Self {
        Self::new(
            "ini",
            vec!["ini", "cfg", "conf"],
            vec!["comment"],
            vec![],
            "ini",
        )
    }

    pub fn dockerfile() -> Self {
        Self::new("dockerfile", vec![], vec!["comment"], vec![], "dockerfile")
    }

    pub fn scala() -> Self {
        Self::new(
            "scala",
            vec!["scala", "sc"],
            vec!["comment", "block_comment"],
            vec!["block_comment"],
            "scala",
        )
    }

    pub fn dart() -> Self {
        Self::new(
            "dart",
            vec!["dart"],
            vec!["comment"],
            vec!["documentation_comment"],
            "dart",
        )
    }

    pub fn r() -> Self {
        Self::new("r", vec!["r", "R"], vec!["comment"], vec![], "r")
    }

    pub fn julia() -> Self {
        Self::new("julia", vec!["jl"], vec!["line_comment"], vec![], "julia")
    }

    pub fn zig() -> Self {
        Self::new("zig", vec!["zig"], vec!["line_comment"], vec![], "zig")
    }

    pub fn clojure() -> Self {
        Self::new(
            "clojure",
            vec!["clj", "cljs", "cljc", "edn"],
            vec!["comment"],
            vec![],
            "clojure",
        )
    }

    pub fn elm() -> Self {
        Self::new(
            "elm",
            vec!["elm"],
            vec!["line_comment", "block_comment"],
            vec![],
            "elm",
        )
    }

    pub fn erlang() -> Self {
        Self::new(
            "erlang",
            vec!["erl", "hrl"],
            vec!["comment"],
            vec![],
            "erlang",
        )
    }

    pub fn vue() -> Self {
        Self::new("vue", vec!["vue"], vec!["comment"], vec![], "vue")
    }

    pub fn svelte() -> Self {
        Self::new("svelte", vec!["svelte"], vec!["comment"], vec![], "svelte")
    }

    pub fn scss() -> Self {
        Self::new(
            "scss",
            vec!["scss"],
            vec!["comment", "js_comment"],
            vec![],
            "scss",
        )
    }

    pub fn latex() -> Self {
        Self::new(
            "latex",
            vec!["tex", "sty", "cls"],
            vec!["line_comment"],
            vec![],
            "latex",
        )
    }

    pub fn fish() -> Self {
        Self::new("fish", vec!["fish"], vec!["comment"], vec![], "fish")
    }

    pub fn perl() -> Self {
        Self::new("perl", vec!["pl", "pm"], vec!["comment"], vec![], "perl")
    }

    pub fn groovy() -> Self {
        Self::new(
            "groovy",
            vec!["groovy", "gradle"],
            vec!["line_comment", "block_comment"],
            vec!["block_comment"],
            "groovy",
        )
    }

    pub fn ocaml() -> Self {
        Self::new("ocaml", vec!["ml", "mli"], vec!["comment"], vec![], "ocaml")
    }

    pub fn fortran() -> Self {
        Self::new(
            "fortran",
            vec!["f90", "f95", "f03", "f08"],
            vec!["comment"],
            vec![],
            "fortran",
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
        assert!(rust_config.supports_extension("RS"));

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
            LanguageConfig::shell(),
            LanguageConfig::haskell(),
            LanguageConfig::html(),
            LanguageConfig::css(),
            LanguageConfig::xml(),
            LanguageConfig::sql(),
            LanguageConfig::kotlin(),
            LanguageConfig::swift(),
            LanguageConfig::lua(),
            LanguageConfig::nix(),
            LanguageConfig::powershell(),
            LanguageConfig::proto(),
            LanguageConfig::ini(),
        ];

        for lang in languages {
            assert!(!lang.name.is_empty());
            assert!(!lang.extensions.is_empty());
            assert!(!lang.comment_types.is_empty());
        }
    }
}
