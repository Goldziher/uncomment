use crate::ast::visitor::CommentInfo;
use regex::Regex;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum PreservationRule {
    /// Preserve comments containing a specific pattern
    Pattern(String),
    /// Preserve comments based on regex pattern
    Regex(Regex),
    /// Preserve comments of a specific node type
    NodeType(String),
    /// Preserve documentation comments
    Documentation,
    /// Preserve comments at the beginning of files (headers)
    FileHeader,
    /// Preserve comments with specific prefixes
    Prefix(String),
    /// Preserve comments with specific suffixes
    Suffix(String),
    /// Custom function-based rule
    Custom(fn(&CommentInfo) -> bool),
}

impl PreservationRule {
    pub fn matches(&self, comment: &CommentInfo) -> bool {
        match self {
            PreservationRule::Pattern(pattern) => comment.content.contains(pattern),
            PreservationRule::Regex(regex) => regex.is_match(&comment.content),
            PreservationRule::NodeType(node_type) => comment.node_type == *node_type,
            PreservationRule::Documentation => self.is_documentation_comment(comment),
            PreservationRule::FileHeader => self.is_file_header_comment(comment),
            PreservationRule::Prefix(prefix) => self
                .extract_comment_text(&comment.content)
                .starts_with(prefix),
            PreservationRule::Suffix(suffix) => self
                .extract_comment_text(&comment.content)
                .ends_with(suffix),
            PreservationRule::Custom(func) => func(comment),
        }
    }

    fn is_documentation_comment(&self, comment: &CommentInfo) -> bool {
        // Common documentation comment patterns
        let doc_patterns = [
            "/**",
            "///",
            "//!",
            "##",
            "\"\"\"", // Common doc comment starters
            "doc_comment",
            "documentation_comment",
            "inner_doc_comment",
            "outer_doc_comment",
        ];

        // Check node type
        if doc_patterns
            .iter()
            .any(|&pattern| comment.node_type.contains(pattern))
        {
            return true;
        }

        // Check content patterns
        let content = comment.content.trim();
        doc_patterns
            .iter()
            .any(|&pattern| content.starts_with(pattern))
    }

    fn is_file_header_comment(&self, comment: &CommentInfo) -> bool {
        // Consider comments at the beginning of the file (first few lines) as headers
        comment.start_row < 10 && self.looks_like_header(&comment.content)
    }

    fn looks_like_header(&self, content: &str) -> bool {
        let header_indicators = [
            "copyright",
            "license",
            "author",
            "version",
            "file:",
            "description:",
            "created:",
            "modified:",
            "encoding:",
            "@file",
            "@author",
            "@version",
            "@copyright",
            "@license",
            "===",
            "---",
            "***", // Common header decorations
        ];

        let lower_content = content.to_lowercase();
        header_indicators
            .iter()
            .any(|&indicator| lower_content.contains(indicator))
    }

    fn extract_comment_text(&self, content: &str) -> String {
        // Remove common comment markers to get the actual text
        let mut text = content.trim();

        // Remove common comment prefixes
        let prefixes = ["//", "/*", "*/", "#", "<!--", "-->", "\"\"\"", "'''"];
        for prefix in &prefixes {
            if text.starts_with(prefix) {
                text = text.strip_prefix(prefix).unwrap_or(text).trim();
            }
        }

        // Remove common comment suffixes
        let suffixes = ["*/", "-->", "\"\"\"", "'''"];
        for suffix in &suffixes {
            if text.ends_with(suffix) {
                text = text.strip_suffix(suffix).unwrap_or(text).trim();
            }
        }

        text.to_string()
    }

    /// Create a pattern-based preservation rule
    pub fn pattern(pattern: &str) -> Self {
        PreservationRule::Pattern(pattern.to_string())
    }

    /// Create a regex-based preservation rule
    #[allow(dead_code)]
    pub fn regex(pattern: &str) -> Result<Self, regex::Error> {
        let regex = Regex::new(pattern)?;
        Ok(PreservationRule::Regex(regex))
    }

    /// Create a node type preservation rule
    #[allow(dead_code)]
    pub fn node_type(node_type: &str) -> Self {
        PreservationRule::NodeType(node_type.to_string())
    }

    /// Create a documentation preservation rule
    pub fn documentation() -> Self {
        PreservationRule::Documentation
    }

    /// Create a file header preservation rule
    pub fn file_header() -> Self {
        PreservationRule::FileHeader
    }

    /// Create a prefix-based preservation rule
    #[allow(dead_code)]
    pub fn prefix(prefix: &str) -> Self {
        PreservationRule::Prefix(prefix.to_string())
    }

