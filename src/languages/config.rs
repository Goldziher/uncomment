use ahash::AHashSet;

/// A language's literal comment delimiters, as opposed to the tree-sitter node kind
/// names in [`LanguageConfig::comment_types`].
///
/// `line` is always the *plain* line-comment token, never a documentation form:
/// [`crate::rules::preservation::PreservationRule::Documentation`] classifies `///`,
/// `//!`, `/**` and `##` as documentation, and a marker comment written with one of
/// those is preserved as documentation instead of being read as a marker — it does
/// nothing, silently.
///
/// `block` is likewise the plain pair — `("/*", "*/")`, never `("/**", "*/")` — and is
/// only populated when the pair can be written at a comment's own indentation. A form
/// anchored to column 0, such as Ruby's `=begin`/`=end` or Perl's POD, is left `None`:
/// wrapping an indented marker in it would not parse as a comment.
///
/// Either form may be absent. CSS and OCaml have only a block pair, Python and YAML
/// only a line token, and plain JSON has neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommentSyntax {
    pub line: Option<&'static str>,
    pub block: Option<(&'static str, &'static str)>,
}

impl CommentSyntax {
    /// No comment syntax at all.
    pub const NONE: Self = Self {
        line: None,
        block: None,
    };
    /// `//` plus `/* */` — the C family and the languages modelled on it.
    pub const C_STYLE: Self = Self::both("//", "/*", "*/");
    /// `//` with no block form, as in Zig.
    pub const SLASH: Self = Self::line_only("//");
    /// `#` with no block form.
    pub const HASH: Self = Self::line_only("#");
    /// `#` plus the C block pair, as in Nix and HCL.
    pub const HASH_C_BLOCK: Self = Self::both("#", "/*", "*/");
    /// `<!-- -->` only — markup languages.
    pub const MARKUP: Self = Self::block_only("<!--", "-->");
    /// `/* */` only — CSS.
    pub const C_BLOCK: Self = Self::block_only("/*", "*/");
    /// `--` plus `{- -}` — the Haskell family.
    pub const DASH_BRACE: Self = Self::both("--", "{-", "-}");

    #[must_use]
    pub const fn line_only(line: &'static str) -> Self {
        Self {
            line: Some(line),
            block: None,
        }
    }

    #[must_use]
    pub const fn block_only(open: &'static str, close: &'static str) -> Self {
        Self {
            line: None,
            block: Some((open, close)),
        }
    }

    #[must_use]
    pub const fn both(line: &'static str, open: &'static str, close: &'static str) -> Self {
        Self {
            line: Some(line),
            block: Some((open, close)),
        }
    }

    /// Syntax shared by every language that parses with the given tree-sitter grammar,
    /// used as a fallback for languages registered at runtime without their own entry.
    ///
    /// Returns `None` where no single answer is correct — `json` backs both plain JSON
    /// (no comments) and JSONC (`//` and `/* */`), so guessing either would be wrong.
    #[must_use]
    pub fn for_tree_sitter_language(tslp_name: &str) -> Option<Self> {
        let syntax = match tslp_name {
            "c" | "cpp" | "csharp" | "dart" | "go" | "groovy" | "java" | "javascript" | "kotlin" | "objc" | "php"
            | "proto" | "rust" | "scala" | "scss" | "swift" | "tsx" | "typescript" => Self::C_STYLE,
            "bash" | "dockerfile" | "elixir" | "fish" | "make" | "perl" | "properties" | "python" | "r"
            | "starlark" | "toml" | "yaml" => Self::HASH,
            "hcl" | "nix" => Self::HASH_C_BLOCK,
            "html" | "markdown" | "svelte" | "vue" | "xml" => Self::MARKUP,
            "elm" | "haskell" => Self::DASH_BRACE,
            "css" => Self::C_BLOCK,
            "zig" => Self::SLASH,
            "clojure" | "ini" => Self::line_only(";"),
            "erlang" | "latex" => Self::line_only("%"),
            "fortran" => Self::line_only("!"),
            // Ruby's `=begin`/`=end` is recognised only at column 0, so it cannot wrap a
            // marker at a comment's own indentation — `#` is the only usable form.
            "ruby" => Self::HASH,
            "sql" => Self::both("--", "/*", "*/"),
            "lua" => Self::both("--", "--[[", "]]"),
            "powershell" => Self::both("#", "<#", "#>"),
            "julia" => Self::both("#", "#=", "=#"),
            "ocaml" => Self::block_only("(*", "*)"),
            _ => return None,
        };
        Some(syntax)
    }
}

