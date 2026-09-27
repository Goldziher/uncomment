use tree_sitter::Node;

/// A handler's verdict on a node whose *kind* is configured as a comment.
///
/// Node kind alone is enough for almost every grammar: a `comment` node is a comment. It is not
/// enough where one kind covers both comments and non-comments — markdown's `html_block` spans an
/// HTML comment and an embedded `<div>` alike — so a handler may look at the text and either reject
/// the node or narrow the span that counts as the comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentNodeVerdict {
    /// Not a comment after all. The node is skipped, and so is nothing else: its children are still
    /// visited, exactly as for any node that is not a comment.
    Rejected,
    /// A comment occupying this byte range, which must lie within the node.
    ///
    /// Narrowing exists because a node can carry more than the comment: markdown's `html_block`
    /// swallows its own trailing newline, and a span taken verbatim from the node would, after the
    /// usual whole-line expansion, delete the blank line after the comment too — merging the blocks
    /// around it.
    Accepted { start_byte: usize, end_byte: usize },
}

pub trait LanguageHandler {
    fn is_documentation_comment(&self, node: &Node, parent: Option<Node>, source: &str) -> Option<bool>;

    fn should_preserve_comment(&self, node: &Node, parent: Option<Node>, source: &str) -> Option<bool>;

    /// Whether a node whose kind is configured as a comment really is one, and over which bytes.
    ///
    /// `None` — the default — means "no opinion": the node is a comment because its kind says so,
    /// spanning the whole node. Only a grammar whose comment kind is ambiguous needs to override
    /// this.
    fn classify_comment_node(&self, _node: &Node, _parent: Option<Node>, _source: &str) -> Option<CommentNodeVerdict> {
        None
    }
}

pub struct DefaultHandler;

impl LanguageHandler for DefaultHandler {
    fn is_documentation_comment(&self, _node: &Node, _parent: Option<Node>, _source: &str) -> Option<bool> {
        None
    }

    fn should_preserve_comment(&self, _node: &Node, _parent: Option<Node>, _source: &str) -> Option<bool> {
        None
    }
}

pub struct PythonHandler;

