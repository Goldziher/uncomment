use crate::ast::visitor::CommentInfo;

#[derive(Debug, Clone)]
pub enum PreservationRule {
    /// Preserve comments containing a specific pattern
    Pattern(String),
    /// Preserve documentation comments
    Documentation,
    /// Preserve comments at the beginning of files (headers)
    FileHeader,
}

impl PreservationRule {
    pub fn matches(&self, comment: &CommentInfo) -> bool {
        match self {
            PreservationRule::Pattern(pattern) => comment.content.contains(pattern),
            PreservationRule::Documentation => self.is_documentation_comment(comment),
            PreservationRule::FileHeader => self.is_file_header_comment(comment),
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

    /// Create a pattern-based preservation rule
    pub fn pattern(pattern: &str) -> Self {
        PreservationRule::Pattern(pattern.to_string())
    }

    /// Create a documentation preservation rule
    pub fn documentation() -> Self {
        PreservationRule::Documentation
    }

    /// Create a file header preservation rule
    pub fn file_header() -> Self {
        PreservationRule::FileHeader
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
    fn test_default_rules() {
        let rules = PreservationRule::default_rules();
        assert!(!rules.is_empty());

        let todo_comment = create_test_comment("// TODO: Fix this", "line_comment", 5);
        let matches = rules.iter().any(|rule| rule.matches(&todo_comment));
        assert!(matches);
    }

    #[test]
    fn test_rule_presets() {
        let default = PreservationRule::default_rules();
        let comprehensive = PreservationRule::comprehensive_rules();

        assert!(default.len() <= comprehensive.len());

        // Both presets should preserve TODO comments
        let todo_comment = create_test_comment("// TODO: Test", "line_comment", 5);
        for rules in [&default, &comprehensive] {
            let matches = rules.iter().any(|rule| rule.matches(&todo_comment));
            assert!(matches, "TODO should be preserved by all rule presets");
        }
    }
}
