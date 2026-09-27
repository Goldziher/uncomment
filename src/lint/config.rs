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
//! [`LintTable`] is the `lint` field of [`crate::config::Config`], so one config file serves both
//! readers and discovery happens once, in [`crate::config::ConfigManager`]. What is left here is the
//! table's own schema, its layering rule, and turning a resolved table into a validated
//! [`LintConfig`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::config::ConfigManager;
use crate::paths::absolute_normalized;

pub const DEFAULT_TAGS: [&str; 4] = ["TODO", "FIXME", "HACK", "XXX"];
pub const DEFAULT_CANONICAL_TAG: &str = "TODO";

/// Matched against a comment's text from the tag onwards, so the leading `^\s*` absorbs nothing more
/// than the gap between the comment delimiter and the tag.
pub const DEFAULT_KEY_PATTERN: &str = r"^\s*(?:TODO|FIXME|HACK|XXX)\((?<key>[A-Z][A-Z0-9]+-\d+)\)\s*:";

/// Matched against the current branch name. Deliberately unanchored: Armis branches are
/// `naaman.hirschfeld.AMVP-160815.description`, so the key sits in the middle.
pub const DEFAULT_BRANCH_PATTERN: &str = r"([A-Z][A-Z0-9]+-\d+)";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
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
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuleTable {
    #[serde(rename = "tag-not-canonical", skip_serializing_if = "Option::is_none")]
    pub tag_not_canonical: Option<Severity>,

    #[serde(rename = "todo-missing-key", skip_serializing_if = "Option::is_none")]
    pub todo_missing_key: Option<Severity>,

    #[serde(rename = "todo-self-reference", skip_serializing_if = "Option::is_none")]
    pub todo_self_reference: Option<Severity>,
}

impl RuleTable {
    /// `self` layered on top of `base`: a severity `self` names wins, one it omits is inherited.
    fn layer_over(&self, base: &RuleTable) -> RuleTable {
        RuleTable {
            tag_not_canonical: self.tag_not_canonical.or(base.tag_not_canonical),
            todo_missing_key: self.todo_missing_key.or(base.todo_missing_key),
            todo_self_reference: self.todo_self_reference.or(base.todo_self_reference),
        }
    }
}

/// The `[lint]` table exactly as written, before any defaulting or regex compilation.
///
/// Every key is optional so that layering can tell "absent" from "set to the default", which is what
/// lets a nested `[lint]` override one key and inherit the rest.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LintTable {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical_tag: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_pattern: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_issue_from_branch: Option<String>,

    #[serde(default)]
    pub rules: RuleTable,
}

impl LintTable {
    /// `self` layered on top of `base`: every key `self` names wins, every key it omits — including
    /// each severity inside `[lint.rules]` — is inherited.
    ///
    /// Merging rather than replacing is what makes a nested `[lint]` a local amendment instead of a
    /// second, unrelated policy: a directory that only silences one rule keeps `enabled` and the tag
    /// vocabulary of the config above it.
    pub fn layer_over(&self, base: &LintTable) -> LintTable {
        LintTable {
            enabled: self.enabled.or(base.enabled),
            tags: self.tags.clone().or_else(|| base.tags.clone()),
            canonical_tag: self.canonical_tag.clone().or_else(|| base.canonical_tag.clone()),
            key_pattern: self.key_pattern.clone().or_else(|| base.key_pattern.clone()),
            current_issue_from_branch: self
                .current_issue_from_branch
                .clone()
                .or_else(|| base.current_issue_from_branch.clone()),
            rules: self.rules.layer_over(&base.rules),
        }
    }