    /// Create a suffix-based preservation rule
    #[allow(dead_code)]
    pub fn suffix(suffix: &str) -> Self {
        PreservationRule::Suffix(suffix.to_string())
    }

    /// Create a custom function-based preservation rule
    #[allow(dead_code)]
    pub fn custom(func: fn(&CommentInfo) -> bool) -> Self {
        PreservationRule::Custom(func)
    }
}

// Common preservation rule presets
impl PreservationRule {
    /// Get default preservation rules for most projects
    pub fn default_rules() -> Vec<Self> {
        vec![
            Self::pattern("TODO"),
            Self::pattern("FIXME"),
            Self::pattern("HACK"),
            Self::pattern("NOTE"),
            Self::pattern("WARNING"),
            Self::pattern("COPYRIGHT"),
            Self::pattern("LICENSE"),
            Self::documentation(),
            Self::file_header(),
        ]
    }

    /// Get minimal preservation rules (only critical patterns)
    #[allow(dead_code)]
    pub fn minimal_rules() -> Vec<Self> {
        vec![
            Self::pattern("TODO"),
            Self::pattern("FIXME"),
            Self::pattern("COPYRIGHT"),
            Self::pattern("LICENSE"),
        ]
    }

    /// Get comprehensive preservation rules
    pub fn comprehensive_rules() -> Vec<Self> {
        let mut rules = Self::default_rules();
        rules.extend(vec![
            Self::pattern("BUG"),
            Self::pattern("REVIEW"),
            Self::pattern("OPTIMIZE"),
            Self::pattern("PERFORMANCE"),
            Self::pattern("SECURITY"),
            Self::pattern("DEPRECATED"),
            // JSDoc/Documentation patterns
            Self::pattern("@param"),
            Self::pattern("@return"),
            Self::pattern("@throws"),
            Self::pattern("@author"),
            Self::pattern("@since"),
            Self::pattern("@version"),
            // TypeScript directives
            Self::pattern("@ts-expect-error"),
            Self::pattern("@ts-ignore"),
            Self::pattern("@ts-nocheck"),
            Self::pattern("@ts-check"),
            // ESLint directives
            Self::pattern("eslint-disable"),
            Self::pattern("eslint-enable"),
            Self::pattern("eslint-disable-next-line"),
            Self::pattern("eslint-disable-line"),
            // Prettier directives
            Self::pattern("prettier-ignore"),
            // Python type checking
            Self::pattern("type:"),
            Self::pattern("mypy:"),
            Self::pattern("pyright:"),
            Self::pattern("ruff:"),
            Self::pattern("noqa"),
            Self::pattern("pragma:"),
            Self::pattern("pylint:"),
            Self::pattern("flake8:"),
            // Go directives
            Self::pattern("//go:"),
            Self::pattern("nolint"),
            Self::pattern("lint:ignore"),
            // Rust directives
            Self::pattern("#["),
            Self::pattern("allow("),
            Self::pattern("deny("),
            Self::pattern("warn("),
            Self::pattern("forbid("),
            Self::pattern("expect("),
            Self::pattern("cfg("),
            Self::pattern("#!["),
            // Ruby directives
            Self::pattern("rubocop:"),
            // Java annotations
            Self::pattern("@Override"),
            Self::pattern("@SuppressWarnings"),
            Self::pattern("@Deprecated"),
            Self::pattern("@Generated"),
        ]);
        rules
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_comment(content: &str, node_type: &str, row: usize) -> CommentInfo {
        CommentInfo {
            start_byte: 0,
            end_byte: content.len(),
            start_row: row,
            end_row: row,
            content: content.to_string(),
            node_type: node_type.to_string(),
            should_preserve: false,
        }
    }

    #[test]
    fn test_pattern_rule() {
        let rule = PreservationRule::pattern("TODO");
        let comment = create_test_comment("// TODO: Fix this", "line_comment", 5);
        assert!(rule.matches(&comment));

        let comment2 = create_test_comment("// Regular comment", "line_comment", 5);
        assert!(!rule.matches(&comment2));
    }

    #[test]
    fn test_regex_rule() {
        let rule = PreservationRule::regex(r"TODO|FIXME").unwrap();
        let todo_comment = create_test_comment("// TODO: Fix this", "line_comment", 5);
        let fixme_comment = create_test_comment("// FIXME: Bug here", "line_comment", 5);
        let regular_comment = create_test_comment("// Regular comment", "line_comment", 5);

        assert!(rule.matches(&todo_comment));
        assert!(rule.matches(&fixme_comment));
        assert!(!rule.matches(&regular_comment));
    }

    #[test]
    fn test_node_type_rule() {
        let rule = PreservationRule::node_type("doc_comment");
        let doc_comment = create_test_comment("/// Documentation", "doc_comment", 5);
        let line_comment = create_test_comment("// Regular comment", "line_comment", 5);

        assert!(rule.matches(&doc_comment));
        assert!(!rule.matches(&line_comment));
    }

    #[test]
    fn test_documentation_rule() {
        let rule = PreservationRule::documentation();

        let cases = vec![
            ("/// Rust doc comment", "doc_comment", true),
            ("/** Java doc comment */", "block_comment", true),
            ("//! Inner doc comment", "line_comment", true),
            ("## Python docstring", "comment", true),
            ("// Regular comment", "line_comment", false),
        ];

        for (content, node_type, expected) in cases {
            let comment = create_test_comment(content, node_type, 5);
            assert_eq!(
                rule.matches(&comment),
                expected,
                "Failed for: {} ({})",
                content,
                node_type
            );
        }
    }

    #[test]
    fn test_file_header_rule() {
        let rule = PreservationRule::file_header();

        // Comments at the beginning of file with header-like content
        let header_comment = create_test_comment(
            "// Copyright 2023 Author\n// Licensed under MIT",
            "line_comment",
            0,
        );
        assert!(rule.matches(&header_comment));

        // Regular comment at the beginning
        let early_comment = create_test_comment("// Just a comment", "line_comment", 1);
        assert!(!rule.matches(&early_comment));

        // Header-like comment later in file
        let late_header = create_test_comment("// Copyright 2023 Author", "line_comment", 50);
        assert!(!rule.matches(&late_header));
    }

    #[test]
    fn test_prefix_suffix_rules() {
        let prefix_rule = PreservationRule::prefix("IMPORTANT");
        let suffix_rule = PreservationRule::suffix("END");

        let comment1 = create_test_comment("// IMPORTANT: Read this", "line_comment", 5);
        let comment2 = create_test_comment("// This is the END", "line_comment", 5);
        let comment3 = create_test_comment("// Regular comment", "line_comment", 5);

        assert!(prefix_rule.matches(&comment1));
        assert!(!prefix_rule.matches(&comment2));
        assert!(!prefix_rule.matches(&comment3));

        assert!(!suffix_rule.matches(&comment1));
        assert!(suffix_rule.matches(&comment2));
        assert!(!suffix_rule.matches(&comment3));
    }

    #[test]
    fn test_custom_rule() {
        let rule = PreservationRule::custom(|comment| comment.content.len() > 50);

        let long_comment = create_test_comment(
            "// This is a very long comment that should be preserved because it exceeds the length threshold",
            "line_comment",
            5,
        );
        let short_comment = create_test_comment("// Short", "line_comment", 5);

        assert!(rule.matches(&long_comment));
        assert!(!rule.matches(&short_comment));
    }

    #[test]
    fn test_extract_comment_text() {
        let rule = PreservationRule::pattern("test");

        let cases = vec![
            ("// Hello world", "Hello world"),
            ("/* Block comment */", "Block comment"),
            ("# Python comment", "Python comment"),
            ("<!-- HTML comment -->", "HTML comment"),
            ("\"\"\" Python docstring \"\"\"", "Python docstring"),
        ];

        for (input, expected) in cases {
            let extracted = rule.extract_comment_text(input);
            assert_eq!(extracted, expected, "Failed for input: {}", input);
        }
    }

    #[test]
    fn test_default_rules() {
        let rules = PreservationRule::default_rules();
        assert!(!rules.is_empty());

        let todo_comment = create_test_comment("// TODO: Fix this", "line_comment", 5);
        let matches = rules.iter().any(|rule| rule.matches(&todo_comment));
        assert!(matches);
    }

    #[test]
    fn test_rule_presets() {
        let minimal = PreservationRule::minimal_rules();
        let default = PreservationRule::default_rules();
        let comprehensive = PreservationRule::comprehensive_rules();

        assert!(minimal.len() <= default.len());
        assert!(default.len() <= comprehensive.len());

        // All presets should preserve TODO comments
        let todo_comment = create_test_comment("// TODO: Test", "line_comment", 5);
        for rules in [&minimal, &default, &comprehensive] {
            let matches = rules.iter().any(|rule| rule.matches(&todo_comment));
            assert!(matches, "TODO should be preserved by all rule presets");
        }
    }
}
