//! The three rules, evaluated against one comment at a time.
//!
//! Every input is an [`InspectedComment`] from [`crate::processor::Processor::inspect`], never raw
//! file text. That is the whole reason this tool carries tree-sitter: `let s = "TODO: fix";` is a
//! string literal, the parser knows it, and a linter that grepped for `TODO` would flag it.
//!
//! A finding carries absolute byte offsets and, when the rule is fixable, the [`Edit`]s that fix it.
//! Converting an offset to a line and column needs the file's text and belongs to the caller, which
//! keeps everything here a pure function of the comment.

use crate::edit::Edit;
use crate::lint::config::{LintConfig, Rule, Severity};
use crate::processor::{CommentKind, InspectedComment};
use crate::rules::preservation::is_documentation_syntax;

/// The longest excerpt reported for one finding, in characters.
const EXCERPT_LIMIT: usize = 120;

/// One tag occurrence inside a comment, with everything the rules need to judge it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagSite {
    /// Absolute byte offset of the tag token in the file.
    pub start: usize,
    /// Absolute byte offset just past the tag token.
    pub end: usize,
    pub tag: String,
    pub key: Option<String>,
    /// A `(...)` group follows the tag, whatever it holds.
    pub has_group: bool,
    /// A `:` follows the tag, past any `(...)` group.
    pub has_colon: bool,
    pub excerpt: String,
}

/// A rule violation at one tag site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub rule: Rule,
    pub severity: Severity,
    /// Absolute byte offset of the tag token, for line and column reporting.
    pub offset: usize,
    pub tag: String,
    pub key: Option<String>,
    pub message: String,
    pub excerpt: String,
    /// Empty when the rule cannot be fixed automatically.
    pub edits: Vec<Edit>,
}

impl Finding {
    pub fn is_fixable(&self) -> bool {
        !self.edits.is_empty()
    }
}

/// Every tag occurrence in `comment`, at most one per line of the comment's text.
///
/// One per line because a tag comment is a single statement about a single piece of work:
/// `// TODO(K-1): drop this once FIXME above is gone` mentions `FIXME` in prose and must not be read
/// as a second, unkeyed tag. A block comment with a tag on each of three lines is three sites.
pub fn tag_sites(comment: &InspectedComment, config: &LintConfig) -> Vec<TagSite> {
    if !config.include_doc_comments && is_documentation(comment) {
        return Vec::new();
    }
    let text = &comment.text;
    let mut sites = Vec::new();
    let mut claimed_line: Option<usize> = None;

    for matched in config.tag_pattern.find_iter(text) {
        let line_index = text[..matched.start()].matches('\n').count();
        if claimed_line == Some(line_index) {
            continue;
        }
        if !is_tag_site(text, &matched, comment, config) {
            continue;
        }
        claimed_line = Some(line_index);

        let from_tag = &text[matched.start()..];
        let after_tag = &text[matched.end()..];

        sites.push(TagSite {
            start: comment.start_byte + matched.start(),
            end: comment.start_byte + matched.end(),
            tag: matched.as_str().to_string(),
            key: config.extract_key(from_tag).map(str::to_string),
            has_group: group_follows(after_tag),
            has_colon: colon_follows(after_tag),
            excerpt: excerpt_of(text, matched.start()),
        });
    }

    sites
}

/// Findings for one comment.
///
/// `current_issue` is the issue key derived from the branch, `None` when there is none to compare
/// against — outside a repository, on a detached `HEAD`, or on a branch carrying no key. In that case
/// `todo-self-reference` is silently not evaluated; the caller reports the reason as a note, because
/// a linter that failed a run over an unrelated fact about the checkout would be unusable in CI.
///
/// `todo_key` is `--todo-key`: the one way a missing key can be fixed, since a key cannot be
/// invented.
pub fn check(
    comment: &InspectedComment,
    config: &LintConfig,
    current_issue: Option<&str>,
    todo_key: Option<&str>,
) -> Vec<Finding> {
    let mut findings = Vec::new();

    for site in tag_sites(comment, config) {
        let canonical = site.tag == config.canonical_tag;

        if !canonical && config.is_on(Rule::TagNotCanonical) {
            findings.push(Finding {
                rule: Rule::TagNotCanonical,
                severity: config.severity(Rule::TagNotCanonical),
                offset: site.start,
                tag: site.tag.clone(),
                key: site.key.clone(),
                message: format!("`{}` should be written as `{}`", site.tag, config.canonical_tag),
                excerpt: site.excerpt.clone(),
                edits: vec![Edit::new(site.start, site.end, config.canonical_tag.clone())],
            });
        }

        match &site.key {
            None => {
                if config.is_on(Rule::TodoMissingKey) {
                    findings.push(Finding {
                        rule: Rule::TodoMissingKey,
                        severity: config.severity(Rule::TodoMissingKey),
                        offset: site.start,
                        tag: site.tag.clone(),
                        key: None,
                        message: format!("`{}` carries no issue key", site.tag),
                        excerpt: site.excerpt.clone(),
                        edits: missing_key_edits(&site, todo_key),
                    });
                }
            }
            Some(key) => {
                if config.is_on(Rule::TodoSelfReference) && current_issue.is_some_and(|current| current == key) {
                    findings.push(Finding {
                        rule: Rule::TodoSelfReference,
                        severity: config.severity(Rule::TodoSelfReference),
                        offset: site.start,
                        tag: site.tag.clone(),
                        key: Some(key.clone()),
                        message: format!(
                            "`{key}` is the issue this change is being made under, which closes on merge — \
                             name a follow-up issue instead"
                        ),
                        excerpt: site.excerpt.clone(),
                        edits: Vec::new(),
                    });
                }
            }
        }
    }

    findings
}

