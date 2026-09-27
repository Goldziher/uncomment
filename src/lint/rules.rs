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
use crate::processor::InspectedComment;

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
    let text = &comment.text;
    let mut sites = Vec::new();
    let mut claimed_line: Option<usize> = None;

    for matched in config.tag_pattern.find_iter(text) {
        let line_index = text[..matched.start()].matches('\n').count();
        if claimed_line == Some(line_index) {
            continue;
        }
        if !is_tag_site(text, matched.as_str(), matched.start(), config) {
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

/// Whether a tag match is a tag at all, rather than the same word used as English.
///
/// A tag spelled exactly as configured is always one: `TODO` and `FIXME` in capitals are not words
/// anybody writes by accident, so a mid-sentence "see also FIXME" is a deliberate reference and has
/// always been reported as such.
///
/// A miscased one has to open its line. `todo`, `hack` and `xxx` *are* English, and matching them
/// anywhere would turn "this is a hack to work around the upstream bug" into a violation whose fix
/// rewrites the sentence. Measured on an 89k-file monorepo: 165 of the 183 miscased occurrences in
/// code files were prose, so without this the case-insensitive pass would do an order of magnitude
/// more damage than work.
fn is_tag_site(text: &str, written: &str, offset: usize, config: &LintConfig) -> bool {
    if config.tags.iter().any(|tag| tag == written) {
        return true;
    }

    // Everything between the line's start and the tag must be delimiter or decoration — `//`, `#`,
    // `/*`, `*`, `"""`, a `-` bullet. Testing for the absence of a word character covers every
    // comment syntax at once, where an allowlist of punctuation would have to be kept in step with
    // the language list.
    let line_start = text[..offset].rfind('\n').map_or(0, |index| index + 1);
    !text[line_start..offset]
        .chars()
        .any(|c| c.is_alphanumeric() || c == '_')
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
    use crate::processor::{CommentKind, Verdict};

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

    /// The canonical spelling is not an English word, so a deliberate mid-sentence reference to it
    /// stays a tag. This is the pre-existing behaviour for every one of the 3,148 uppercase tag
    /// occurrences in that monorepo, and narrowing it would be a regression.
    #[test]
    fn a_canonically_spelled_tag_is_a_tag_anywhere_in_the_comment() {
        let sites = tag_sites(&comment("// see also FIXME: the driver bug", 0), &config());
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].tag, "FIXME");
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
        let text = "// café — TODO: later";
        let sites = tag_sites(&comment(text, 0), &config());
        assert_eq!(sites.len(), 1);
        assert!(text.is_char_boundary(sites[0].start));
        assert!(text.is_char_boundary(sites[0].end));
        assert_eq!(&text[sites[0].start..sites[0].end], "TODO");
    }
}
