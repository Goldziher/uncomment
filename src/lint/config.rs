//! The `[lint]` table: which tags mark tracked work, what a valid issue key looks like, and how
//! loudly each rule complains.
//!
//! Nothing here is hardcoded on purpose. The convention this was written for — `TODO(AMVP-12345):`,
//! with `FIXME`/`HACK`/`XXX` rewritten to `TODO` — is one convention among many, so every tag,
//! pattern and severity is configuration and `lint` does nothing at all until `enabled = true`.
//!
//! Both regexes are compiled and validated the moment the table is read, so an unparseable pattern
//! is a configuration error naming the key it came from rather than a panic partway through a run
//! that has already rewritten files.
//!
//! This loader duplicates the config discovery in [`crate::config`] — same file names, same
//! nearest-wins layering, same git-root ceiling — because `Config` carries
//! `#[serde(deny_unknown_fields)]` and has no `lint` field yet. Only the `[lint]` table is
//! deserialized here and every other key in the document is ignored, which is what keeps this
//! loader from re-implementing the rest of the schema.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde::Deserialize;

use crate::paths::{absolute_normalized, find_repo_root};

/// Searched in this order within one directory, matching [`crate::config`].
const CONFIG_FILE_NAMES: [&str; 2] = [".uncommentrc.toml", "uncomment.toml"];

pub const DEFAULT_TAGS: [&str; 4] = ["TODO", "FIXME", "HACK", "XXX"];
pub const DEFAULT_CANONICAL_TAG: &str = "TODO";

/// Matched against a comment's text from the tag onwards, so the leading `^\s*` absorbs nothing more
/// than the gap between the comment delimiter and the tag.
pub const DEFAULT_KEY_PATTERN: &str = r"^\s*(?:TODO|FIXME|HACK|XXX)\((?<key>[A-Z][A-Z0-9]+-\d+)\)\s*:";

/// Matched against the current branch name. Deliberately unanchored: Armis branches are
/// `naaman.hirschfeld.AMVP-160815.description`, so the key sits in the middle.
pub const DEFAULT_BRANCH_PATTERN: &str = r"([A-Z][A-Z0-9]+-\d+)";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Counts towards a non-zero exit status.
    Error,
    /// Reported, but the run still succeeds.
    Warn,
    /// Not evaluated at all.
    Off,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warn => "warn",
            Severity::Off => "off",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rule {
    /// A non-canonical tag (`FIXME`/`HACK`/`XXX`) that should read as the canonical one.
    TagNotCanonical,
    /// A tag comment carrying no issue key.
    TodoMissingKey,
    /// A tag comment naming the issue the current change is being made under.
    TodoSelfReference,
}

impl Rule {
    pub const ALL: [Rule; 3] = [Rule::TagNotCanonical, Rule::TodoMissingKey, Rule::TodoSelfReference];

    pub fn as_str(self) -> &'static str {
        match self {
            Rule::TagNotCanonical => "tag-not-canonical",
            Rule::TodoMissingKey => "todo-missing-key",
            Rule::TodoSelfReference => "todo-self-reference",
        }
    }

    fn index(self) -> usize {
        match self {
            Rule::TagNotCanonical => 0,
            Rule::TodoMissingKey => 1,
            Rule::TodoSelfReference => 2,
        }
    }
}

/// `[lint.rules]` as written. Absent means "use the default severity", which lets a table that only
/// silences one rule leave the others alone.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleTable {
    #[serde(rename = "tag-not-canonical")]
    pub tag_not_canonical: Option<Severity>,

    #[serde(rename = "todo-missing-key")]
    pub todo_missing_key: Option<Severity>,

    #[serde(rename = "todo-self-reference")]
    pub todo_self_reference: Option<Severity>,
}

/// The `[lint]` table exactly as written, before any defaulting or regex compilation.
///
/// `deny_unknown_fields` applies to this table alone: a typo inside `[lint]` is a clear error, while
/// every key outside it stays none of this loader's business.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LintTable {
    pub enabled: Option<bool>,
    pub tags: Option<Vec<String>>,
    pub canonical_tag: Option<String>,
    pub key_pattern: Option<String>,
    pub current_issue_from_branch: Option<String>,

    #[serde(default)]
    pub rules: RuleTable,
}

