use tree_sitter::Node;

pub trait LanguageHandler {
    fn is_documentation_comment(
        &self,
        node: &Node,
        parent: Option<Node>,
        source: &str,
    ) -> Option<bool>;

    fn should_preserve_comment(
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

    fn should_preserve_comment(
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

    fn should_preserve_comment(
        &self,
        _node: &Node,
        _parent: Option<Node>,
        _source: &str,
    ) -> Option<bool> {
        None
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

    fn should_preserve_comment(
        &self,
        node: &Node,
        parent: Option<Node>,
        source: &str,
    ) -> Option<bool> {
        if node.kind() != "comment" {
            return None;
        }

        let Ok(text) = node.utf8_text(source.as_bytes()) else {
            return None;
        };

        if self.is_go_directive_comment(text) {
            return Some(true);
        }

        if self.precedes_cgo_import(node, parent, source) {
            return Some(true);
        }

        None
    }
}

impl GoHandler {
    fn is_go_directive_comment(&self, comment_text: &str) -> bool {
        let trimmed = comment_text.trim_start();
        trimmed.starts_with("//go:")
            || trimmed.starts_with("/*go:")
            || trimmed.starts_with("// +build")
            || trimmed.starts_with("//+build")
            || trimmed.starts_with("//line ")
            || trimmed.starts_with("/*line ")
    }

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

    fn precedes_cgo_import(&self, comment_node: &Node, parent: Option<Node>, source: &str) -> bool {
        let parent = match parent {
            Some(p) => p,
            None => return false,
        };

        let Some(next_sibling) = self.find_next_non_comment_sibling(comment_node, &parent) else {
            return false;
        };

        if next_sibling.kind() != "import_declaration" {
            return false;
        }

        self.import_declaration_includes_c(&next_sibling, source)
    }

    fn import_declaration_includes_c(&self, import_decl: &Node, source: &str) -> bool {
        let Ok(text) = import_decl.utf8_text(source.as_bytes()) else {
            return false;
        };

        text.contains("\"C\"") || text.contains("`C`")
    }
}

pub fn get_handler(language_name: &str) -> Box<dyn LanguageHandler> {
    match language_name.to_lowercase().as_str() {
        "python" => Box::new(PythonHandler),
        "go" => Box::new(GoHandler),
        "ruby" => Box::new(RubyHandler),
        "c" | "cpp" => Box::new(CFamilyHandler),
        _ => Box::new(DefaultHandler),
    }
}

pub struct CFamilyHandler;

impl LanguageHandler for CFamilyHandler {
    fn is_documentation_comment(
        &self,
        _node: &Node,
        _parent: Option<Node>,
        _source: &str,
    ) -> Option<bool> {
        None
    }

    fn should_preserve_comment(
        &self,
        node: &Node,
        _parent: Option<Node>,
        source: &str,
    ) -> Option<bool> {
        if node.kind() != "comment" {
            return None;
        }

        if self.is_trailing_preprocessor_comment(node, source) {
            return Some(true);
        }

        None
    }
}

impl CFamilyHandler {
    fn is_trailing_preprocessor_comment(&self, node: &Node, source: &str) -> bool {
        let start = node.start_byte();
        if start > source.len() {
            return false;
        }

        let bytes = source.as_bytes();
        let mut line_start = start;
        while line_start > 0 && bytes[line_start - 1] != b'\n' {
            line_start -= 1;
        }

        let before = &source[line_start..start];
        before.trim_start().starts_with('#')
    }
}

pub struct RubyHandler;

impl LanguageHandler for RubyHandler {
    fn is_documentation_comment(
        &self,
        node: &Node,
        parent: Option<Node>,
        source: &str,
    ) -> Option<bool> {
        if node.kind() != "comment" {
            return None;
        }

        let Ok(text) = node.utf8_text(source.as_bytes()) else {
            return None;
        };

        if self.looks_like_yard_documentation(text) {
            return Some(true);
        }

        if self.precedes_declaration(node, parent) {
            return Some(true);
        }

        Some(false)
    }

    fn should_preserve_comment(
        &self,
        node: &Node,
        _parent: Option<Node>,
        source: &str,
    ) -> Option<bool> {
        if node.kind() != "comment" {
            return None;
        }

        let Ok(text) = node.utf8_text(source.as_bytes()) else {
            return None;
        };

        let trimmed = text.trim_start();
        if !trimmed.starts_with('#') {
            return None;
        }

        let magic_prefixes = [
            "# frozen_string_literal:",
            "# encoding:",
            "# coding:",
            "# typed:",
        ];

        if magic_prefixes
            .iter()
            .any(|prefix| trimmed.starts_with(prefix))
        {
            return Some(true);
        }

        None
    }
}

impl RubyHandler {
    fn looks_like_yard_documentation(&self, comment_text: &str) -> bool {
        let trimmed = comment_text.trim_start();
        trimmed.starts_with("# @") || trimmed.starts_with("# @!")
    }

    fn precedes_declaration(&self, comment_node: &Node, parent: Option<Node>) -> bool {
        let parent = match parent {
            Some(p) => p,
            None => return false,
        };

        let Some(next_sibling) = self.find_next_non_comment_sibling(comment_node, &parent) else {
            return false;
        };

        matches!(
            next_sibling.kind(),
            "method" | "singleton_method" | "class" | "module"
        )
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