/// The edit that supplies an explicit key, or nothing.
///
/// Refuses to act when a `(...)` group already follows the tag: the key pattern rejected whatever is
/// in there, and prepending a second group would produce `TODO(K-1)(whatever):`. The colon is added
/// only when there is not one already, which is what makes `--fix --todo-key` idempotent: the result
/// matches the key pattern, so the second run finds nothing to do.
fn missing_key_edits(site: &TagSite, todo_key: Option<&str>) -> Vec<Edit> {
    let Some(key) = todo_key else {
        return Vec::new();
    };
    if site.has_group {
        return Vec::new();
    }

    let suffix = if site.has_colon { "" } else { ":" };
    vec![Edit::insert(site.end, format!("({key}){suffix}"))]
}

/// Characters that quote rather than decorate. A tag word right after one is being named, as in
/// `# "TODO" is the canonical tag`, not used.
const QUOTING_CHARS: [char; 3] = ['`', '"', '\''];

/// Comment markers that open a trailing segment inside a comment's own text, as in
/// `# noqa: T201  # TODO: fix T201` or ESLint's `// eslint-disable-line x -- TODO: y`. The parser
/// sees one comment there; a reader sees a directive followed by a tag comment.
const SEGMENT_MARKERS: [&str; 3] = ["#", "//", "--"];

/// Punctuation that ends a clause, so a tag written straight after it starts a statement of its own:
/// `handling - TODO: x`, `compatibility, TODO: x`, `it. TODO: x`, `[CP] TODO x`.
///
/// `=` and `/` are left out on purpose. `?tenant_id=XXX` is a placeholder in a URL, not a tag.
const CLAUSE_BOUNDARIES: [char; 16] = [
    '.', ',', ';', ':', '!', '?', '-', '\u{2013}', '\u{2014}', '(', ')', '[', ']', '{', '}', '$',
];

/// Dashes that separate a tag from its text, as in `# FIXME - x`.
const DASHES: [char; 3] = ['-', '\u{2013}', '\u{2014}'];

/// Whether a tag match is a tag at all, rather than the same word used in prose.
///
/// A tag in *head position* is always one: the first word of its line of the comment, or the first
/// word of a trailing segment another comment marker opens. Everything between that start and the
/// tag must be delimiter or decoration — `//`, `#`, `/*`, a `*` block continuation, a `-` bullet —
/// which a missing word character tests for every comment syntax at once, where an allowlist of
/// punctuation would have to be kept in step with the language list.
///
/// Anywhere else the tag has to be spelled exactly as configured *and* written the way a tag is
/// written: introducing its text (`pkt TODO: add`, `NOTE TODO(K-1):`, `insert TODO - remove`) or
/// opening a clause (`handling - TODO fix`, `noBarrelFile: TODO remove`). What that rejects is the
/// tag word used as a noun — `# Line contains TODO`, `# Invalid TODO tag`, `canonical TODO,` — the
/// shape of every ruff rule description and every sentence about the convention itself. A miscased
/// tag gets no mid-comment reading at all: `todo`, `hack` and `xxx` are English, and on an 89k-file
/// monorepo 165 of the 183 miscased occurrences were prose.
///
/// In either position a quoted tag word is named rather than used: right after a quote, or inside a
/// backtick span.
/// The Doxygen doc-comment opener for `#`-comment languages. Preservation counts it as
/// documentation, but in Python, YAML and shell `## TODO:` is an ordinary, emphasised comment, and
/// a real monorepo writes it that way far more often than as Doxygen.
const DOUBLE_HASH: &str = "##";