/// Only the `[lint]` table; every other section of the document is discarded unread.
#[derive(Debug, Default, Deserialize)]
struct Document {
    #[serde(default)]
    lint: Option<LintTable>,
}

/// A validated `[lint]` table with its regexes compiled.
#[derive(Debug)]
pub struct LintConfig {
    pub enabled: bool,
    pub tags: Vec<String>,
    pub canonical_tag: String,
    pub key_pattern: Regex,
    pub branch_pattern: Regex,
    /// Locates configured tags inside a comment's text; derived from `tags`, never configured
    /// directly.
    pub tag_pattern: Regex,
    severities: [Severity; 3],
    /// The config file this came from, for error messages. `None` for the built-in defaults.
    pub source: Option<PathBuf>,
}

impl LintConfig {
    /// Defaults with every rule at `error` — but `enabled = false`, so they do nothing.
    pub fn defaults() -> Result<Self> {
        Self::from_table(&LintTable::default(), None)
    }

    pub fn from_table(table: &LintTable, source: Option<PathBuf>) -> Result<Self> {
        let where_from = || match &source {
            Some(path) => format!(" in {}", path.display()),
            None => String::new(),
        };

        let tags: Vec<String> = table
            .tags
            .clone()
            .unwrap_or_else(|| DEFAULT_TAGS.iter().map(|tag| (*tag).to_string()).collect());
        if tags.is_empty() {
            bail!("lint.tags{} is empty: there is nothing to lint for", where_from());
        }
        if let Some(blank) = tags.iter().find(|tag| tag.trim().is_empty()) {
            bail!("lint.tags{} contains a blank tag {blank:?}", where_from());
        }

        let canonical_tag = table
            .canonical_tag
            .clone()
            .unwrap_or_else(|| DEFAULT_CANONICAL_TAG.to_string());
        // A canonical tag outside `tags` would make `--fix` non-idempotent: the rewrite would
        // produce a tag no later run recognises.
        if !tags.iter().any(|tag| tag == &canonical_tag) {
            bail!(
                "lint.canonical_tag{} is {canonical_tag:?}, which is not one of lint.tags {tags:?}",
                where_from()
            );
        }

        let key_pattern = compile(
            table.key_pattern.as_deref().unwrap_or(DEFAULT_KEY_PATTERN),
            "lint.key_pattern",
            &where_from(),
        )?;
        let branch_pattern = compile(
            table
                .current_issue_from_branch
                .as_deref()
                .unwrap_or(DEFAULT_BRANCH_PATTERN),
            "lint.current_issue_from_branch",
            &where_from(),
        )?;

        let mut alternation = String::from(r"\b(?:");
        // Longest first so `TODO` cannot claim the prefix of a longer configured tag.
        let mut ordered = tags.clone();
        ordered.sort_by(|a, b| b.len().cmp(&a.len()).then(a.cmp(b)));
        for (index, tag) in ordered.iter().enumerate() {
            if index > 0 {
                alternation.push('|');
            }
            alternation.push_str(&regex::escape(tag));
        }
        alternation.push_str(r")\b");
        let tag_pattern = compile(&alternation, "lint.tags", &where_from())?;

        let severities = [
            table.rules.tag_not_canonical.unwrap_or(Severity::Error),
            table.rules.todo_missing_key.unwrap_or(Severity::Error),
            table.rules.todo_self_reference.unwrap_or(Severity::Error),
        ];

        Ok(Self {
            enabled: table.enabled.unwrap_or(false),
            tags,
            canonical_tag,
            key_pattern,
            branch_pattern,
            tag_pattern,
            severities,
            source,
        })
    }

    pub fn severity(&self, rule: Rule) -> Severity {
        self.severities[rule.index()]
    }

    pub fn is_on(&self, rule: Rule) -> bool {
        self.severity(rule) != Severity::Off
    }

