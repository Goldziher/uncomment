use crate::ast::visitor::CommentInfo;

#[derive(Debug, Clone)]
pub enum PreservationRule {
    Pattern(String),
    Documentation,
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

    pub fn pattern_matches(&self, pattern: &str) -> bool {
        match self {
            PreservationRule::Pattern(rule_pattern) => rule_pattern == pattern,
            _ => false,
        }
    }

    fn is_documentation_comment(&self, comment: &CommentInfo) -> bool {
        if comment.is_documentation {
            return true;
        }

        let doc_patterns = [
            "/**",
            "///",
            "//!",
            "##",
            "\"\"\"",
            "doc_comment",
            "documentation_comment",
            "inner_doc_comment",
            "outer_doc_comment",
        ];

        if doc_patterns
            .iter()
            .any(|&pattern| comment.node_type.contains(pattern))
        {
            return true;
        }

        if comment.node_type == "string" {
            let content = comment.content.trim();
            if content.starts_with("\"\"\"") || content.starts_with("'''") {
                return true;
            }
        }

        let content = comment.content.trim();
        doc_patterns
            .iter()
            .any(|&pattern| content.starts_with(pattern))
    }

    fn is_file_header_comment(&self, comment: &CommentInfo) -> bool {
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
            "***",
        ];

        let lower_content = content.to_lowercase();
        header_indicators
            .iter()
            .any(|&indicator| lower_content.contains(indicator))
    }

    pub fn pattern(pattern: &str) -> Self {
        PreservationRule::Pattern(pattern.to_string())
    }

    pub fn documentation() -> Self {
        PreservationRule::Documentation
    }

    pub fn file_header() -> Self {
        PreservationRule::FileHeader
    }
}

