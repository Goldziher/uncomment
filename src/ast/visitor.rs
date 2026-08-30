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
    /// Indices of comments preserved *by* a `~keep` on a different comment
    /// (block extension or an above-line marker), rather than by a marker of
    /// their own. Used to tell a load-bearing marker from a redundant one.
    extended: std::collections::HashSet<usize>,
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
            extended: std::collections::HashSet::new(),
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
                    // A comment without a marker of its own owes its survival to
                    // a neighbour's marker, which makes that marker load-bearing.
                    if !self.comments[i].content(self.source).contains("~keep") {
                        self.extended.insert(i);
                    }
                    self.comments[i].should_preserve = true;
                }
            }

            run_start = run_end + 1;
        }
    }

    /// Extend `~keep` preservation from a marker comment down to the comment
    /// directly beneath it.
    ///
    /// `~keep` is plain comment text, so a marker written *inside* a doc comment
    /// is republished by every tool that consumes doc comments — rustdoc, OpenAPI
    /// schemas generated from `utoipa`, generated API clients, editor hover text.
    /// This pass gives the marker a home that never renders: a plain line comment
    /// on its own line, directly above the comment it protects.
    ///
    /// ```text
    /// // ~keep
    /// /// Parent element ID for hierarchical relationships.
    /// pub parent_id: Option<String>,
    /// ```
    ///
    /// [`Self::extend_keep_blocks`] already covers the case where both the marker
    /// and its target are standalone *single-line* comments. It cannot cover this
    /// one: a `///` node spans two rows (it swallows its trailing newline), so it
    /// never joins a single-line run. This pass matches on byte adjacency instead
    /// of row arithmetic, so it reaches doc comments and block comments alike.
    ///
    /// Scope mirrors the block pass:
    /// - Only a *standalone, non-documentation* comment acts as a marker, since a
    ///   doc comment carrying `~keep` is the very thing this exists to avoid.
    /// - Preservation runs forward through comments separated by nothing but
    ///   whitespace spanning at most one newline, so a blank line or any code
    ///   between comments ends the run.
    ///
    /// Purely additive: only ever sets `should_preserve = true`.
    pub fn extend_keep_above(&mut self) {
        let mut indices: Vec<usize> = (0..self.comments.len()).collect();
        indices.sort_by_key(|&i| (self.comments[i].start_byte, self.comments[i].end_byte));

        for position in 0..indices.len() {
            let marker = indices[position];
            if !self.is_keep_marker_line(&self.comments[marker]) {
                continue;
            }

            let mut previous_end = self.comments[marker].end_byte;
            for &next in &indices[position + 1..] {
                if !Self::gap_is_adjacent(
                    &self.source[previous_end.min(self.comments[next].start_byte)..self.comments[next].start_byte],
                ) {
                    break;
                }
                if !self.comments[next].should_preserve {
                    self.extended.insert(next);
                }
                self.comments[next].should_preserve = true;
                previous_end = previous_end.max(self.comments[next].end_byte);
            }
        }
    }

    /// Whether `comment` is a standalone, non-documentation comment whose text
    /// carries `~keep` — the form that may protect the comment below it.
    fn is_keep_marker_line(&self, comment: &CommentInfo) -> bool {
        let content = comment.content(self.source);
        !Self::is_doc_comment(comment, content) && self.is_standalone(comment) && content.contains("~keep")
    }

    /// Whether a comment is documentation, by the same content-based test the
    /// [`PreservationRule::Documentation`] rule applies. Grammar handlers leave
    /// [`CommentInfo::is_documentation`] unset for most languages — Rust records a
    /// `///` line as a plain `line_comment` — so the flag alone under-reports.
    fn is_doc_comment(comment: &CommentInfo, content: &str) -> bool {
        PreservationRule::documentation().matches(comment, content)
    }

    /// Whether the source between two comments separates them by nothing but the
    /// line break, so they read as one contiguous run. Any code, or a blank line
    /// (two or more newlines), ends the run.
    fn gap_is_adjacent(gap: &str) -> bool {
        gap.bytes().all(|byte| byte.is_ascii_whitespace()) && gap.bytes().filter(|&b| b == b'\n').count() <= 1
    }

    /// Byte ranges of `~keep` markers that sit inside a preserved documentation
    /// comment and do nothing there, together with the surrounding space that
    /// should collapse with them.
    ///
    /// Such a marker is redundant: doc comments are preserved anyway unless
    /// `--remove-doc` is set, so the token only travels outward into rendered
    /// documentation. Callers pass `remove_docs` so the one case where an in-doc
    /// marker *is* load-bearing — it is the only thing protecting this doc comment
    /// from `--remove-doc` — is left untouched.
    ///
    /// Ranges are deduplicated: a `///` line is commonly recorded twice, once as
    /// the outer comment node and once as its inner doc node.
    #[must_use]
    pub fn redundant_keep_markers(&self, remove_docs: bool) -> Vec<(usize, usize)> {
        if remove_docs {
            return Vec::new();
        }

        // A `///` line is commonly recorded twice: once as the outer comment node,
        // which still carries the `///`, and once as an inner doc node whose text
        // begins after it. The two disagree about where the doc marker ends, so a
        // span counts as a marker only when no node covering it objects.
        let mut ranges: Vec<(usize, usize)> = Vec::new();
        let mut rejected: std::collections::HashSet<usize> = std::collections::HashSet::new();
        for (index, comment) in self.comments.iter().enumerate() {
            let content = comment.content(self.source);
            if !comment.should_preserve || !Self::is_doc_comment(comment, content) {
                continue;
            }
            // A marker that is holding a neighbouring comment alive is doing work,
            // even from inside a doc comment. Leave it be.
            if self.run_members(index).any(|member| self.extended.contains(&member)) {
                continue;
            }
            if !content.contains("~keep") {
                continue;
            }
            for (offset, _) in content.match_indices("~keep") {
                let start = comment.start_byte + offset;
                if Self::is_marker_occurrence(content, offset) {
                    ranges.push(Self::widen_marker(self.source, start, start + "~keep".len()));
                } else {
                    rejected.insert(start);
                }
            }
        }

        ranges.retain(|&(start, end)| !rejected.contains(&(end - "~keep".len())) && !rejected.contains(&start));
        ranges.sort_unstable();
        ranges.dedup();
        ranges
    }

    /// Indices of comments that share a contiguous run with `index`, itself
    /// included — the comments whose survival a marker on `index` could explain.
    fn run_members(&self, index: usize) -> impl Iterator<Item = usize> + '_ {
        let anchor = &self.comments[index];
        let (low, high) = (anchor.start_row.saturating_sub(1), anchor.end_row + 1);
        (0..self.comments.len())
            .filter(move |&other| self.comments[other].start_row >= low && self.comments[other].start_row <= high)
    }

    /// Whether the `~keep` at `offset` in a doc comment is an actual marker
    /// rather than prose *about* the marker.
    ///
    /// Documentation that explains `~keep` — this crate's own docs, a README
    /// excerpt, a rustdoc example — mentions the token constantly, and rewriting
    /// those mentions would corrupt the very documentation this feature exists to
    /// protect. Three cheap signals separate the two:
    ///
    /// - A marker is a whole word. `~keepsake` is not one.
    /// - A line carrying a backtick is discussing the token, not using it, so
    ///   `` `~keep` `` and `` `/// ~keep Parent element ID.` `` are both left alone.
    /// - A `//` or `#` *inside* the doc text (past the doc marker itself) means the
    ///   line is a commented-out code sample, as in a rustdoc fenced block.
    ///
    /// The bias is deliberate: leaving a real marker in place is a cosmetic miss,
    /// while stripping a word out of prose is data loss.
    fn is_marker_occurrence(content: &str, offset: usize) -> bool {
        let end = offset + "~keep".len();
        let preceded_by_word = content[..offset]
            .chars()
            .next_back()
            .is_some_and(|c| !c.is_whitespace());
        let followed_by_word = content[end..].chars().next().is_some_and(|c| !c.is_whitespace());
        if preceded_by_word || followed_by_word {
            return false;
        }

        let line_start = content[..offset].rfind('\n').map_or(0, |pos| pos + 1);
        let line_end = content[offset..].find('\n').map_or(content.len(), |pos| offset + pos);
        let line = &content[line_start..line_end];
        if line.contains('`') {
            return false;
        }

        // Skip the doc marker that opens the line (`///`, `//!`, `##`, `/**`, `*`)
        // so only a comment marker *within* the documented text counts.
        let body = line.trim_start();
        let body_offset = line.len() - body.len();
        let text = body.trim_start_matches(['/', '!', '*', '#']);
        let text_start = line_start + body_offset + (body.len() - text.len());
        if text_start > offset {
            return false;
        }
        let before = &content[text_start..offset];
        !before.contains("//") && !before.contains('#')
    }

    /// Grow a `~keep` span to swallow one adjacent space, so stripping the token
    /// from `/// ~keep Parent element ID.` leaves `/// Parent element ID.` rather
    /// than a doubled space. Prefers the space after the marker; falls back to the
    /// one before it when the marker ends the line.
    fn widen_marker(source: &str, start: usize, end: usize) -> (usize, usize) {
        if source[end..].starts_with(' ') {
            return (start, end + 1);
        }
        if source[..start].ends_with(' ') {
            return (start - 1, end);
        }
        (start, end)
    }

    /// Whether `comment` occupies its line alone (only whitespace precedes it).
    fn is_standalone(&self, comment: &CommentInfo) -> bool {
        let line_start = self.source[..comment.start_byte].rfind('\n').map_or(0, |pos| pos + 1);
        self.source[line_start..comment.start_byte]
            .bytes()
            .all(|byte| byte.is_ascii_whitespace())
    }

    /// Whether `comment` is a single-line comment node that occupies its line
    /// alone (only whitespace precedes it). Trailing comments and multi-line
    /// (block) comment nodes return `false`.
    fn is_standalone_single_line(&self, comment: &CommentInfo) -> bool {
        comment.start_row == comment.end_row && self.is_standalone(comment)
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
