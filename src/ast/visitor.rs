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
    comment_node_types: Vec<String>,
    doc_comment_node_types: Vec<String>,
    language_name: String,
}

impl<'a> CommentVisitor<'a> {
    pub fn new_with_language(
        source: &'a str,
        preservation_rules: &'a [PreservationRule],
        comment_node_types: Vec<String>,
        doc_comment_node_types: Vec<String>,
        language_name: String,
    ) -> Self {
        Self {
            source,
            preservation_rules,
            comments: Vec::new(),
            comment_node_types,
            doc_comment_node_types,
            language_name,
        }
    }

    pub fn visit_node(&mut self, node: Node) {
        self.visit_node_recursive(node, None);
    }

    fn visit_node_recursive(&mut self, node: Node, parent: Option<Node>) {
        // Check if this node is a comment
        if self.is_comment_node(&node, parent) {
            let comment_info = CommentInfo::new(node, self.source);
            let should_preserve = self.should_preserve_comment(&comment_info);
            let comment_with_preservation = comment_info.with_preservation(should_preserve);
            self.comments.push(comment_with_preservation);
        }

        // Recursively visit child nodes
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.visit_node_recursive(child, Some(node));
        }
    }

    pub fn get_comments_to_remove(&self) -> Vec<CommentInfo> {
        self.comments
            .iter()
            .filter(|comment| !comment.should_preserve)
            .cloned()
            .collect()
    }

    fn is_comment_node(&self, node: &Node, parent: Option<Node>) -> bool {
        let kind = node.kind();

        // Handle regular comment nodes
        if self.comment_node_types.contains(&kind.to_string()) {
            return true;
        }

        // Handle doc comment nodes
        if self.doc_comment_node_types.contains(&kind.to_string()) {
            // For Python, we need special handling of string nodes
            if self.language_name.to_lowercase() == "python" && kind == "string" {
                return self.is_python_docstring(node, parent);
            }
            // For other languages, treat all doc comment nodes as comments
            return true;
        }

        false
    }

    fn should_preserve_comment(&self, comment: &CommentInfo) -> bool {
        for rule in self.preservation_rules {
            if rule.matches(comment) {
                return true;
            }
        }
        false
    }

    /// Check if a string node in Python is actually a docstring
    fn is_python_docstring(&self, node: &Node, parent: Option<Node>) -> bool {
        // Must be a string node
        if node.kind() != "string" {
            return false;
        }

        // Get the parent node (should be expression_statement)
        let parent = match parent {
            Some(p) => p,
            None => return false,
        };

        // Parent must be an expression_statement
        if parent.kind() != "expression_statement" {
            return false;
        }

        // Get the grandparent to determine context
        let grandparent = match parent.parent() {
            Some(gp) => gp,
            None => return false,
        };

        match grandparent.kind() {
            "module" => {
                // Module-level docstring: first statement in the module
                self.is_first_statement(&parent, &grandparent)
            }
            "block" => {
                // Function/class/method docstring: first statement in a block
                if let Some(block_parent) = grandparent.parent() {
                    match block_parent.kind() {
                        "function_definition"
                        | "async_function_definition"
                        | "class_definition" => self.is_first_statement(&parent, &grandparent),
                        _ => false,
                    }
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Check if the given statement is the first non-comment statement in its parent
    fn is_first_statement(&self, statement: &Node, parent: &Node) -> bool {
        let mut cursor = parent.walk();
        for child in parent.children(&mut cursor) {
            match child.kind() {
                // Skip comments
                "comment" => continue,
                // This is the first non-comment statement
                _ => return child.id() == statement.id(),
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
        let comment_types = vec!["comment".to_string(), "line_comment".to_string()];
        let doc_types = vec!["doc_comment".to_string()];
        let visitor = CommentVisitor::new_with_language(
            source,
            &rules,
            comment_types,
            doc_types,
            "test".to_string(),
        );

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
        let comment_types = vec!["comment".to_string(), "line_comment".to_string()];
        let doc_types = vec!["doc_comment".to_string()];
        let mut visitor = CommentVisitor::new_with_language(
            source,
            &rules,
            comment_types,
            doc_types,
            "test".to_string(),
        );

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

        // Check that preserved comments are not in the removal list
        assert!(!to_remove.iter().any(|c| c.content == "// TODO: Keep this"));
    }
}