    /// The issue key in `haystack`, which is a comment's text from a tag onwards.
    ///
    /// Prefers the `key` capture group, falls back to the first group and then to the whole match,
    /// so a pattern written without a named group still works.
    pub fn extract_key<'t>(&self, haystack: &'t str) -> Option<&'t str> {
        let captures = self.key_pattern.captures(haystack)?;
        captures
            .name("key")
            .or_else(|| captures.get(1))
            .or_else(|| captures.get(0))
            .map(|m| m.as_str())
    }
}

fn compile(pattern: &str, key: &str, where_from: &str) -> Result<Regex> {
    Regex::new(pattern).with_context(|| format!("invalid regex for {key}{where_from}: {pattern}"))
}

/// Nearest-config-wins resolution of the `[lint]` table, memoized per directory.
///
/// Resolution is a sequential pre-pass over the directories holding the files to lint, so the
/// per-file lookup that follows is an immutable map read and a bad pattern is reported before
/// anything is inspected — let alone rewritten.
#[derive(Debug)]
pub struct Resolver {
    forced: Option<Arc<LintConfig>>,
    cache: HashMap<PathBuf, Arc<LintConfig>>,
    base: PathBuf,
}

impl Resolver {
    /// `forced` is `--config`: one file, used for every path, discovery skipped.
    pub fn new(base: &Path, forced: Option<&Path>) -> Result<Self> {
        let forced = match forced {
            Some(path) => Some(Arc::new(load_file(path)?.unwrap_or(LintConfig::defaults()?))),
            None => None,
        };

        Ok(Self {
            forced,
            cache: HashMap::new(),
            base: base.to_path_buf(),
        })
    }

    /// The table in force for `file`, which need not exist yet.
    pub fn for_file(&mut self, file: &Path) -> Result<Arc<LintConfig>> {
        if let Some(forced) = &self.forced {
            return Ok(Arc::clone(forced));
        }

        let absolute = absolute_normalized(&self.base, file);
        let dir = absolute.parent().unwrap_or(&absolute).to_path_buf();
        if let Some(cached) = self.cache.get(&dir) {
            return Ok(Arc::clone(cached));
        }

        let resolved = Arc::new(self.discover(&dir)?);
        self.cache.insert(dir, Arc::clone(&resolved));
        Ok(resolved)
    }

    /// Walk from `dir` up to the enclosing git root; the first `[lint]` table found wins outright.
    ///
    /// Tables are not merged across levels. A nested `[lint]` is a deliberate local policy, and
    /// half-inheriting one would make "which rules are on here" impossible to read off one file.
    fn discover(&self, dir: &Path) -> Result<LintConfig> {
        let ceiling = find_repo_root(dir).unwrap_or_else(|| dir.to_path_buf());

        let mut current = Some(dir);
        while let Some(candidate) = current {
            for name in CONFIG_FILE_NAMES {
                let path = candidate.join(name);
                if path.is_file()
                    && let Some(config) = load_file(&path)?
                {
                    return Ok(config);
                }
            }

            if candidate == ceiling {
                break;
            }
            current = candidate.parent();
        }

        LintConfig::defaults()
    }
}

/// The nearest config file at or above `dir`, bounded by the enclosing git root.
///
/// Whether it carries a `[lint]` table is not considered: this answers "which file configures
/// `uncomment` here", which is what the language configuration has to be read from.
pub fn nearest_config_file(dir: &Path) -> Option<PathBuf> {
    let ceiling = find_repo_root(dir).unwrap_or_else(|| dir.to_path_buf());

    let mut current = Some(dir);
    while let Some(candidate) = current {
        for name in CONFIG_FILE_NAMES {
            let path = candidate.join(name);
            if path.is_file() {
                return Some(path);
            }
        }

        if candidate == ceiling {
            break;
        }
        current = candidate.parent();
    }

    None
}

