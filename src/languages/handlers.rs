use tree_sitter::Node;

pub trait LanguageHandler {
    fn is_documentation_comment(
        &self,
        node: &Node,
        parent: Option<Node>,
        source: &str,
    ) -> Option<bool>;
}

pub struct DefaultHandler;

impl LanguageHandler for DefaultHandler {
    fn is_documentation_comment(
        &self,
        _node: &Node,
        _parent: Option<Node>,
        _source: &str,
    ) -> Option<bool> {
        None
    }
}

pub struct PythonHandler;

impl LanguageHandler for PythonHandler {
    fn is_documentation_comment(
        &self,
        node: &Node,
        parent: Option<Node>,
        _source: &str,
    ) -> Option<bool> {
        if node.kind() != "string" {
            return None;
        }

        let parent = parent?;
        if parent.kind() != "expression_statement" {
            return Some(false);
        }

        let grandparent = parent.parent()?;

        match grandparent.kind() {
            "module" => Some(self.is_first_statement(&parent, &grandparent)),
            "block" => {
                if let Some(block_parent) = grandparent.parent() {
                    match block_parent.kind() {
                        "function_definition"
                        | "async_function_definition"
                        | "class_definition" => {
                            Some(self.is_first_statement(&parent, &grandparent))
                        }
                        _ => Some(false),
                    }
                } else {
                    Some(false)
                }
            }
            _ => Some(false),
        }
    }
}

impl PythonHandler {
    fn is_first_statement(&self, statement: &Node, parent: &Node) -> bool {
        let mut cursor = parent.walk();
        for child in parent.children(&mut cursor) {
            if child.kind() != "comment" {
                return child.id() == statement.id();
            }
        }
        false
    }
}

pub struct GoHandler;

impl LanguageHandler for GoHandler {
    fn is_documentation_comment(
        &self,
        node: &Node,
        parent: Option<Node>,
        _source: &str,
    ) -> Option<bool> {
        if node.kind() != "comment" {
            return None;
        }

        if self.precedes_declaration(node, parent) {
            Some(true)
        } else {
            Some(false)
        }
    }
}

impl GoHandler {
    fn precedes_declaration(&self, comment_node: &Node, parent: Option<Node>) -> bool {
        let parent = match parent {
            Some(p) => p,
            None => return false,
        };

        if let Some(next_sibling) = self.find_next_non_comment_sibling(comment_node, &parent) {
            matches!(
                next_sibling.kind(),
                "function_declaration"
                    | "method_declaration"
                    | "type_declaration"
                    | "const_declaration"
                    | "var_declaration"
                    | "package_clause"
            )
        } else {
            false
        }
    }

    fn find_next_non_comment_sibling<'a>(
        &self,
        comment_node: &Node,
        parent: &Node<'a>,
    ) -> Option<Node<'a>> {
        let mut cursor = parent.walk();
        let mut found_comment = false;

        for child in parent.children(&mut cursor) {
            if found_comment && child.kind() != "comment" {
                return Some(child);
            }

            if child.id() == comment_node.id() {
                found_comment = true;
            }
        }

        None
    }
}

pub fn get_handler(language_name: &str) -> Box<dyn LanguageHandler> {
    match language_name.to_lowercase().as_str() {
        "python" => Box::new(PythonHandler),
        "go" => Box::new(GoHandler),
        _ => Box::new(DefaultHandler),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_handler() {
        let _handler = DefaultHandler;
    }

    #[test]
    fn test_handler_factory() {
        let _python_handler = get_handler("python");
        let _go_handler = get_handler("go");
        let _default_handler = get_handler("unknown");
    }
}