/// Whether a comment is documentation by its syntax: a docstring, or a doc comment whose own
/// delimiter or node kind says so. A plain `// TODO` that a handler files as documentation only
/// because it sits directly above a declaration, as Go's does, stays in scope, and so does `##`.
fn is_documentation(comment: &InspectedComment) -> bool {
    match comment.kind {
        CommentKind::Docstring => true,
        CommentKind::Doc => {
            !comment.text.trim_start().starts_with(DOUBLE_HASH)
                && is_documentation_syntax(&comment.node_type, &comment.text)
        }
        CommentKind::Line | CommentKind::Block => false,
    }
}

fn is_tag_site(text: &str, matched: &regex::Match<'_>, comment: &InspectedComment, config: &LintConfig) -> bool {
    let offset = matched.start();
    let line_start = text[..offset].rfind('\n').map_or(0, |index| index + 1);
    let mut before = &text[line_start..offset];
    if line_start == 0 && comment.kind == CommentKind::Docstring {
        before = without_string_opener(before);
    }
    let after_tag = &text[matched.end()..];
    let rest_of_line = after_tag.split('\n').next().unwrap_or_default();
    if is_quoted(before, rest_of_line) {
        return false;
    }

    let segment = trailing_segment(before);
    if segment.chars().all(|c| !is_word_char(c) && !QUOTING_CHARS.contains(&c)) {
        return true;
    }

    let spelled_as_configured = config.tags.iter().any(|tag| tag == matched.as_str());
    spelled_as_configured && (introduces_text(after_tag) || opens_clause(segment, after_tag))
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Whether the tag between `before` and `rest_of_line` sits inside quotes: straight after a quote
/// character, or inside a backtick code span on its line.
fn is_quoted(before: &str, rest_of_line: &str) -> bool {
    if before.chars().next_back().is_some_and(|c| QUOTING_CHARS.contains(&c)) {
        return true;
    }
    open_code_span(before).is_some_and(|opener| backtick_runs(rest_of_line).any(|run| run == opener))
}

/// Length of the backtick run that opens a code span still unclosed at the end of `before`.
///
/// A span closes only on a run of the same length, as in Markdown, so a lone ```` ``` ```` mentioned
/// in prose opens a span nothing closes rather than flipping the quoting of everything after it.
fn open_code_span(before: &str) -> Option<usize> {
    backtick_runs(before).fold(None, |open, run| match open {
        Some(opener) if opener == run => None,
        None => Some(run),
        still_open => still_open,
    })
}

fn backtick_runs(text: &str) -> impl Iterator<Item = usize> + '_ {
    text.split(|c| c != '`').filter(|run| !run.is_empty()).map(str::len)
}

/// Whether the text after a tag is the tag's own text being introduced: a `:`, a `(…)` group, or a
/// spaced dash, past any blanks.
fn introduces_text(after_tag: &str) -> bool {
    let rest = skip_blanks(after_tag);
    if rest.starts_with([':', '(']) {
        return true;
    }
    let mut chars = rest.chars();
    rest.len() < after_tag.len()
        && chars.next().is_some_and(|c| DASHES.contains(&c))
        && chars.next().is_none_or(char::is_whitespace)
}

/// Whether the tag follows a clause boundary and is followed by a blank or the end of its line,
/// which is how a tag opening a statement mid-comment reads — and how `{"uuid": XXX}`, a
/// placeholder hugging its closing brace, does not.
fn opens_clause(segment: &str, after_tag: &str) -> bool {
    let boundary = segment
        .trim_end()
        .chars()
        .next_back()
        .is_some_and(|c| CLAUSE_BOUNDARIES.contains(&c));
    boundary && after_tag.chars().next().is_none_or(char::is_whitespace)
}

/// `before` from just past the last [`SEGMENT_MARKERS`] entry that follows whitespace, or all of it.
///
/// Requiring whitespace before the marker is what keeps `issue#123` and `https://x` from opening a
/// segment.
fn trailing_segment(before: &str) -> &str {
    let mut segment_start = 0;
    for (index, c) in before.char_indices() {
        if !c.is_whitespace() {
            continue;
        }
        let after_blank = index + c.len_utf8();
        if let Some(marker) = SEGMENT_MARKERS
            .iter()
            .find(|marker| before[after_blank..].starts_with(*marker))
        {
            segment_start = after_blank + marker.len();
        }
    }
    &before[segment_start..]
}

/// The first line of a docstring past its opening quotes and any `r`/`b`/`f`/`u` string prefix,
/// which are the docstring's delimiter rather than a quotation of the tag.
fn without_string_opener(before: &str) -> &str {
    let unprefixed = before
        .trim_start()
        .trim_start_matches(|c: char| c.is_ascii_alphabetic());
    if unprefixed.starts_with(['"', '\'']) {
        unprefixed.trim_start_matches(['"', '\''])
    } else {
        before
    }
}

