use crate::languages::{LanguageHandler, get_handler};
use crate::rules::preservation::PreservationRule;
use tree_sitter::Node;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentInfo {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_row: usize,
    pub end_row: usize,
    pub node_type: &'static str,
    pub should_preserve: bool,
    pub is_documentation: bool,
}

impl CommentInfo {
    #[must_use]
    pub fn new(node: Node) -> Self {
        Self {
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            start_row: node.start_position().row,
            end_row: node.end_position().row,
            node_type: node.kind(),
            should_preserve: false,
            is_documentation: false,
        }
    }

    #[must_use]
    pub const fn with_documentation(mut self, is_documentation: bool) -> Self {
        self.is_documentation = is_documentation;
        self
    }

    #[must_use]
    pub const fn with_preservation(mut self, should_preserve: bool) -> Self {
        self.should_preserve = should_preserve;
        self
    }

    /// Extract comment content from source by byte range.
    #[inline]
    pub fn content<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start_byte..self.end_byte]
    }
}

pub struct CommentVisitor<'a> {
    source: &'a str,
    preservation_rules: &'a [PreservationRule],
    comments: Vec<CommentInfo>,
    comment_node_types: &'a [String],
    doc_comment_node_types: &'a [String],
    language_handler: Box<dyn LanguageHandler>,
}

impl<'a> CommentVisitor<'a> {
    #[must_use]
    pub fn new_with_language(
        source: &'a str,
        preservation_rules: &'a [PreservationRule],
        comment_node_types: &'a [String],
        doc_comment_node_types: &'a [String],
        language_name: &str,
    ) -> Self {
        let language_handler = get_handler(language_name);
        Self {
            source,
            preservation_rules,
            comments: Vec::with_capacity(32),
            comment_node_types,
            doc_comment_node_types,
            language_handler,
        }
    }

    pub fn visit_node(&mut self, node: Node) {
        self.visit_node_recursive(node, None);
    }

    fn visit_node_recursive(&mut self, node: Node, parent: Option<Node>) {
        if self.is_comment_node(&node, parent) {
            let mut comment_info = CommentInfo::new(node);

            if let Some(is_doc) = self
                .language_handler
                .is_documentation_comment(&node, parent, self.source)
            {
                comment_info = comment_info.with_documentation(is_doc);
            }

            let forced_preserve = self
                .language_handler
                .should_preserve_comment(&node, parent, self.source)
                .unwrap_or(false);

            let content = comment_info.content(self.source);
            let should_preserve = forced_preserve || self.should_preserve_comment(&comment_info, content);
            let comment_with_preservation = comment_info.with_preservation(should_preserve);
            self.comments.push(comment_with_preservation);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.visit_node_recursive(child, Some(node));
        }
    }

    #[must_use]
    pub fn get_comments_to_remove(&self) -> Vec<&CommentInfo> {
        self.comments
            .iter()
            .filter(|comment| !comment.should_preserve)
            .collect()
    }

    fn is_comment_node(&self, node: &Node, parent: Option<Node>) -> bool {
        let kind = node.kind();

        if self.comment_node_types.iter().any(|node_type| node_type == kind) {
            return true;
        }

        if self.doc_comment_node_types.iter().any(|node_type| node_type == kind) {
            if let Some(is_doc) = self
                .language_handler
                .is_documentation_comment(node, parent, self.source)
            {
                return is_doc;
            }
            return true;
        }

        false
    }

    fn should_preserve_comment(&self, comment: &CommentInfo, content: &str) -> bool {
        for rule in self.preservation_rules {
            if rule.matches(comment, content) {
                return true;
            }
        }
        false
    }