impl LanguageHandler for PythonHandler {
    fn is_documentation_comment(&self, node: &Node, parent: Option<Node>, _source: &str) -> Option<bool> {
        if node.kind() != "string" {
            return None;
        }

        let parent = parent?;

        if parent.kind() == "expression_statement" {
            let grandparent = parent.parent()?;
            match grandparent.kind() {
                "module" => Some(self.is_first_statement(&parent, &grandparent)),
                "block" => {
                    if let Some(block_parent) = grandparent.parent() {
                        match block_parent.kind() {
                            "function_definition" | "async_function_definition" | "class_definition" => {
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
        } else {
            match parent.kind() {
                "module" => Some(self.is_first_statement(node, &parent)),
                "block" => {
                    if let Some(block_parent) = parent.parent() {
                        match block_parent.kind() {
                            "function_definition" | "async_function_definition" | "class_definition" => {
                                Some(self.is_first_statement(node, &parent))
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

    fn should_preserve_comment(&self, _node: &Node, _parent: Option<Node>, _source: &str) -> Option<bool> {
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
    fn is_documentation_comment(&self, node: &Node, parent: Option<Node>, _source: &str) -> Option<bool> {
        if node.kind() != "comment" {
            return None;
        }

        if self.precedes_declaration(node, parent) {
            Some(true)
        } else {
            Some(false)
        }
    }

    fn should_preserve_comment(&self, node: &Node, parent: Option<Node>, source: &str) -> Option<bool> {
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

    fn find_next_non_comment_sibling<'a>(&self, comment_node: &Node, parent: &Node<'a>) -> Option<Node<'a>> {
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
        "c" | "cpp" | "objc" => Box::new(CFamilyHandler),
        "markdown" => Box::new(MarkdownHandler),
        _ => Box::new(DefaultHandler),
    }
}

/// Markdown, whose grammar emits no `comment` node at all.
///
/// An HTML comment is raw HTML as far as CommonMark is concerned, so `<!-- … -->` surfaces as an
/// `html_block` — the same node kind as a `<div align="center">` badge row or a `<details>` block.
/// Declaring `html_block` as a comment kind without this classifier would delete embedded HTML, so
/// the kind is only half the test: the block's text has to be *nothing but* an HTML comment.
pub struct MarkdownHandler;

impl LanguageHandler for MarkdownHandler {
    fn is_documentation_comment(&self, _node: &Node, _parent: Option<Node>, _source: &str) -> Option<bool> {
        None
    }

    fn should_preserve_comment(&self, _node: &Node, _parent: Option<Node>, _source: &str) -> Option<bool> {
        None
    }

    fn classify_comment_node(&self, node: &Node, _parent: Option<Node>, source: &str) -> Option<CommentNodeVerdict> {
        if node.kind() != "html_block" {
            return None;
        }

        let Ok(text) = node.utf8_text(source.as_bytes()) else {
            return Some(CommentNodeVerdict::Rejected);
        };

        Some(match Self::comment_bounds(text) {
            Some((start, end)) => CommentNodeVerdict::Accepted {
                start_byte: node.start_byte() + start,
                end_byte: node.start_byte() + end,
            },
            None => CommentNodeVerdict::Rejected,
        })
    }
}

impl MarkdownHandler {
    /// The offsets within an `html_block`'s text of the HTML comment that is the whole of it, or
    /// `None` when the block is not purely a comment.
    ///
    /// Three shapes have to be turned away, and each would cost real content if it were not:
    ///
    /// - **Anything but a comment first.** `<div>…</div>` is embedded HTML; so is a `<!DOCTYPE>`.
    /// - **An unterminated `<!--`.** CommonMark runs such a block to the end of the document, so its
    ///   node spans every line that follows. Deleting it would delete the rest of the file.
    /// - **Anything after the closing `-->`.** CommonMark ends the block at the *line* carrying
    ///   `-->`, so `<!-- x --><div>kept</div>` and `<!-- x --> trailing text` are each one node
    ///   holding a comment and something else. A second comment on the same line lands here too, so
    ///   `<!-- a --> <!-- b -->` is left alone rather than half-deleted.
    ///
    /// Surrounding whitespace is trimmed off the accepted span: an `html_block` carries both the
    /// comment's leading indentation and its trailing newline, and keeping the newline would make
    /// the removal eat the blank line after the comment as well.
    fn comment_bounds(text: &str) -> Option<(usize, usize)> {
        const OPEN: &str = "<!--";
        const CLOSE: &str = "-->";

        let start = text.len() - text.trim_start().len();
        let body = &text[start..];
        if !body.starts_with(OPEN) {
            return None;
        }

        let close = body.find(CLOSE)? + CLOSE.len();
        if !body[close..].trim().is_empty() {
            return None;
        }

        Some((start, start + close))
    }
}

pub struct CFamilyHandler;

impl LanguageHandler for CFamilyHandler {
    fn is_documentation_comment(&self, _node: &Node, _parent: Option<Node>, _source: &str) -> Option<bool> {
        None
    }

    fn should_preserve_comment(&self, node: &Node, _parent: Option<Node>, source: &str) -> Option<bool> {
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

        let line_start = match memchr::memrchr(b'\n', &source.as_bytes()[..start]) {
            Some(pos) => pos + 1,
            None => 0,
        };

        let before = &source[line_start..start];
        before.trim_start().starts_with('#')
    }
}

pub struct RubyHandler;

impl LanguageHandler for RubyHandler {
    fn is_documentation_comment(&self, node: &Node, parent: Option<Node>, source: &str) -> Option<bool> {
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

    fn should_preserve_comment(&self, node: &Node, _parent: Option<Node>, source: &str) -> Option<bool> {
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

        let magic_prefixes = ["# frozen_string_literal:", "# encoding:", "# coding:", "# typed:"];

        if magic_prefixes.iter().any(|prefix| trimmed.starts_with(prefix)) {
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

        matches!(next_sibling.kind(), "method" | "singleton_method" | "class" | "module")
    }

    fn find_next_non_comment_sibling<'a>(&self, comment_node: &Node, parent: &Node<'a>) -> Option<Node<'a>> {
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

    /// The `html_block` predicate, stated as a table. Every rejected case is an `html_block` whose
    /// text would cost real content if the node were deleted.
    #[test]
    fn markdown_comment_bounds_accepts_only_a_whole_comment() {
        // (block text, the comment within it, or None to reject the block)
        let cases: &[(&str, Option<&str>)] = &[
            ("<!-- an aside -->\n", Some("<!-- an aside -->")),
            ("   <!-- indented -->\n", Some("<!-- indented -->")),
            ("<!-- c -->\r\n", Some("<!-- c -->")),
            ("<!-- last -->", Some("<!-- last -->")),
            ("<!--\nmulti\nline\n-->\n", Some("<!--\nmulti\nline\n-->")),
            ("<!-- <div>hi</div> -->\n", Some("<!-- <div>hi</div> -->")),
            ("<!-- a -- b -->\n", Some("<!-- a -- b -->")),
            // Embedded HTML, never a comment.
            ("<div align=\"center\">\n</div>\n\n", None),
            ("<details>\n<summary>x</summary>\n\n", None),
            ("<!DOCTYPE html>\n", None),
            ("<div>\n<!-- inner -->\n</div>\n", None),
            ("<div>\n</div>\n<!-- after -->\n", None),
            // Unterminated: the block runs to the end of the document.
            ("<!-- never closed\n\nreal content\n", None),
            // A comment sharing its block with something else.
            ("<!-- c --><div>kept</div>\n", None),
            ("<!-- c --> trailing text\n", None),
            ("<!-- a --> <!-- b -->\n", None),
        ];

        for (text, expected) in cases {
            let bounds = MarkdownHandler::comment_bounds(text);
            let found = bounds.map(|(start, end)| &text[start..end]);
            assert_eq!(found, *expected, "comment_bounds({text:?})");
        }
    }
}