impl PreservationRule {
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
            Self::pattern("eslint-disable"),
            Self::pattern("prettier-ignore"),
            Self::pattern("//nolint"),
            Self::pattern("# noqa"),
            Self::pattern("type: ignore"),
            Self::pattern("fmt: off"),
            Self::pattern("fmt: on"),
            Self::pattern("@ts-ignore"),
            Self::pattern("@ts-expect-error"),
            Self::pattern("/// <reference"),
        ]
    }

    pub fn comprehensive_rules() -> Vec<Self> {
        let mut rules = Self::default_rules();
        rules.extend(vec![
            Self::pattern("BUG"),
            Self::pattern("REVIEW"),
            Self::pattern("OPTIMIZE"),
            Self::pattern("PERFORMANCE"),
            Self::pattern("SECURITY"),
            Self::pattern("DEPRECATED"),
            Self::pattern("eslint-disable"),
            Self::pattern("eslint-enable"),
            Self::pattern("eslint-disable-next-line"),
            Self::pattern("eslint-disable-line"),
            Self::pattern("eslint-env"),
            Self::pattern("eslint:"),
            Self::pattern("biome-ignore"),
            Self::pattern("biome:"),
            Self::pattern("oxlint-ignore"),
            Self::pattern("oxlint-disable"),
            Self::pattern("deno-lint-ignore"),
            Self::pattern("deno-fmt-ignore"),
            Self::pattern("@ts-expect-error"),
            Self::pattern("@ts-ignore"),
            Self::pattern("@ts-nocheck"),
            Self::pattern("@ts-check"),
            Self::pattern("/// <reference"),
            Self::pattern("/// <amd-module"),
            Self::pattern("/// <amd-dependency"),
            Self::pattern("prettier-ignore"),
            Self::pattern("v8 ignore"),
            Self::pattern("c8 ignore"),
            Self::pattern("istanbul ignore"),
            Self::pattern("node:coverage"),
            Self::pattern("@preserve"),
            Self::pattern("webpack:"),
            Self::pattern("vite:"),
            Self::pattern("rollup:"),
            Self::pattern("esbuild:"),
            Self::pattern("type: ignore"),
            Self::pattern("type:ignore"),
            Self::pattern("mypy:"),
            Self::pattern("pyright:"),
            Self::pattern("pyright: ignore"),
            Self::pattern("ruff:"),
            Self::pattern("noqa"),
            Self::pattern("# noqa"),
            Self::pattern("pragma:"),
            Self::pattern("pylint: disable"),
            Self::pattern("pylint: enable"),
            Self::pattern("pylint:"),
            Self::pattern("flake8:"),
            Self::pattern("black:"),
            Self::pattern("fmt: off"),
            Self::pattern("fmt: on"),
            Self::pattern("fmt:off"),
            Self::pattern("fmt:on"),
            Self::pattern("bandit:"),
            Self::pattern("isort:"),
            Self::pattern("pyre-ignore"),
            Self::pattern("pyre-fixme"),
            Self::pattern("#["),
            Self::pattern("allow("),
            Self::pattern("deny("),
            Self::pattern("warn("),
            Self::pattern("forbid("),
            Self::pattern("expect("),
            Self::pattern("cfg("),
            Self::pattern("#!["),
            Self::pattern("clippy::"),
            Self::pattern("rustfmt::"),
            Self::pattern("#[rustfmt"),
            Self::pattern("#![rustfmt"),
            Self::pattern("//go:"),
            Self::pattern("nolint"),
            Self::pattern("//nolint"),
            Self::pattern("golangci-lint"),
            Self::pattern("lint:ignore"),
            Self::pattern("gosec:"),
            Self::pattern("gocyclo:"),
            Self::pattern("staticcheck:"),
            Self::pattern("exhaustive:"),
            Self::pattern("govet:"),
            Self::pattern("rubocop:"),
            Self::pattern("rubocop:disable"),
            Self::pattern("rubocop:enable"),
            Self::pattern("reek:"),
            Self::pattern("standard:disable"),
            Self::pattern("standard:enable"),
            Self::pattern("@Override"),
            Self::pattern("@SuppressWarnings"),
            Self::pattern("@Deprecated"),
            Self::pattern("@Generated"),
            Self::pattern("@SuppressFBWarnings"),
            Self::pattern("checkstyle:"),
            Self::pattern("//noinspection"),
            Self::pattern("// noinspection"),
            Self::pattern("spotbugs:"),
            Self::pattern("trivy:ignore"),
            Self::pattern("trivy ignore"),
            Self::pattern("tfsec:ignore"),
            Self::pattern("checkov:skip"),
            Self::pattern("terrascan:skip"),
            Self::pattern("terraform-docs:"),
            Self::pattern("tflint-ignore:"),
            Self::pattern("tflint:"),
            Self::pattern("#pragma"),
            Self::pattern("NOLINT"),
            Self::pattern("NOLINTNEXTLINE"),
            Self::pattern("clang-format off"),
            Self::pattern("clang-format on"),
            Self::pattern("cppcheck-suppress"),
            Self::pattern("coverity["),
            Self::pattern("shellcheck disable"),
            Self::pattern("shellcheck source"),
            Self::pattern("hadolint ignore"),
            Self::pattern("yamllint disable"),
            Self::pattern("yamllint enable"),
            Self::pattern("@param"),
            Self::pattern("@return"),
            Self::pattern("@throws"),
            Self::pattern("@author"),
            Self::pattern("@since"),
            Self::pattern("@version"),
            Self::pattern("@see"),
            Self::pattern("@example"),
            Self::pattern("@deprecated"),
            Self::pattern("@internal"),
            Self::pattern("@public"),
            Self::pattern("@private"),
            Self::pattern("@protected"),
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
            is_documentation: false,
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

        let header_comment = create_test_comment(
            "// Copyright 2023 Author\n// Licensed under MIT",
            "line_comment",
            0,
        );
        assert!(rule.matches(&header_comment));

        let early_comment = create_test_comment("// Just a comment", "line_comment", 1);
        assert!(!rule.matches(&early_comment));

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

    #[test]
    fn test_linting_ignore_patterns() {
        let rules = PreservationRule::comprehensive_rules();

        let test_cases = vec![
            ("// eslint-disable-next-line no-console", "line_comment"),
            (
                "// biome-ignore lint/suspicious/noExplicitAny",
                "line_comment",
            ),
            ("// @ts-ignore", "line_comment"),
            ("/* v8 ignore next */", "block_comment"),
            ("# noqa: F401", "comment"),
            ("# pylint: disable=unused-import", "comment"),
            ("# rubocop:disable Metrics/LineLength", "comment"),
            ("#[allow(clippy::too_many_arguments)]", "line_comment"),
            ("//nolint:gocyclo", "line_comment"),
            ("// @SuppressWarnings(\"unchecked\")", "line_comment"),
        ];

        for (content, node_type) in test_cases {
            let comment = create_test_comment(content, node_type, 5);
            let matches = rules.iter().any(|rule| rule.matches(&comment));
            assert!(
                matches,
                "Linting ignore pattern should be preserved: {content}"
            );
        }
    }

    #[test]
    fn test_coverage_ignore_patterns() {
        let rules = PreservationRule::comprehensive_rules();

        let coverage_patterns = vec![
            "/* v8 ignore next 3 */",
            "/* istanbul ignore next */",
            "/* c8 ignore start */",
            "// @preserve",
        ];

        for pattern in coverage_patterns {
            let comment = create_test_comment(pattern, "block_comment", 5);
            let matches = rules.iter().any(|rule| rule.matches(&comment));
            assert!(
                matches,
                "Coverage ignore pattern should be preserved: {pattern}"
            );
        }
    }

    #[test]
    fn test_formatter_ignore_patterns() {
        let rules = PreservationRule::comprehensive_rules();

        let formatter_patterns = vec!["// prettier-ignore", "# fmt: off", "# black: off"];

        for pattern in formatter_patterns {
            let comment = create_test_comment(pattern, "line_comment", 5);
            let matches = rules.iter().any(|rule| rule.matches(&comment));
            assert!(
                matches,
                "Formatter ignore pattern should be preserved: {pattern}"
            );
        }
    }
}
