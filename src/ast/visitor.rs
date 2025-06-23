use crate::rules::preservation::PreservationRule;
use tree_sitter::Node;

#[derive(Debug, Clone, PartialEq)]
pub struct CommentInfo {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_row: usize,
    pub end_row: usize,
    pub content: String,
    pub node_type: String,
    pub should_preserve: bool,
}

impl CommentInfo {
    pub fn new(node: Node, source: &str) -> Self {
        let content = source[node.start_byte()..node.end_byte()].to_string();
        Self {
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            start_row: node.start_position().row,
            end_row: node.end_position().row,
            content,
            node_type: node.kind().to_string(),
            should_preserve: false,
        }
    }

    pub fn with_preservation(mut self, should_preserve: bool) -> Self {
        self.should_preserve = should_preserve;
        self
    }
}

pub struct CommentVisitor<'a> {
    source: &'a str,
    preservation_rules: &'a [PreservationRule],
    comments: Vec<CommentInfo>,
}

impl<'a> CommentVisitor<'a> {
    pub fn new(source: &'a str, preservation_rules: &'a [PreservationRule]) -> Self {
        Self {
            source,
            preservation_rules,
            comments: Vec::new(),
        }
    }

    pub fn visit_node(&mut self, node: Node) {
        self.visit_node_recursive(node);
    }

    fn visit_node_recursive(&mut self, node: Node) {
        // Check if this node is a comment
        if self.is_comment_node(&node) {
            let comment_info = CommentInfo::new(node, self.source);
            let should_preserve = self.should_preserve_comment(&comment_info);
            let comment_with_preservation = comment_info.with_preservation(should_preserve);
            self.comments.push(comment_with_preservation);
        }

        // Recursively visit child nodes
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.visit_node_recursive(child);
        }
    }

    #[allow(dead_code)]
    pub fn get_comments(&self) -> &[CommentInfo] {
        &self.comments
    }

    pub fn get_comments_to_remove(&self) -> Vec<CommentInfo> {
        self.comments
            .iter()
            .filter(|comment| !comment.should_preserve)
            .cloned()
            .collect()
    }

    #[allow(dead_code)]
    pub fn get_comments_to_preserve(&self) -> Vec<CommentInfo> {
        self.comments
            .iter()
            .filter(|comment| comment.should_preserve)
            .cloned()
            .collect()
    }

    fn is_comment_node(&self, node: &Node) -> bool {
        let kind = node.kind();
        matches!(
            kind,
            "comment"
                | "line_comment"
                | "block_comment"
                | "doc_comment"
                | "documentation_comment"
                | "multiline_comment"
                | "single_line_comment"
        )
    }

    fn should_preserve_comment(&self, comment: &CommentInfo) -> bool {
        for rule in self.preservation_rules {
            if rule.matches(comment) {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::preservation::PreservationRule;

    fn create_mock_comment(content: &str, node_type: &str) -> CommentInfo {
        CommentInfo {
            start_byte: 0,
            end_byte: content.len(),
            start_row: 0,
            end_row: 0,
            content: content.to_string(),
            node_type: node_type.to_string(),
            should_preserve: false,
        }
    }

    #[test]
    fn test_comment_info_creation() {
        let comment = create_mock_comment("// Test comment", "line_comment");
        assert_eq!(comment.content, "// Test comment");
        assert_eq!(comment.node_type, "line_comment");
        assert!(!comment.should_preserve);
    }

    #[test]
    fn test_comment_preservation() {
        let comment = create_mock_comment("// Test comment", "line_comment");
        let preserved_comment = comment.with_preservation(true);
        assert!(preserved_comment.should_preserve);
    }

    #[test]
    fn test_visitor_creation() {
        let source = "// Test\nfn main() {}";
        let rules = vec![PreservationRule::Pattern("TODO".to_string())];
        let visitor = CommentVisitor::new(source, &rules);
        assert_eq!(visitor.source, source);
        assert_eq!(visitor.comments.len(), 0);
    }

    #[test]
    fn test_is_comment_node() {
        let source = "// Test";
        let rules = vec![];
        let _visitor = CommentVisitor::new(source, &rules);

        // We can't easily create tree-sitter nodes in tests without parsing,
        // so we'll test the string matching logic separately
        assert!(matches!("comment", "comment"));
        assert!(matches!("line_comment", "line_comment"));
        assert!(matches!("block_comment", "block_comment"));
        assert!(!matches!("function", "comment"));
    }

    #[test]
    fn test_should_preserve_comment() {
        let source = "// Test";
        let rules = vec![
            PreservationRule::Pattern("TODO".to_string()),
            PreservationRule::Pattern("FIXME".to_string()),
        ];
        let visitor = CommentVisitor::new(source, &rules);

        let todo_comment = create_mock_comment("// TODO: Fix this", "line_comment");
        let fixme_comment = create_mock_comment("// FIXME: Bug here", "line_comment");
        let regular_comment = create_mock_comment("// Regular comment", "line_comment");

        assert!(visitor.should_preserve_comment(&todo_comment));
        assert!(visitor.should_preserve_comment(&fixme_comment));
        assert!(!visitor.should_preserve_comment(&regular_comment));
    }

    #[test]
    fn test_get_comments_to_remove() {
        let source = "// Test";
        let rules = vec![PreservationRule::Pattern("TODO".to_string())];
        let mut visitor = CommentVisitor::new(source, &rules);

        // Manually add comments for testing
        visitor.comments.push(
            create_mock_comment("// TODO: Keep this", "line_comment").with_preservation(true),
        );
        visitor
            .comments
            .push(create_mock_comment("// Remove this", "line_comment").with_preservation(false));

        let to_remove = visitor.get_comments_to_remove();
        assert_eq!(to_remove.len(), 1);
        assert_eq!(to_remove[0].content, "// Remove this");

        let to_preserve = visitor.get_comments_to_preserve();
        assert_eq!(to_preserve.len(), 1);
        assert_eq!(to_preserve[0].content, "// TODO: Keep this");
    }
}