/// Whether a `(…)` group opens immediately after the tag, ignoring horizontal whitespace.
fn group_follows(after_tag: &str) -> bool {
    skip_blanks(after_tag).starts_with('(')
}

/// Whether a `:` follows the tag, past at most one `(…)` group.
///
/// Hand-rolled rather than a regex because both shapes are fixed: there is no configuration here to
/// get wrong, and a hardcoded pattern would need a panicking `unwrap` to compile.
fn colon_follows(after_tag: &str) -> bool {
    let rest = skip_blanks(after_tag);
    let rest = match rest.strip_prefix('(') {
        Some(inside) => match inside.find([')', '\n']) {
            Some(index) if inside.as_bytes().get(index) == Some(&b')') => &inside[index + 1..],
            _ => return false,
        },
        None => rest,
    };
    skip_blanks(rest).starts_with(':')
}

fn skip_blanks(text: &str) -> &str {
    text.trim_start_matches([' ', '\t'])
}

/// The line of `text` holding `offset`, trimmed and length-capped.
fn excerpt_of(text: &str, offset: usize) -> String {
    let line_start = text[..offset].rfind('\n').map_or(0, |index| index + 1);
    let line_end = text[offset..].find('\n').map_or(text.len(), |index| offset + index);
    let line = text[line_start..line_end].trim();

    if line.chars().count() <= EXCERPT_LIMIT {
        return line.to_string();
    }

    let truncated: String = line.chars().take(EXCERPT_LIMIT).collect();
    format!("{truncated}…")
}

#[cfg(test)]
mod tests {
    use crate::lint::config::LintTable;
    use crate::processor::Verdict;

    use super::*;

    fn config() -> LintConfig {
        let table: LintTable = toml::from_str("enabled = true\n").expect("valid table");
        LintConfig::from_table(&table, None).expect("valid config")
    }

    fn config_from(toml: &str) -> LintConfig {
        let table: LintTable = toml::from_str(toml).expect("valid table");
        LintConfig::from_table(&table, None).expect("valid config")
    }

    /// A comment as `inspect` would report it, placed at `start` in some file.
    fn comment(text: &str, start: usize) -> InspectedComment {
        InspectedComment {
            start_byte: start,
            end_byte: start + text.len(),
            start_row: 0,
            end_row: text.matches('\n').count(),
            node_type: "line_comment".to_string(),
            kind: CommentKind::Line,
            verdict: Verdict::Preserve,
            reason: None,
            text: text.to_string(),
            is_documentation: false,
        }
    }

    fn rules_fired(text: &str, current_issue: Option<&str>) -> Vec<Rule> {
        check(&comment(text, 0), &config(), current_issue, None)
            .into_iter()
            .map(|finding| finding.rule)
            .collect()
    }

    #[test]
    fn a_keyed_canonical_tag_is_clean() {
        assert!(rules_fired("// TODO(AMVP-1): later", None).is_empty());
        assert!(rules_fired("# TODO(PPSC-42): later", None).is_empty());
    }

    #[test]
    fn a_tag_without_a_key_is_missing_one() {
        assert_eq!(rules_fired("// TODO: later", None), vec![Rule::TodoMissingKey]);
        assert_eq!(rules_fired("# TODO later", None), vec![Rule::TodoMissingKey]);
    }

    #[test]
    fn a_non_canonical_tag_fires_both_rules_when_it_also_lacks_a_key() {
        assert_eq!(
            rules_fired("// FIXME: later", None),
            vec![Rule::TagNotCanonical, Rule::TodoMissingKey]
        );
        assert_eq!(
            rules_fired("// FIXME(AMVP-1): later", None),
            vec![Rule::TagNotCanonical]
        );
        assert_eq!(rules_fired("// HACK(AMVP-1): later", None), vec![Rule::TagNotCanonical]);
        assert_eq!(rules_fired("// XXX(AMVP-1): later", None), vec![Rule::TagNotCanonical]);
    }

    #[test]
    fn a_key_equal_to_the_current_issue_is_a_self_reference() {
        assert_eq!(
            rules_fired("// TODO(AMVP-160815): later", Some("AMVP-160815")),
            vec![Rule::TodoSelfReference]
        );
        assert!(rules_fired("// TODO(AMVP-999999): later", Some("AMVP-160815")).is_empty());
        assert!(
            rules_fired("// TODO(AMVP-160815): later", None).is_empty(),
            "with no current issue there is nothing to compare against"
        );
    }

    #[test]
    fn a_self_referencing_non_canonical_tag_reports_both() {
        assert_eq!(
            rules_fired("// FIXME(AMVP-160815): later", Some("AMVP-160815")),
            vec![Rule::TagNotCanonical, Rule::TodoSelfReference]
        );
    }