    /// Reject what is wrong with this table on its own, before it is layered with any other.
    ///
    /// Only the keys this table actually names are judged, so [`crate::config::Config::validate`] can
    /// call it per file and report the file that carries the mistake. Cross-key rules that depend on
    /// the layered result — a canonical tag outside `tags` — are checked in
    /// [`LintConfig::from_table`] instead.
    pub fn validate(&self) -> Result<()> {
        if let Some(tags) = &self.tags {
            if tags.is_empty() {
                bail!("lint.tags is empty: there is nothing to lint for");
            }
            if let Some(blank) = tags.iter().find(|tag| tag.trim().is_empty()) {
                bail!("lint.tags contains a blank tag {blank:?}");
            }
        }
        if let Some(pattern) = &self.key_pattern {
            compile(pattern, "lint.key_pattern", "")?;
        }
        if let Some(pattern) = &self.current_issue_from_branch {
            compile(pattern, "lint.current_issue_from_branch", "")?;
        }

        Ok(())
    }
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

/// Per-directory resolution of the `[lint]` table, memoized.
///
/// Discovery and layering belong to [`ConfigManager`], which reads each config file once for every
/// reader; this adds only the compiled, validated form and the memo. Resolution runs as a sequential
/// pre-pass over the files to lint, so the per-file lookup that follows is a map read and a bad
/// pattern is reported before anything is inspected — let alone rewritten.
#[derive(Debug)]
pub struct Resolver<'manager> {
    manager: &'manager ConfigManager,
    /// Relative paths resolve against this, so the cache key is canonical.
    base: PathBuf,
    cache: HashMap<PathBuf, Arc<LintConfig>>,
}

impl<'manager> Resolver<'manager> {
    pub fn new(base: &Path, manager: &'manager ConfigManager) -> Self {
        Self {
            manager,
            base: base.to_path_buf(),
            cache: HashMap::new(),
        }
    }