/// Outcome of asking a [`LanguageConfig`] for its literal comment syntax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentSyntaxResolution {
    Resolved(CommentSyntax),
    /// Nothing is known about this language's delimiters. Callers must skip and report,
    /// never fall back to a guess: a wrong marker is worse than no marker.
    Unknown,
}

#[derive(Debug, Clone)]
pub struct LanguageConfig {
    pub name: String,
    pub extensions: Vec<String>,
    /// Whole filenames this language claims, matched case-sensitively and before any extension rule.
    ///
    /// An entry ending in `.*` claims every name starting with the part before the `*`, which is how
    /// `Dockerfile.prod` is reached without naming the language in detection code.
    pub filenames: Vec<String>,
    pub comment_types: Vec<String>,
    pub doc_comment_types: Vec<String>,
    pub tslp_name: String,
    pub comment_syntax: Option<CommentSyntax>,
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
            filenames: Vec::new(),
            comment_types: comment_types.iter().map(|&s| s.to_string()).collect(),
            doc_comment_types: doc_comment_types.iter().map(|&s| s.to_string()).collect(),
            tslp_name: tslp_name.to_string(),
            comment_syntax: None,
        }
    }

    #[must_use]
    pub const fn with_comment_syntax(mut self, syntax: CommentSyntax) -> Self {
        self.comment_syntax = Some(syntax);
        self
    }

    #[must_use]
    pub fn with_filenames(mut self, filenames: Vec<&str>) -> Self {
        self.filenames = filenames.iter().map(|&s| s.to_string()).collect();
        self
    }

    /// This language's literal delimiters, falling back to the grammar family when the
    /// config carries no entry of its own.
    #[must_use]
    pub fn resolve_comment_syntax(&self) -> CommentSyntaxResolution {
        if let Some(syntax) = self.comment_syntax {
            return CommentSyntaxResolution::Resolved(syntax);
        }

        match CommentSyntax::for_tree_sitter_language(&self.tslp_name) {
            Some(syntax) => CommentSyntaxResolution::Resolved(syntax),
            None => CommentSyntaxResolution::Unknown,
        }
    }

    /// The plain line-comment token to prefix a marker line with, if the language has one.
    #[must_use]
    pub fn line_comment_token(&self) -> Option<&'static str> {
        match self.resolve_comment_syntax() {
            CommentSyntaxResolution::Resolved(syntax) => syntax.line,
            CommentSyntaxResolution::Unknown => None,
        }
    }

    /// The plain block-comment delimiters to wrap a marker line in, for a language that has no line
    /// form — CSS and HTML carry every comment they have this way.
    #[must_use]
    pub fn block_comment_delimiters(&self) -> Option<(&'static str, &'static str)> {
        match self.resolve_comment_syntax() {
            CommentSyntaxResolution::Resolved(syntax) => syntax.block,
            CommentSyntaxResolution::Unknown => None,
        }
    }

    pub fn supports_extension(&self, extension: &str) -> bool {
        self.extensions
            .iter()
            .any(|configured| configured.eq_ignore_ascii_case(extension))
    }

    pub fn is_comment_type(&self, node_type: &str) -> bool {
        self.comment_types.iter().any(|configured| configured == node_type)
    }

    pub fn is_doc_comment_type(&self, node_type: &str) -> bool {
        self.doc_comment_types.iter().any(|configured| configured == node_type)
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
        .with_comment_syntax(CommentSyntax::C_STYLE)
    }

    pub fn python() -> Self {
        Self::new(
            "python",
            vec!["py", "pyw", "pyi", "pyx", "pxd"],
            vec!["comment"],
            vec!["string"],
            "python",
        )
        .with_comment_syntax(CommentSyntax::HASH)
    }

    pub fn javascript() -> Self {
        Self::new(
            "javascript",
            vec!["js", "jsx", "mjs", "cjs"],
            vec!["comment"],
            vec!["comment"],
            "javascript",
        )
        .with_comment_syntax(CommentSyntax::C_STYLE)
    }

    pub fn typescript() -> Self {
        Self::new(
            "typescript",
            vec!["ts", "mts", "cts", "d.ts", "d.mts", "d.cts"],
            vec!["comment"],
            vec!["comment"],
            "typescript",
        )
        .with_comment_syntax(CommentSyntax::C_STYLE)
    }

    pub fn tsx() -> Self {
        Self::new("tsx", vec!["tsx"], vec!["comment"], vec!["comment"], "tsx")
            .with_comment_syntax(CommentSyntax::C_STYLE)
    }

    pub fn go() -> Self {
        Self::new("go", vec!["go"], vec!["comment"], vec!["comment"], "go").with_comment_syntax(CommentSyntax::C_STYLE)
    }

    pub fn ruby() -> Self {
        Self::new(
            "ruby",
            vec!["rb", "rbw", "gemspec", "rake"],
            vec!["comment"],
            vec![],
            "ruby",
        )
        .with_comment_syntax(CommentSyntax::HASH)
    }

    pub fn php() -> Self {
        Self::new("php", vec!["php", "phtml"], vec!["comment"], vec![], "php")
            .with_comment_syntax(CommentSyntax::C_STYLE)
    }

    pub fn elixir() -> Self {
        Self::new("elixir", vec!["ex", "exs"], vec!["comment"], vec![], "elixir")
            .with_comment_syntax(CommentSyntax::HASH)
    }

    pub fn toml() -> Self {
        Self::new("toml", vec!["toml"], vec!["comment"], vec![], "toml").with_comment_syntax(CommentSyntax::HASH)
    }

    pub fn csharp() -> Self {
        Self::new("csharp", vec!["cs"], vec!["comment"], vec![], "csharp").with_comment_syntax(CommentSyntax::C_STYLE)
    }

    pub fn java() -> Self {
        Self::new(
            "java",
            vec!["java"],
            vec!["line_comment", "block_comment"],
            vec!["block_comment"],
            "java",
        )
        .with_comment_syntax(CommentSyntax::C_STYLE)
    }

    pub fn c() -> Self {
        Self::new("c", vec!["c", "h"], vec!["comment"], vec!["comment"], "c")
            .with_comment_syntax(CommentSyntax::C_STYLE)
    }

    pub fn cpp() -> Self {
        Self::new(
            "cpp",
            vec!["cpp", "cxx", "cc", "c++", "hpp", "hxx", "hh", "h++"],
            vec!["comment"],
            vec!["comment"],
            "cpp",
        )
        .with_comment_syntax(CommentSyntax::C_STYLE)
    }

    pub fn json() -> Self {
        Self::new("json", vec!["json"], vec![], vec![], "json").with_comment_syntax(CommentSyntax::NONE)
    }

    pub fn jsonc() -> Self {
        Self::new("jsonc", vec!["jsonc"], vec!["comment"], vec![], "json").with_comment_syntax(CommentSyntax::C_STYLE)
    }

    pub fn yaml() -> Self {
        Self::new("yaml", vec!["yaml", "yml"], vec!["comment"], vec![], "yaml").with_comment_syntax(CommentSyntax::HASH)
    }

    pub fn hcl() -> Self {
        Self::new("hcl", vec!["hcl", "tf", "tfvars"], vec!["comment"], vec![], "hcl")
            .with_comment_syntax(CommentSyntax::HASH_C_BLOCK)
    }

    pub fn make() -> Self {
        Self::new("make", vec!["mk"], vec!["comment"], vec![], "make")
            .with_filenames(vec!["Makefile", "makefile", "GNUmakefile"])
            .with_comment_syntax(CommentSyntax::HASH)
    }

    pub fn shell() -> Self {
        Self::new(
            "shell",
            vec!["sh", "bash", "zsh"],
            vec!["comment"],
            vec!["comment"],
            "bash",
        )
        .with_filenames(vec!["bashrc", ".bashrc", "zshrc", ".zshrc", "zshenv", ".zshenv"])
        .with_comment_syntax(CommentSyntax::HASH)
    }

    pub fn haskell() -> Self {
        Self::new("haskell", vec!["hs", "lhs"], vec!["comment"], vec![], "haskell")
            .with_comment_syntax(CommentSyntax::DASH_BRACE)
    }

    pub fn html() -> Self {
        Self::new("html", vec!["html", "htm", "xhtml"], vec!["comment"], vec![], "html")
            .with_comment_syntax(CommentSyntax::MARKUP)
    }

    pub fn css() -> Self {
        Self::new("css", vec!["css"], vec!["comment"], vec![], "css").with_comment_syntax(CommentSyntax::C_BLOCK)
    }

    pub fn xml() -> Self {
        Self::new(
            "xml",
            vec!["xml", "xsd", "xsl", "xslt", "svg"],
            vec!["Comment"],
            vec![],
            "xml",
        )
        .with_comment_syntax(CommentSyntax::MARKUP)
    }

    pub fn sql() -> Self {
        Self::new("sql", vec!["sql"], vec!["comment"], vec![], "sql")
            .with_comment_syntax(CommentSyntax::both("--", "/*", "*/"))
    }

    pub fn kotlin() -> Self {
        Self::new(
            "kotlin",
            vec!["kt", "kts"],
            vec!["line_comment", "block_comment"],
            vec![],
            "kotlin",
        )
        .with_comment_syntax(CommentSyntax::C_STYLE)
    }

    pub fn objc() -> Self {
        Self::new("objc", vec!["m"], vec!["comment"], vec!["comment"], "objc")
            .with_comment_syntax(CommentSyntax::C_STYLE)
    }

    pub fn swift() -> Self {
        Self::new(
            "swift",
            vec!["swift"],
            vec!["comment", "multiline_comment"],
            vec![],
            "swift",
        )
        .with_comment_syntax(CommentSyntax::C_STYLE)
    }

    pub fn lua() -> Self {
        Self::new("lua", vec!["lua"], vec!["comment"], vec![], "lua")
            .with_comment_syntax(CommentSyntax::both("--", "--[[", "]]"))
    }

    pub fn nix() -> Self {
        Self::new("nix", vec!["nix"], vec!["comment"], vec![], "nix").with_comment_syntax(CommentSyntax::HASH_C_BLOCK)
    }

    pub fn powershell() -> Self {
        Self::new(
            "powershell",
            vec!["ps1", "psm1", "psd1"],
            vec!["comment"],
            vec![],
            "powershell",
        )
        .with_comment_syntax(CommentSyntax::both("#", "<#", "#>"))
    }

    pub fn proto() -> Self {
        Self::new("proto", vec!["proto"], vec!["comment"], vec![], "proto").with_comment_syntax(CommentSyntax::C_STYLE)
    }

    pub fn ini() -> Self {
        Self::new("ini", vec!["ini", "cfg", "conf"], vec!["comment"], vec![], "ini")
            .with_comment_syntax(CommentSyntax::line_only(";"))
    }

    pub fn dockerfile() -> Self {
        Self::new("dockerfile", vec![], vec!["comment"], vec![], "dockerfile")
            .with_filenames(vec!["Dockerfile", "dockerfile", "Dockerfile.*", "dockerfile.*"])
            .with_comment_syntax(CommentSyntax::HASH)
    }

    /// Bazel's build language. `BUILD` and `WORKSPACE` carry no extension at all, so the filename
    /// list — not the extension list — is what makes most of a Bazel repository visible.
    ///
    /// `doc_comment_types` is deliberately empty. Starlark has Python's docstrings rather than a
    /// doc-comment form, and only [`crate::languages::handlers::PythonHandler`] can tell a docstring
    /// `string` node from any other string literal; declaring `string` here without that classifier
    /// would hand every string in a `BUILD` file to the doc-comment machinery.
    pub fn starlark() -> Self {
        Self::new(
            "starlark",
            vec!["bzl", "bazel", "star"],
            vec!["comment"],
            vec![],
            "starlark",
        )
        .with_filenames(vec![
            "BUILD",
            "BUILD.bazel",
            "WORKSPACE",
            "WORKSPACE.bazel",
            "WORKSPACE.bzlmod",
            "MODULE.bazel",
        ])
        .with_comment_syntax(CommentSyntax::HASH)
    }

    /// Java `.properties`. The grammar reports both comment forms — `#` and `!` — as `comment`.
    pub fn properties() -> Self {
        Self::new("properties", vec!["properties"], vec!["comment"], vec![], "properties")
            .with_comment_syntax(CommentSyntax::HASH)
    }

    pub fn scala() -> Self {
        Self::new(
            "scala",
            vec!["scala", "sc"],
            vec!["comment", "block_comment"],
            vec!["block_comment"],
            "scala",
        )
        .with_comment_syntax(CommentSyntax::C_STYLE)
    }

    pub fn dart() -> Self {
        Self::new(
            "dart",
            vec!["dart"],
            vec!["comment"],
            vec!["documentation_comment"],
            "dart",
        )
        .with_comment_syntax(CommentSyntax::C_STYLE)
    }

    pub fn r() -> Self {
        Self::new("r", vec!["r", "R"], vec!["comment"], vec![], "r").with_comment_syntax(CommentSyntax::HASH)
    }

    pub fn julia() -> Self {
        Self::new("julia", vec!["jl"], vec!["line_comment"], vec![], "julia")
            .with_comment_syntax(CommentSyntax::both("#", "#=", "=#"))
    }

    pub fn zig() -> Self {
        Self::new("zig", vec!["zig"], vec!["line_comment"], vec![], "zig").with_comment_syntax(CommentSyntax::SLASH)
    }

    pub fn clojure() -> Self {
        Self::new(
            "clojure",
            vec!["clj", "cljs", "cljc", "edn"],
            vec!["comment"],
            vec![],
            "clojure",
        )
        .with_comment_syntax(CommentSyntax::line_only(";"))
    }

    pub fn elm() -> Self {
        Self::new("elm", vec!["elm"], vec!["line_comment", "block_comment"], vec![], "elm")
            .with_comment_syntax(CommentSyntax::DASH_BRACE)
    }

    pub fn erlang() -> Self {
        Self::new("erlang", vec!["erl", "hrl"], vec!["comment"], vec![], "erlang")
            .with_comment_syntax(CommentSyntax::line_only("%"))
    }

    pub fn vue() -> Self {
        Self::new("vue", vec!["vue"], vec!["comment"], vec![], "vue").with_comment_syntax(CommentSyntax::MARKUP)
    }

    pub fn svelte() -> Self {
        Self::new("svelte", vec!["svelte"], vec!["comment"], vec![], "svelte")
            .with_comment_syntax(CommentSyntax::MARKUP)
    }

    pub fn scss() -> Self {
        Self::new("scss", vec!["scss"], vec!["comment", "js_comment"], vec![], "scss")
            .with_comment_syntax(CommentSyntax::C_STYLE)
    }

    pub fn latex() -> Self {
        Self::new(
            "latex",
            vec!["tex", "sty", "cls"],
            vec!["line_comment"],
            vec![],
            "latex",
        )
        .with_comment_syntax(CommentSyntax::line_only("%"))
    }

    pub fn fish() -> Self {
        Self::new("fish", vec!["fish"], vec!["comment"], vec![], "fish").with_comment_syntax(CommentSyntax::HASH)
    }

    pub fn perl() -> Self {
        Self::new("perl", vec!["pl", "pm"], vec!["comment"], vec![], "perl").with_comment_syntax(CommentSyntax::HASH)
    }

    pub fn groovy() -> Self {
        Self::new(
            "groovy",
            vec!["groovy", "gradle"],
            vec!["line_comment", "block_comment"],
            vec!["block_comment"],
            "groovy",
        )
        .with_comment_syntax(CommentSyntax::C_STYLE)
    }

    pub fn ocaml() -> Self {
        Self::new("ocaml", vec!["ml", "mli"], vec!["comment"], vec![], "ocaml")
            .with_comment_syntax(CommentSyntax::block_only("(*", "*)"))
    }

    /// CommonMark. The grammar emits no `comment` node whatsoever — an HTML comment is raw HTML to
    /// CommonMark, so `<!-- … -->` arrives as an `html_block`, the same kind that carries a
    /// `<div align="center">` badge row or a `<details>` block.
    ///
    /// `html_block` is therefore declared here *only* because
    /// [`crate::languages::handlers::MarkdownHandler`] can tell the two apart from the block's text.
    /// Declaring it for a runtime-registered language, which gets no handler, would delete embedded
    /// HTML.
    ///
    /// `doc_comment_types` is empty: markdown has no documentation-comment form.
    pub fn markdown() -> Self {
        Self::new(
            "markdown",
            vec!["md", "markdown", "mdown", "mkd"],
            vec!["html_block"],
            vec![],
            "markdown",
        )
        .with_comment_syntax(CommentSyntax::MARKUP)
    }

    pub fn fortran() -> Self {
        Self::new(
            "fortran",
            vec!["f90", "f95", "f03", "f08"],
            vec!["comment"],
            vec![],
            "fortran",
        )
        .with_comment_syntax(CommentSyntax::line_only("!"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prefixes `PreservationRule::Documentation` treats as documentation. Duplicated
    /// here so a unit test can guard the table without reaching into the rules module;
    /// `tests/language_syntax_test.rs` runs the real classifier.
    const DOC_PREFIXES: &[&str] = &["/**", "///", "//!", "##", "\"\"\"", "'''"];

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
            LanguageConfig::objc(),
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

    #[test]
    fn new_leaves_comment_syntax_unset() {
        let config = LanguageConfig::new("custom", vec!["cst"], vec!["comment"], vec![], "unheard-of");
        assert_eq!(config.comment_syntax, None);
        assert_eq!(config.resolve_comment_syntax(), CommentSyntaxResolution::Unknown);
        assert_eq!(config.line_comment_token(), None);
    }

    #[test]
    fn builtin_configs_carry_their_own_syntax() {
        assert_eq!(LanguageConfig::rust().comment_syntax, Some(CommentSyntax::C_STYLE));
        assert_eq!(LanguageConfig::python().line_comment_token(), Some("#"));
        assert_eq!(LanguageConfig::json().comment_syntax, Some(CommentSyntax::NONE));
        assert_eq!(LanguageConfig::json().line_comment_token(), None);
    }

    #[test]
    fn family_fallback_resolves_by_tree_sitter_name() {
        let config = LanguageConfig::new("bash-ish", vec!["bsh"], vec!["comment"], vec![], "bash");
        assert_eq!(
            config.resolve_comment_syntax(),
            CommentSyntaxResolution::Resolved(CommentSyntax::HASH)
        );
    }

    #[test]
    fn ambiguous_json_family_does_not_guess() {
        assert_eq!(CommentSyntax::for_tree_sitter_language("json"), None);
    }

    #[test]
    fn no_builtin_line_token_is_a_documentation_prefix() {
        for syntax in [
            CommentSyntax::C_STYLE,
            CommentSyntax::SLASH,
            CommentSyntax::HASH,
            CommentSyntax::HASH_C_BLOCK,
            CommentSyntax::DASH_BRACE,
            CommentSyntax::both("--", "/*", "*/"),
            CommentSyntax::both("--", "--[[", "]]"),
            CommentSyntax::both("#", "<#", "#>"),
            CommentSyntax::both("#", "#=", "=#"),
            CommentSyntax::line_only(";"),
            CommentSyntax::line_only("%"),
            CommentSyntax::line_only("!"),
        ] {
            let line = syntax.line.expect("every syntax in this list has a line token");
            let marker = format!("{line} ~keep");
            for prefix in DOC_PREFIXES {
                assert!(
                    !marker.starts_with(prefix),
                    "`{marker}` starts with doc prefix `{prefix}`"
                );
            }
        }
    }
}