/// The removal command's [`crate::config::Config`] from `path`, with the `[lint]` table removed.
///
/// `Config` carries `deny_unknown_fields` and has no `lint` field, so a config file holding a
/// `[lint]` table fails to parse there — which would mean lint could never share a file with the
/// language configuration it needs in order to find comments at all. Stripping the table lets one
/// file serve both readers; the strip disappears once `Config` grows the field.
pub fn config_without_lint(path: &Path) -> Result<crate::config::Config> {
    let text = fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut document: toml::Table =
        toml::from_str(&text).with_context(|| format!("failed to parse config file: {}", path.display()))?;
    document.remove("lint");

    let config: crate::config::Config = document
        .try_into()
        .with_context(|| format!("failed to parse config file: {}", path.display()))?;
    config
        .validate()
        .with_context(|| format!("invalid configuration in: {}", path.display()))?;

    Ok(config)
}

/// `Ok(None)` when the file parses but carries no `[lint]` table, so discovery keeps walking up.
fn load_file(path: &Path) -> Result<Option<LintConfig>> {
    let text = fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let document: Document =
        toml::from_str(&text).with_context(|| format!("failed to parse [lint] from {}", path.display()))?;

    match document.lint {
        Some(table) => Ok(Some(LintConfig::from_table(&table, Some(path.to_path_buf()))?)),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn table(toml: &str) -> Result<LintConfig> {
        let document: Document = toml::from_str(toml)?;
        LintConfig::from_table(&document.lint.unwrap_or_default(), None)
    }

    #[test]
    fn lint_is_inert_until_it_is_enabled() {
        let defaults = LintConfig::defaults().unwrap();
        assert!(!defaults.enabled);
        assert!(table("[lint]\n").unwrap().enabled_is(false));
        assert!(table("[lint]\nenabled = true\n").unwrap().enabled);
        assert!(!table("[lint]\nenabled = false\n").unwrap().enabled);
    }

    #[test]
    fn enabled_alone_is_enough_to_get_the_documented_defaults() {
        let config = table("[lint]\nenabled = true\n").unwrap();
        assert_eq!(config.tags, DEFAULT_TAGS);
        assert_eq!(config.canonical_tag, "TODO");
        for rule in Rule::ALL {
            assert_eq!(config.severity(rule), Severity::Error, "{}", rule.as_str());
        }
    }

    #[test]
    fn the_default_key_pattern_uses_a_named_group_and_the_crates_regex_accepts_it() {
        let config = table("[lint]\nenabled = true\n").unwrap();
        assert!(
            config.key_pattern.capture_names().any(|name| name == Some("key")),
            "the `key` group is what makes the pattern self-documenting"
        );
        assert_eq!(config.extract_key("TODO(AMVP-160815): migrate"), Some("AMVP-160815"));
        assert_eq!(config.extract_key("FIXME(PPSC-42): later"), Some("PPSC-42"));
        assert_eq!(config.extract_key("TODO: no key here"), None);
        assert_eq!(config.extract_key("TODO(lowercase-1): no"), None);
    }

    #[test]
    fn a_pattern_without_a_named_group_falls_back_to_the_first_group() {
        let config = table(
            r#"
[lint]
enabled = true
key_pattern = '^\s*(?:TODO)\[([A-Z]+-\d+)\]'
"#,
        )
        .unwrap();
        assert_eq!(config.extract_key("TODO[ABC-7] do it"), Some("ABC-7"));
    }

    #[test]
    fn both_armis_branch_shapes_yield_the_same_key_with_the_default_pattern() {
        let config = table("[lint]\nenabled = true\n").unwrap();
        for branch in [
            "naaman.hirschfeld.AMVP-160815.ai-rulez-migration",
            "naaman.AMVP-160815.ai-rulez-migration",
        ] {
            let captures = config.branch_pattern.captures(branch);
            let key = captures.as_ref().and_then(|c| c.get(1)).map(|m| m.as_str());
            assert_eq!(key, Some("AMVP-160815"), "branch {branch}");
        }

        assert!(config.branch_pattern.captures("master").is_none());
        assert!(config.branch_pattern.captures("feat/comment-inventory").is_none());
    }

    #[test]
    fn an_invalid_key_pattern_is_a_load_error_naming_the_key() {
        let err = table("[lint]\nenabled = true\nkey_pattern = '([unclosed'\n").unwrap_err();
        let message = format!("{err:#}");
        assert!(message.contains("lint.key_pattern"), "{message}");
    }

    #[test]
    fn an_invalid_branch_pattern_is_a_load_error_naming_the_key() {
        let err = table("[lint]\nenabled = true\ncurrent_issue_from_branch = '(?P<'\n").unwrap_err();
        let message = format!("{err:#}");
        assert!(message.contains("lint.current_issue_from_branch"), "{message}");
    }

    #[test]
    fn a_canonical_tag_outside_tags_is_rejected() {
        let err = table("[lint]\nenabled = true\ntags = ['TODO']\ncanonical_tag = 'NOTE'\n").unwrap_err();
        let message = format!("{err:#}");
        assert!(message.contains("lint.canonical_tag"), "{message}");
    }

    #[test]
    fn empty_or_blank_tags_are_rejected() {
        assert!(format!("{:#}", table("[lint]\ntags = []\n").unwrap_err()).contains("lint.tags"));
        assert!(format!("{:#}", table("[lint]\ntags = ['TODO', '  ']\n").unwrap_err()).contains("lint.tags"));
    }

    #[test]
    fn a_typo_inside_the_lint_table_is_rejected() {
        let err = toml::from_str::<Document>("[lint]\nenable = true\n").unwrap_err();
        assert!(err.to_string().contains("enable"), "{err}");
    }

    #[test]
    fn keys_outside_the_lint_table_are_ignored_rather_than_rejected() {
        // The whole document is not this loader's schema; only `[lint]` is.
        let config = table(
            r#"
[global]
remove_todos = true

[languages.rust]
name = "rust"

[lint]
enabled = true
"#,
        )
        .unwrap();
        assert!(config.enabled);
    }

    #[test]
    fn per_rule_severities_override_only_what_they_name() {
        let config = table(
            r#"
[lint]
enabled = true

[lint.rules]
tag-not-canonical = "warn"
todo-self-reference = "off"
"#,
        )
        .unwrap();
        assert_eq!(config.severity(Rule::TagNotCanonical), Severity::Warn);
        assert_eq!(config.severity(Rule::TodoMissingKey), Severity::Error);
        assert_eq!(config.severity(Rule::TodoSelfReference), Severity::Off);
        assert!(!config.is_on(Rule::TodoSelfReference));
    }

    #[test]
    fn tags_are_matched_longest_first_and_on_word_boundaries() {
        let config = table("[lint]\nenabled = true\ntags = ['TODO', 'TODOLATER']\n").unwrap();
        let found: Vec<&str> = config
            .tag_pattern
            .find_iter("TODOLATER and TODO and TODOS")
            .map(|m| m.as_str())
            .collect();
        assert_eq!(found, vec!["TODOLATER", "TODO"]);
    }

    #[test]
    fn a_tag_with_regex_metacharacters_is_matched_literally() {
        let config = table("[lint]\nenabled = true\ntags = ['TODO', 'T.DO']\n").unwrap();
        assert!(config.tag_pattern.is_match("T.DO: x"));
        assert!(!config.tag_pattern.is_match("TXDO: x"));
    }

    #[test]
    fn the_nearest_config_with_a_lint_table_wins_outright() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join("nested/deeper")).unwrap();

        fs::write(
            root.join(".uncommentrc.toml"),
            "[lint]\nenabled = true\ntags = ['TODO']\n",
        )
        .unwrap();
        fs::write(
            root.join("nested/.uncommentrc.toml"),
            "[lint]\nenabled = true\ntags = ['NOTE']\ncanonical_tag = 'NOTE'\n",
        )
        .unwrap();

        let mut resolver = Resolver::new(root, None).unwrap();
        assert_eq!(resolver.for_file(&root.join("a.rs")).unwrap().tags, vec!["TODO"]);
        assert_eq!(resolver.for_file(&root.join("nested/b.rs")).unwrap().tags, vec!["NOTE"]);
        // Inherited from `nested/`, not merged with the root table.
        assert_eq!(
            resolver.for_file(&root.join("nested/deeper/c.rs")).unwrap().tags,
            vec!["NOTE"]
        );
    }

    #[test]
    fn a_config_without_a_lint_table_does_not_stop_the_upward_walk() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join("nested")).unwrap();

        fs::write(root.join(".uncommentrc.toml"), "[lint]\nenabled = true\n").unwrap();
        fs::write(root.join("nested/.uncommentrc.toml"), "[global]\nremove_todos = true\n").unwrap();

        let mut resolver = Resolver::new(root, None).unwrap();
        assert!(resolver.for_file(&root.join("nested/b.rs")).unwrap().enabled);
    }

    #[test]
    fn discovery_stops_at_the_git_root() {
        let temp = TempDir::new().unwrap();
        let outside = temp.path();
        let repo = outside.join("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();

        fs::write(outside.join(".uncommentrc.toml"), "[lint]\nenabled = true\n").unwrap();

        let mut resolver = Resolver::new(&repo, None).unwrap();
        assert!(
            !resolver.for_file(&repo.join("a.rs")).unwrap().enabled,
            "a config above the repository root must not apply"
        );
    }

    #[test]
    fn the_nearest_config_file_is_found_regardless_of_a_lint_table() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join(".git")).unwrap();
        let nested = root.join("services/api");
        fs::create_dir_all(&nested).unwrap();

        assert_eq!(nearest_config_file(&nested), None);

        fs::write(root.join("uncomment.toml"), "[global]\nremove_todos = false\n").unwrap();
        assert_eq!(nearest_config_file(&nested), Some(root.join("uncomment.toml")));

        fs::write(nested.join(".uncommentrc.toml"), "[lint]\nenabled = true\n").unwrap();
        assert_eq!(nearest_config_file(&nested), Some(nested.join(".uncommentrc.toml")));
    }

    #[test]
    fn stripping_the_lint_table_leaves_the_rest_of_the_document_loadable() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join(".uncommentrc.toml");
        fs::write(
            &path,
            "[global]\nremove_todos = true\n\n[lint]\nenabled = true\ntags = ['NOTE']\n",
        )
        .unwrap();

        // Without the strip this is `unknown field `lint``, and the language config is unreachable.
        let config = config_without_lint(&path).expect("loads with the lint table removed");
        assert!(config.global.remove_todos);
    }

    #[test]
    fn a_broken_document_still_reports_its_own_path() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join(".uncommentrc.toml");
        fs::write(&path, "[global]\nremove_todos = 'not a bool'\n").unwrap();

        let error = config_without_lint(&path).expect_err("an invalid value must not be swallowed");
        assert!(format!("{error:#}").contains(".uncommentrc.toml"), "{error:#}");
    }

    #[test]
    fn a_forced_config_file_skips_discovery_entirely() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join(".uncommentrc.toml"), "[lint]\nenabled = false\n").unwrap();

        let forced = root.join("forced.toml");
        fs::write(
            &forced,
            "[lint]\nenabled = true\ntags = ['NOTE']\ncanonical_tag = 'NOTE'\n",
        )
        .unwrap();

        let mut resolver = Resolver::new(root, Some(&forced)).unwrap();
        let config = resolver.for_file(&root.join("a.rs")).unwrap();
        assert!(config.enabled);
        assert_eq!(config.tags, vec!["NOTE"]);
    }

    #[test]
    fn an_invalid_pattern_in_a_discovered_file_names_that_file() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(
            root.join(".uncommentrc.toml"),
            "[lint]\nenabled = true\nkey_pattern = '([unclosed'\n",
        )
        .unwrap();

        let mut resolver = Resolver::new(root, None).unwrap();
        let err = resolver.for_file(&root.join("a.rs")).unwrap_err();
        let message = format!("{err:#}");
        assert!(message.contains("lint.key_pattern"), "{message}");
        assert!(message.contains(".uncommentrc.toml"), "{message}");
    }

    impl LintConfig {
        fn enabled_is(&self, expected: bool) -> bool {
            self.enabled == expected
        }
    }
}