    /// The table in force for `file`, which need not exist yet.
    pub fn for_file(&mut self, file: &Path) -> Result<Arc<LintConfig>> {
        let absolute = absolute_normalized(&self.base, file);
        let dir = absolute.parent().unwrap_or(&absolute).to_path_buf();
        if let Some(cached) = self.cache.get(&dir) {
            return Ok(Arc::clone(cached));
        }

        let (table, source) = self.manager.lint_table_for_file(&absolute);
        let resolved = Arc::new(LintConfig::from_table(&table.unwrap_or_default(), source)?);
        self.cache.insert(dir, Arc::clone(&resolved));
        Ok(resolved)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    use crate::config::Config;

    /// The `[lint]` table of a whole config document, compiled.
    fn table(toml: &str) -> Result<LintConfig> {
        let config: Config = toml::from_str(toml)?;
        LintConfig::from_table(&config.lint.unwrap_or_default(), None)
    }

    /// The table in force for `file` under `root`, resolved the way a lint run resolves it.
    fn resolve(root: &Path, file: &str) -> Result<Arc<LintConfig>> {
        let manager = ConfigManager::new(root)?;
        Resolver::new(root, &manager).for_file(&root.join(file))
    }

    fn repo() -> TempDir {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();
        temp
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
        let err = toml::from_str::<Config>("[lint]\nenable = true\n").unwrap_err();
        assert!(err.to_string().contains("enable"), "{err}");
    }

    #[test]
    fn one_document_carries_the_removal_settings_and_the_lint_table() {
        // `Config` has `deny_unknown_fields`, so before `lint` became a field this document was
        // `unknown field `lint`` for every reader but this one.
        let config: Config = toml::from_str(
            r#"
[global]
remove_todos = true

[lint]
enabled = true
"#,
        )
        .unwrap();
        assert!(config.global.remove_todos);
        assert!(LintConfig::from_table(&config.lint.unwrap(), None).unwrap().enabled);
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
    fn a_nested_table_overrides_only_the_keys_it_names() {
        let temp = repo();
        let root = temp.path();
        fs::create_dir_all(root.join("nested/deeper")).unwrap();

        fs::write(
            root.join(".uncommentrc.toml"),
            "[lint]\nenabled = true\ntags = ['TODO']\n\n[lint.rules]\ntag-not-canonical = 'warn'\n",
        )
        .unwrap();
        fs::write(
            root.join("nested/.uncommentrc.toml"),
            "[lint]\ntags = ['NOTE']\ncanonical_tag = 'NOTE'\n",
        )
        .unwrap();

        let above = resolve(root, "a.rs").unwrap();
        assert_eq!(above.tags, vec!["TODO"]);
        assert_eq!(above.canonical_tag, "TODO");

        let below = resolve(root, "nested/b.rs").unwrap();
        assert_eq!(below.tags, vec!["NOTE"]);
        // Named by neither the nested table nor a default: inherited from the root.
        assert!(below.enabled, "`enabled` must come from the config above");
        assert_eq!(below.severity(Rule::TagNotCanonical), Severity::Warn);
        assert_eq!(below.severity(Rule::TodoMissingKey), Severity::Error);

        // And the nested table keeps applying further down, where no config of its own exists.
        assert_eq!(resolve(root, "nested/deeper/c.rs").unwrap().tags, vec!["NOTE"]);
    }

    #[test]
    fn a_config_without_a_lint_table_leaves_the_one_above_it_in_force() {
        let temp = repo();
        let root = temp.path();
        fs::create_dir_all(root.join("nested")).unwrap();

        fs::write(root.join(".uncommentrc.toml"), "[lint]\nenabled = true\n").unwrap();
        fs::write(root.join("nested/.uncommentrc.toml"), "[global]\nremove_todos = true\n").unwrap();

        assert!(resolve(root, "nested/b.rs").unwrap().enabled);
    }

    #[test]
    fn discovery_stops_at_the_git_root() {
        let temp = TempDir::new().unwrap();
        let outside = temp.path();
        let repo = outside.join("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();

        fs::write(outside.join(".uncommentrc.toml"), "[lint]\nenabled = true\n").unwrap();

        assert!(
            !resolve(&repo, "a.rs").unwrap().enabled,
            "a config above the repository root must not apply"
        );
    }

    #[test]
    fn a_forced_config_file_skips_discovery_entirely() {
        let temp = repo();
        let root = temp.path();
        fs::create_dir_all(root.join("nested")).unwrap();
        fs::write(root.join(".uncommentrc.toml"), "[lint]\nenabled = false\n").unwrap();
        fs::write(root.join("nested/.uncommentrc.toml"), "[lint]\ntags = ['XXX']\n").unwrap();

        let forced = root.join("forced.toml");
        fs::write(
            &forced,
            "[lint]\nenabled = true\ntags = ['NOTE']\ncanonical_tag = 'NOTE'\n",
        )
        .unwrap();

        let manager = ConfigManager::from_config_file(root, &forced).unwrap();
        let mut resolver = Resolver::new(root, &manager);
        for file in ["a.rs", "nested/b.rs"] {
            let config = resolver.for_file(&root.join(file)).unwrap();
            assert!(config.enabled, "{file}");
            assert_eq!(config.tags, vec!["NOTE"], "{file}");
        }
    }

    #[test]
    fn an_invalid_pattern_in_a_discovered_file_names_that_file() {
        let temp = repo();
        let root = temp.path();
        fs::write(
            root.join(".uncommentrc.toml"),
            "[lint]\nenabled = true\nkey_pattern = '([unclosed'\n",
        )
        .unwrap();

        // A config on the invocation directory's own chain is loaded eagerly, so this is refused
        // before any resolution happens at all.
        let error = ConfigManager::new(root).unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains("lint.key_pattern"), "{message}");
        assert!(message.contains(".uncommentrc.toml"), "{message}");
    }

    #[test]
    fn an_invalid_pattern_below_the_invocation_directory_names_that_file() {
        let temp = repo();
        let root = temp.path();
        fs::create_dir_all(root.join("nested")).unwrap();
        fs::write(root.join(".uncommentrc.toml"), "[lint]\nenabled = true\n").unwrap();
        fs::write(
            root.join("nested/.uncommentrc.toml"),
            "[lint]\nkey_pattern = '([unclosed'\n",
        )
        .unwrap();

        // Below the invocation directory a config is discovered from a call that cannot fail, so the
        // failure is recorded rather than returned — and the run has to check for it.
        let manager = ConfigManager::new(root).unwrap();
        Resolver::new(root, &manager)
            .for_file(&root.join("nested/b.rs"))
            .unwrap();
        let recorded = manager.deferred_config_error().expect("the broken config was rejected");
        assert!(recorded.contains("lint.key_pattern"), "{recorded}");
        assert!(recorded.contains("nested"), "{recorded}");
    }

    #[test]
    fn a_table_is_validated_on_its_own_before_it_is_layered() {
        let broken: LintTable = toml::from_str("key_pattern = '([unclosed'\n").unwrap();
        let message = format!("{:#}", broken.validate().unwrap_err());
        assert!(message.contains("lint.key_pattern"), "{message}");

        // Only the keys the table names are judged: a table that sets `tags` alone must not be
        // rejected for the canonical tag it inherits.
        let partial: LintTable = toml::from_str("tags = ['NOTE']\n").unwrap();
        assert!(partial.validate().is_ok());
        assert!(toml::from_str::<LintTable>("tags = []\n").unwrap().validate().is_err());
    }

    impl LintConfig {
        fn enabled_is(&self, expected: bool) -> bool {
            self.enabled == expected
        }
    }
}