    #[test]
    fn a_rule_set_to_off_is_not_evaluated() {
        let config = config_from("enabled = true\n[rules]\ntodo-missing-key = 'off'\n");
        let findings = check(&comment("// FIXME: later", 0), &config, None, None);
        assert_eq!(
            findings.iter().map(|f| f.rule).collect::<Vec<_>>(),
            vec![Rule::TagNotCanonical]
        );
    }

    #[test]
    fn a_warn_severity_is_carried_onto_the_finding() {
        let config = config_from("enabled = true\n[rules]\ntodo-missing-key = 'warn'\n");
        let findings = check(&comment("// TODO: later", 0), &config, None, None);
        assert_eq!(findings[0].severity, Severity::Warn);
    }

    #[test]
    fn one_tag_per_line_so_prose_mentioning_another_tag_is_not_a_second_site() {
        let sites = tag_sites(
            &comment("// TODO(AMVP-1): drop once the FIXME above is gone", 0),
            &config(),
        );
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].tag, "TODO");
    }

    #[test]
    fn a_block_comment_reports_one_site_per_tagged_line() {
        let text = "/*\n * TODO: first\n * plain prose\n * FIXME(AMVP-2): second\n */";
        let sites = tag_sites(&comment(text, 0), &config());
        assert_eq!(
            sites.iter().map(|site| site.tag.as_str()).collect::<Vec<_>>(),
            vec!["TODO", "FIXME"]
        );
        assert_eq!(sites[0].key, None);
        assert_eq!(sites[1].key.as_deref(), Some("AMVP-2"));
    }

    /// Measured on a 89k-file monorepo: of 183 lowercase or titlecase occurrences of the default
    /// tag words in code files, only 18 opened a comment. The other 165 were English — "this is a
    /// hack to work around…" — and every one of them would have been rewritten into `TODO`.
    #[test]
    fn a_miscased_tag_word_inside_prose_is_not_a_tag() {
        for text in [
            "// this is a hack to work around the upstream bug",
            "# the todo list lives in Jira",
            "// fixing that is a Hack, but it works",
            "# marked xxx in the vendor dump",
        ] {
            assert!(
                tag_sites(&comment(text, 0), &config()).is_empty(),
                "prose should yield no tag site: {text:?}"
            );
        }
    }

    #[test]
    fn a_miscased_tag_that_opens_the_comment_is_a_tag() {
        for (text, expected) in [
            ("// todo: later", "todo"),
            ("#fixme later", "fixme"),
            ("/* Hack: works around the driver */", "Hack"),
            ("   ///   xxx placeholder", "xxx"),
            ("* todo: inside a block continuation", "todo"),
        ] {
            let sites = tag_sites(&comment(text, 0), &config());
            assert_eq!(sites.len(), 1, "expected one site in {text:?}, got {sites:?}");
            assert_eq!(sites[0].tag, expected);
        }
    }

    /// Casing is no excuse either: a tag word used as a noun is being talked about, not used. Every
    /// one of these was reported, with an unkeyed-TODO fix on offer, in a real monorepo.
    #[test]
    fn a_canonically_spelled_tag_used_as_a_noun_is_prose_not_a_tag() {
        for text in [
            "# Line contains TODO",
            "# Line contains FIXME, consider resolving the issue",
            "# Line contains XXX",
            "# Missing author in TODO",
            "# Invalid TODO tag: `FIXME`",
            "# [*] Invalid TODO capitalization: `Todo` should be `TODO`",
            "# needs — canonical TODO, case-insensitive matching, a key pattern that accepts",
            "# usage: /api/devices/daily/aggregations/column/_search?tenant_id=XXX&utc_offset=2",
            "# usage: /api/devices/hourly/aggregations/table/_search?tenant_id=XXX",
            "# returns {\"uuid\": XXX} for the next page",
            "# Missing author in TODO; try: `# TODO(<author_name>): ...`",
        ] {
            assert!(
                tag_sites(&comment(text, 0), &config()).is_empty(),
                "prose should yield no tag site: {text:?}"
            );
        }
    }

    #[test]
    fn a_tag_word_in_backticks_or_quotes_is_quoted_not_a_tag() {
        for text in [
            "// `// TODO AMVP-...` markers — same model as ruff.toml per-file-ignores.",
            "# `TODO` is the canonical tag",
            "# \"FIXME\" is rewritten to TODO",
            "// 'XXX' is a placeholder",
            "# ``TODO`` in a double-backtick span",
        ] {
            assert!(
                tag_sites(&comment(text, 0), &config()).is_empty(),
                "a quoted tag word should yield no tag site: {text:?}"
            );
        }
    }

    /// Deliberate: a label before the tag does not make it prose. `// biome-ignore rule: TODO remove
    /// this`, `* @deprecated TODO: x` and `# [CP] TODO: x` are how real tags are written mid-comment;
    /// on a large monorepo a first-word-only rule lost 40 of them, and not one `Label: TODO` was prose.
    #[test]
    fn a_tag_after_a_label_or_clause_is_a_tag_because_real_tags_are_written_that_way() {
        for (text, expected) in [
            ("# Note: TODO later", "TODO"),
            ("// see also FIXME: the driver bug", "FIXME"),
            (
                "// biome-ignore lint/performance/noBarrelFile: TODO remove this barrel file",
                "TODO",
            ),
            (" * @deprecated TODO: do this generically", "TODO"),
            ("# NOTE TODO: temporary fix", "TODO"),
            ("# [12/2/23 WP] TODO figure out how this is happening", "TODO"),
            ("# Socks Exceptions handling - TODO: Create new Failure Type", "TODO"),
            ("# for backwards compatibility, TODO: delete after the rollout", "TODO"),
            (
                "# Deprecated! TODO - Remove after all terraforms use the per env one",
                "TODO",
            ),
            ("# in case of insert TODO - remove when export uses status", "TODO"),
            ("# Integrations (TODO: Change to GA)", "TODO"),
            (
                "# image_cluster=asset.get(\"cluster_name\"), TODO(sandu): SI-3573 use tags?",
                "TODO",
            ),
            (
                "// Removes ``` as Jira complains about it - TODO - replace with code block syntax",
                "TODO",
            ),
        ] {
            let sites = tag_sites(&comment(text, 0), &config());
            assert_eq!(sites.len(), 1, "expected one site in {text:?}, got {sites:?}");
            assert_eq!(sites[0].tag, expected, "{text:?}");
        }
    }

    /// The mid-comment reading is for the configured spelling only; a miscased tag word has to open
    /// its line or segment, because `hack` and `todo` are English.
    #[test]
    fn a_miscased_tag_after_a_label_is_still_prose() {
        assert!(tag_sites(&comment("# Note: todo later", 0), &config()).is_empty());
        assert!(tag_sites(&comment("# a quick hack: works", 0), &config()).is_empty());
    }

    #[test]
    fn a_tag_in_tag_position_is_a_tag_in_every_written_form() {
        for (text, expected) in [
            ("# TODO: x", "TODO"),
            ("# TODO(AMVP-1): x", "TODO"),
            ("# TODO AMVP-1 x", "TODO"),
            ("# FIXME - x", "FIXME"),
            ("# XXX: x", "XXX"),
            ("# HACK x", "HACK"),
            ("#TODO: x", "TODO"),
            ("// TODO x", "TODO"),
            ("/* TODO x */", "TODO"),
            ("/*TODO x*/", "TODO"),
            ("# todo: x", "todo"),
            ("# - TODO: a bulleted tag", "TODO"),
            ("-- TODO: a SQL or Lua tag", "TODO"),
            ("<!-- TODO: a markup tag -->", "TODO"),
            ("// — TODO: after a dash", "TODO"),
        ] {
            let sites = tag_sites(&comment(text, 0), &config());
            assert_eq!(sites.len(), 1, "expected one site in {text:?}, got {sites:?}");
            assert_eq!(sites[0].tag, expected, "{text:?}");
        }
    }

    /// `# noqa: T201  # TODO: fix T201` is one comment node to the parser, and the ruff idiom of
    /// hanging a tag off a directive this way is how hundreds of real TODOs are written.
    #[test]
    fn a_tag_opening_a_trailing_comment_segment_is_a_tag() {
        for (text, expected) in [
            ("# noqa: T201  # TODO: fix T201", "TODO"),
            ("# type: ignore[attr-defined]  # FIXME(AMVP-1): upstream stubs", "FIXME"),
            ("# pylint: disable=broad-except # todo: narrow it", "todo"),
            ("// eslint-disable-line no-console  // TODO remove", "TODO"),
            ("// eslint-disable-next-line no-console -- TODO(AMVP-1): remove", "TODO"),
        ] {
            let sites = tag_sites(&comment(text, 0), &config());
            assert_eq!(sites.len(), 1, "expected one site in {text:?}, got {sites:?}");
            assert_eq!(sites[0].tag, expected, "{text:?}");
        }
    }

    #[test]
    fn a_marker_that_does_not_follow_whitespace_opens_no_segment() {
        assert!(tag_sites(&comment("# see issue#TODO", 0), &config()).is_empty());
        assert!(tag_sites(&comment("# url: https://x//TODO", 0), &config()).is_empty());
    }

    #[test]
    fn fix_never_rewrites_a_tag_word_in_prose() {
        for text in [
            "# Line contains FIXME, consider resolving the issue",
            "# Missing colon in TODO",
            "# Invalid TODO tag: `FIXME`",
        ] {
            let findings = check(&comment(text, 0), &config(), None, Some("AMVP-9"));
            assert!(
                findings.is_empty(),
                "prose must yield no finding and no edit: {text:?} -> {findings:?}"
            );
        }
    }

    // --- documentation ---------------------------------------------------------------------------

    fn documented(text: &str, kind: CommentKind, node_type: &str) -> InspectedComment {
        InspectedComment {
            kind,
            node_type: node_type.to_string(),
            is_documentation: true,
            ..comment(text, 0)
        }
    }

    fn with_doc_comments() -> LintConfig {
        config_from("enabled = true\ninclude_doc_comments = true\n")
    }

    #[test]
    fn doc_comments_and_docstrings_are_not_inspected_by_default() {
        for doc in [
            documented("\"\"\"TODO: Implement\"\"\"", CommentKind::Docstring, "string"),
            documented(
                "\"\"\"\n    Summary.\n\n    TODO: Change file name after deprecation\n    \"\"\"",
                CommentKind::Docstring,
                "string",
            ),
            documented("/// TODO: document the panics", CommentKind::Doc, "line_comment"),
            documented("//! FIXME: crate docs", CommentKind::Doc, "line_comment"),
            documented(
                "/**\n * TODO: describe the return value\n */",
                CommentKind::Doc,
                "comment",
            ),
        ] {
            assert!(
                check(&doc, &config(), None, Some("AMVP-9")).is_empty(),
                "documentation is out of scope by default: {:?}",
                doc.text
            );
        }
    }

    #[test]
    fn include_doc_comments_puts_documentation_back_in_scope() {
        for (doc, expected) in [
            (
                documented("\"\"\"TODO: Implement\"\"\"", CommentKind::Docstring, "string"),
                "TODO",
            ),
            (
                documented("r'''FIXME: raw'''", CommentKind::Docstring, "string"),
                "FIXME",
            ),
            (
                documented(
                    "\"\"\"\n    Summary.\n\n    TODO: rename\n    \"\"\"",
                    CommentKind::Docstring,
                    "string",
                ),
                "TODO",
            ),
            (
                documented("/// TODO: document the panics", CommentKind::Doc, "line_comment"),
                "TODO",
            ),
            (
                documented("/**\n * HACK: works around it\n */", CommentKind::Doc, "comment"),
                "HACK",
            ),
        ] {
            let sites = tag_sites(&doc, &with_doc_comments());
            assert_eq!(sites.len(), 1, "expected one site in {:?}, got {sites:?}", doc.text);
            assert_eq!(sites[0].tag, expected);
        }
    }

    #[test]
    fn a_docstring_tag_still_needs_tag_position_when_doc_comments_are_included() {
        for text in [
            "\"\"\"{\"uuid\": XXX} will be returned (similar to the regular search).\"\"\"",
            "\"\"\"Returns the TODO list of a tenant.\"\"\"",
            "\"\"\"\n    Replaces the prefix, for example Firmware_XXX to firmware XXX.\n    \"\"\"",
        ] {
            let doc = documented(text, CommentKind::Docstring, "string");
            assert!(tag_sites(&doc, &with_doc_comments()).is_empty(), "{text:?}");
        }
    }

    /// A Go or Ruby handler calls any comment directly above a declaration documentation, by
    /// position alone. `// TODO: x` above a `func` is still written as a plain comment, and is the
    /// most common place a Go TODO sits, so only documentation *syntax* takes a comment out of scope.
    #[test]
    fn a_plain_comment_classified_as_documentation_only_by_position_is_still_inspected() {
        let above_a_func = documented("// TODO: split this function", CommentKind::Doc, "comment");
        let sites = tag_sites(&above_a_func, &config());
        assert_eq!(sites.len(), 1, "{sites:?}");
    }

    #[test]
    fn a_double_hash_comment_is_a_plain_comment_not_documentation() {
        for text in [
            "## TODO: command error handling",
            "## TODO add support for removing these values.",
        ] {
            let hash_rule = documented(text, CommentKind::Doc, "comment");
            let sites = tag_sites(&hash_rule, &config());
            assert_eq!(sites.len(), 1, "expected one site in {text:?}, got {sites:?}");
        }
    }

    #[test]
    fn an_untagged_comment_has_no_sites_at_all() {
        assert!(tag_sites(&comment("// just a note", 0), &config()).is_empty());
        assert!(tag_sites(&comment("// TODOS are tracked in Jira", 0), &config()).is_empty());
    }

    #[test]
    fn offsets_are_absolute_so_the_fix_lands_inside_the_comments_own_span() {
        let content = "fn main() {}\n    // FIXME: later\n";
        let comment_start = content.find("//").expect("comment present");
        let inspected = comment("// FIXME: later", comment_start);

        let findings = check(&inspected, &config(), None, None);
        let fix = findings
            .iter()
            .find(|finding| finding.rule == Rule::TagNotCanonical)
            .expect("tag-not-canonical is fixable");
        let edit = &fix.edits[0];

        assert_eq!(&content[edit.start..edit.end], "FIXME");
        assert!(edit.start >= inspected.start_byte && edit.end <= inspected.end_byte);
        assert_eq!(
            crate::edit::apply_edits(content, fix.edits.clone()).expect("applies"),
            "fn main() {}\n    // TODO: later\n"
        );
    }

    #[test]
    fn a_fix_preserves_the_existing_key_and_punctuation_exactly() {
        let content = "# FIXME(AMVP-1): x\n";
        let findings = check(&comment("# FIXME(AMVP-1): x", 0), &config(), None, None);
        let edits: Vec<Edit> = findings.into_iter().flat_map(|finding| finding.edits).collect();
        assert_eq!(
            crate::edit::apply_edits(content, edits).expect("applies"),
            "# TODO(AMVP-1): x\n"
        );
    }

    #[test]
    fn a_missing_key_is_unfixable_without_an_explicit_key() {
        let findings = check(&comment("// TODO: later", 0), &config(), None, None);
        assert!(!findings[0].is_fixable());
    }

    #[test]
    fn an_explicit_key_fixes_a_missing_one_and_supplies_the_colon_when_absent() {
        let cases = [
            ("// TODO: later", "// TODO(AMVP-9): later"),
            ("// TODO later", "// TODO(AMVP-9): later"),
            ("//TODO", "//TODO(AMVP-9):"),
            ("// TODO  :  later", "// TODO(AMVP-9)  :  later"),
        ];

        for (input, expected) in cases {
            let findings = check(&comment(input, 0), &config(), None, Some("AMVP-9"));
            let edits: Vec<Edit> = findings.into_iter().flat_map(|finding| finding.edits).collect();
            assert_eq!(
                crate::edit::apply_edits(input, edits).expect("applies"),
                expected,
                "fixing {input:?}"
            );
        }
    }

    #[test]
    fn a_non_canonical_tag_missing_a_key_is_fixed_by_both_edits_at_once() {
        let findings = check(&comment("// FIXME: later", 0), &config(), None, Some("AMVP-9"));
        let edits: Vec<Edit> = findings.into_iter().flat_map(|finding| finding.edits).collect();
        assert_eq!(
            crate::edit::apply_edits("// FIXME: later", edits).expect("applies"),
            "// TODO(AMVP-9): later"
        );
    }

    #[test]
    fn a_key_the_pattern_rejects_is_not_fixed_by_prepending_a_second_group() {
        let findings = check(&comment("// TODO(nope): later", 0), &config(), None, Some("AMVP-9"));
        assert_eq!(findings[0].rule, Rule::TodoMissingKey);
        assert!(
            !findings[0].is_fixable(),
            "rewriting would produce `TODO(AMVP-9)(nope):`"
        );
    }

    #[test]
    fn fixing_a_missing_key_is_idempotent() {
        let once = {
            let findings = check(&comment("// TODO later", 0), &config(), None, Some("AMVP-9"));
            let edits: Vec<Edit> = findings.into_iter().flat_map(|f| f.edits).collect();
            crate::edit::apply_edits("// TODO later", edits).expect("applies")
        };
        assert_eq!(once, "// TODO(AMVP-9): later");

        let twice = check(&comment(&once, 0), &config(), None, Some("AMVP-9"));
        assert!(twice.is_empty(), "{twice:?}");
    }

    #[test]
    fn an_excerpt_is_the_tags_own_line_trimmed_and_capped() {
        let text = "/*\n *   TODO: the second line\n */";
        let sites = tag_sites(&comment(text, 0), &config());
        assert_eq!(sites[0].excerpt, "*   TODO: the second line");

        let long = format!("// TODO: {}", "x".repeat(400));
        let sites = tag_sites(&comment(&long, 0), &config());
        assert_eq!(sites[0].excerpt.chars().count(), EXCERPT_LIMIT + 1);
        assert!(sites[0].excerpt.ends_with('…'));
    }

    #[test]
    fn a_multi_byte_comment_yields_char_boundary_offsets() {
        let text = "// — TODO: café later";
        let sites = tag_sites(&comment(text, 0), &config());
        assert_eq!(sites.len(), 1);
        assert!(text.is_char_boundary(sites[0].start));
        assert!(text.is_char_boundary(sites[0].end));
        assert_eq!(&text[sites[0].start..sites[0].end], "TODO");
    }
}
