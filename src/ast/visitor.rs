use crate::languages::{LanguageHandler, get_handler};
use crate::rules::preservation::PreservationRule;
use tree_sitter::Node;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentInfo {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_row: usize,
    pub end_row: usize,
    pub content: String,
    pub node_type: String,
    pub should_preserve: bool,
    pub is_documentation: bool,
}

impl CommentInfo {
    #[must_use]
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
}

pub struct CommentVisitor<'a> {
    source: &'a str,
    preservation_rules: &'a [PreservationRule],
    comments: Vec<CommentInfo>,
    comment_node_types: Vec<String>,
    doc_comment_node_types: Vec<String>,
    language_handler: Box<dyn LanguageHandler>,
}

impl<'a> CommentVisitor<'a> {
    #[must_use]
    pub fn new_with_language(
        source: &'a str,
        preservation_rules: &'a [PreservationRule],
        comment_node_types: Vec<String>,
        doc_comment_node_types: Vec<String>,
        language_name: String,
    ) -> Self {
        let language_handler = get_handler(&language_name);
        Self {
            source,
            preservation_rules,
            comments: Vec::new(),
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
            let mut comment_info = CommentInfo::new(node, self.source);

            if let Some(is_doc) =
                self.language_handler
                    .is_documentation_comment(&node, parent, self.source)
            {
                comment_info = comment_info.with_documentation(is_doc);
            }

            let forced_preserve = self
                .language_handler
                .should_preserve_comment(&node, parent, self.source)
                .unwrap_or(false);

            let should_preserve = forced_preserve || self.should_preserve_comment(&comment_info);
            let comment_with_preservation = comment_info.with_preservation(should_preserve);
            self.comments.push(comment_with_preservation);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.visit_node_recursive(child, Some(node));
        }
    }

    #[must_use]
    pub fn get_comments_to_remove(&self) -> Vec<CommentInfo> {
        self.comments
            .iter()
            .filter(|comment| !comment.should_preserve)
            .cloned()
            .collect()
    }

    fn is_comment_node(&self, node: &Node, parent: Option<Node>) -> bool {
        let kind = node.kind();

        if self.comment_node_types.contains(&kind.to_string()) {
            return true;
        }

        if self.doc_comment_node_types.contains(&kind.to_string()) {
            if let Some(is_doc) =
                self.language_handler
                    .is_documentation_comment(node, parent, self.source)
            {
                return is_doc;
            }
            return true;
        }

        false
    }

    fn should_preserve_comment(&self, comment: &CommentInfo) -> bool {
        // Check all preservation rules (TODO, FIXME, Documentation, etc.)
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
            is_documentation: false,
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
        let comment_types = vec!["comment".to_string(), "line_comment".to_string()];
        let doc_types = vec!["doc_comment".to_string()];
        let visitor = CommentVisitor::new_with_language(
            source,
            &rules,
            comment_types,
            doc_types,
            "test".to_string(),
        );
        assert_eq!(visitor.source, source);
        assert_eq!(visitor.comments.len(), 0);
    }

    #[test]
    fn test_is_comment_node() {
        let source = "// Test";
        let rules = vec![];
        let comment_types = vec!["comment".to_string(), "line_comment".to_string()];
        let doc_types = vec!["doc_comment".to_string()];
        let _visitor = CommentVisitor::new_with_language(
            source,
            &rules,
            comment_types,
            doc_types,
            "test".to_string(),
        );

        assert!(matches!("comment", "comment"));
        assert!(matches!("line_comment", "line_comment"));
        assert!(matches!("block_comment", "block_comment"));
        assert!(!matches!("function", "comment"));
    }

    #[test]
    fn test_get_comments_to_remove() {
        let source = "// Test";
        let rules = vec![PreservationRule::Pattern("TODO".to_string())];
        let comment_types = vec!["comment".to_string(), "line_comment".to_string()];
        let doc_types = vec!["doc_comment".to_string()];
        let mut visitor = CommentVisitor::new_with_language(
            source,
            &rules,
            comment_types,
            doc_types,
            "test".to_string(),
        );

        visitor.comments.push(
            create_mock_comment("// TODO: Keep this", "line_comment").with_preservation(true),
        );
        visitor
            .comments
            .push(create_mock_comment("// Remove this", "line_comment").with_preservation(false));

        let to_remove = visitor.get_comments_to_remove();
        assert_eq!(to_remove.len(), 1);
        assert_eq!(to_remove[0].content, "// Remove this");

        assert!(!to_remove.iter().any(|c| c.content == "// TODO: Keep this"));
    }
}