    /// Extend `~keep` preservation across contiguous single-line comment blocks.
    ///
    /// A rationale comment often spans several consecutive `//` (or `#`, `--`, …)
    /// lines that tree-sitter models as one node *per line*, so a per-comment
    /// `~keep` would preserve only the marked line and strip the rest, gutting the
    /// block. This pass groups **standalone single-line comments on consecutive
    /// rows** into blocks and, when any line in a block carries `~keep`, preserves
    /// the whole block.
    ///
    /// Scope is deliberately narrow so the behaviour is unsurprising:
    /// - Only `~keep` extends — other preservation rules (TODO, patterns,
    ///   directives) stay per-comment.
    /// - Only *standalone* comments join a block; a trailing comment (`code // x`)
    ///   never anchors or joins one, and never drags in the line below.
    /// - Only *single-line* comment nodes group; a `/* … */` block comment is
    ///   already one node, so `~keep` inside it preserves it without this pass.
    /// - A blank line (non-consecutive rows) or any code between comments ends the
    ///   block.
    ///
    /// The pass is purely additive: it only ever sets `should_preserve = true`,
    /// never clears it, so running it after the per-comment decisions is safe.
    pub fn extend_keep_blocks(&mut self) {
        // Standalone single-line comments, in source order.
        let mut indices: Vec<usize> = (0..self.comments.len())
            .filter(|&i| self.is_standalone_single_line(&self.comments[i]))
            .collect();
        indices.sort_by_key(|&i| self.comments[i].start_byte);

        let mut run_start = 0;
        while run_start < indices.len() {
            // Extend the run while the next comment sits on the immediately
            // following row (consecutive standalone single-line comments).
            let mut run_end = run_start;
            while run_end + 1 < indices.len()
                && self.comments[indices[run_end + 1]].start_row == self.comments[indices[run_end]].start_row + 1
            {
                run_end += 1;
            }

            let has_keep = indices[run_start..=run_end]
                .iter()
                .any(|&i| self.comments[i].content(self.source).contains("~keep"));
            if has_keep {
                for &i in &indices[run_start..=run_end] {
                    self.comments[i].should_preserve = true;
                }
            }

            run_start = run_end + 1;
        }
    }

    /// Whether `comment` is a single-line comment node that occupies its line
    /// alone (only whitespace precedes it). Trailing comments and multi-line
    /// (block) comment nodes return `false`.
    fn is_standalone_single_line(&self, comment: &CommentInfo) -> bool {
        if comment.start_row != comment.end_row {
            return false;
        }
        let line_start = self.source[..comment.start_byte].rfind('\n').map_or(0, |pos| pos + 1);
        self.source[line_start..comment.start_byte]
            .bytes()
            .all(|byte| byte.is_ascii_whitespace())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::preservation::PreservationRule;

    fn create_mock_comment(node_type: &'static str) -> CommentInfo {
        CommentInfo {
            start_byte: 0,
            end_byte: 0,
            start_row: 0,
            end_row: 0,
            node_type,
            should_preserve: false,
            is_documentation: false,
        }
    }

    #[test]
    fn test_comment_info_creation() {
        let comment = create_mock_comment("line_comment");
        assert_eq!(comment.node_type, "line_comment");
        assert!(!comment.should_preserve);
    }

    #[test]
    fn test_comment_preservation() {
        let comment = create_mock_comment("line_comment");
        let preserved_comment = comment.with_preservation(true);
        assert!(preserved_comment.should_preserve);
    }

    #[test]
    fn test_visitor_creation() {
        let source = "// Test\nfn main() {}";
        let rules = vec![PreservationRule::pattern("TODO")];
        let comment_types = vec!["comment".to_string(), "line_comment".to_string()];
        let doc_types = vec!["doc_comment".to_string()];
        let visitor = CommentVisitor::new_with_language(source, &rules, &comment_types, &doc_types, "test");
        assert_eq!(visitor.source, source);
        assert_eq!(visitor.comments.len(), 0);
    }

    #[test]
    fn test_get_comments_to_remove() {
        let source = "// Test";
        let rules = vec![PreservationRule::pattern("TODO")];
        let comment_types = vec!["comment".to_string(), "line_comment".to_string()];
        let doc_types = vec!["doc_comment".to_string()];
        let mut visitor = CommentVisitor::new_with_language(source, &rules, &comment_types, &doc_types, "test");

        visitor
            .comments
            .push(create_mock_comment("line_comment").with_preservation(true));
        visitor
            .comments
            .push(create_mock_comment("line_comment").with_preservation(false));

        let to_remove = visitor.get_comments_to_remove();
        assert_eq!(to_remove.len(), 1);
        assert!(!to_remove[0].should_preserve);
    }
}
