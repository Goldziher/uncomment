//! Config discovery and per-file resolution: which files apply to a path, in what order, and the
//! caches that keep asking cheap.
//!
//! The file shapes being layered here are `super::file`.

use super::file::LoadedConfig;
use super::{
    CONFIG_FILE_NAME, CONFIG_FILE_NAMES, Config, ExcludeSet, LEGACY_NAME_NOTICE, LanguageConfig, ResolvedConfig,
    exclude,
};
use crate::lint::config::LintTable;
use crate::paths;
use ahash::{AHashMap, AHashSet};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Once, RwLock};

#[derive(Debug)]
pub struct ConfigManager {
    /// The user-level config, merged beneath every project config.
    global_config: Option<Arc<LoadedConfig>>,

    /// The root directory and its ancestors up to the git root, outermost first.
    ancestor_configs: Vec<Arc<LoadedConfig>>,

    /// Configs below the root that may also declare custom languages, deepest first.
    /// Empty until [`ConfigManager::discover_language_sources`] is called, because
    /// finding them costs a walk and nothing else about resolution needs one.
    descendant_language_configs: Vec<Arc<LoadedConfig>>,

    /// An explicit `--config` file, which replaces directory discovery entirely.
    forced_config: Option<Arc<LoadedConfig>>,

    /// Memoised "is there a config file in this directory?", filled on first use.
    dir_configs: RwLock<AHashMap<PathBuf, Option<Arc<LoadedConfig>>>>,

    /// Memoised per-file resolution results.
    file_configs: RwLock<AHashMap<PathBuf, ResolvedConfig>>,

    /// The highest directory the upward search may reach.
    ceiling: PathBuf,

    /// The directory the requested paths are resolved against, and the floor of the
    /// eagerly loaded ancestor chain.
    root_dir: PathBuf,

    current_dir: PathBuf,

    lazy_language_warning: Once,

    /// First failure from a lazily discovered config, readable through
    /// `deferred_config_error`.
    deferred_error: RwLock<Option<String>>,
}

impl ConfigManager {
    pub fn new<P: AsRef<Path>>(root_dir: P) -> Result<Self> {
        let current_dir = std::env::current_dir().context("Failed to get current directory")?;
        let root_dir = paths::absolute_normalized(&current_dir, root_dir.as_ref());
        // Bounding the upward search at the git root keeps configuration outside the
        // repository from applying.
        let ceiling = paths::find_repo_root(&root_dir).unwrap_or_else(|| root_dir.clone());

        // Only the ancestor chain can ever apply to `root_dir` itself, and it is
        // bounded by path depth rather than by tree size. Everything below the root
        // is discovered lazily, on the first file that needs it.
        let mut ancestor_configs = Vec::new();
        let mut dir_configs = AHashMap::new();
        let mut dir = Some(root_dir.as_path());
        while let Some(current) = dir {
            let loaded = Self::load_dir_config(current)?;
            if let Some(loaded) = &loaded {
                ancestor_configs.push(loaded.clone());
            }
            dir_configs.insert(current.to_path_buf(), loaded);

            if current == ceiling {
                break;
            }
            dir = current.parent();
        }
        ancestor_configs.reverse();

        Ok(Self {
            global_config: Self::load_global_config(),
            ancestor_configs,
            descendant_language_configs: Vec::new(),
            forced_config: None,
            dir_configs: RwLock::new(dir_configs),
            file_configs: RwLock::new(AHashMap::new()),
            ceiling,
            root_dir,
            current_dir,
            lazy_language_warning: Once::new(),
            deferred_error: RwLock::new(None),
        })
    }

    pub fn from_single_config<P: AsRef<Path>>(root_dir: P, config: Config) -> Result<Self> {
        Self::forced(root_dir, None, config)
    }

    /// One explicit config file — `--config` — read here so that errors naming it stay available to
    /// every reader of the resulting manager, `[lint]` included.
    pub fn from_config_file<P: AsRef<Path>, Q: AsRef<Path>>(root_dir: P, path: Q) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let config = Config::from_file(&path)?;
        Self::forced(root_dir, Some(path), config)
    }

    fn forced<P: AsRef<Path>>(root_dir: P, path: Option<PathBuf>, config: Config) -> Result<Self> {
        let current_dir = std::env::current_dir().context("Failed to get current directory")?;
        let root_dir = paths::absolute_normalized(&current_dir, root_dir.as_ref());
        let loaded = Arc::new(LoadedConfig::new(root_dir.clone(), path, config)?);

        Ok(Self {
            global_config: None,
            ancestor_configs: vec![loaded.clone()],
            descendant_language_configs: Vec::new(),
            forced_config: Some(loaded),
            dir_configs: RwLock::new(AHashMap::new()),
            file_configs: RwLock::new(AHashMap::new()),
            ceiling: root_dir.clone(),
            root_dir,
            current_dir,
            lazy_language_warning: Once::new(),
            deferred_error: RwLock::new(None),
        })
    }

    fn global_config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join("uncomment").join("config.toml"))
    }

    fn load_global_config() -> Option<Arc<LoadedConfig>> {
        let path = Self::global_config_path()?;
        if !path.exists() {
            return None;
        }

        let dir = path.parent()?.to_path_buf();
        match Config::from_file(&path).and_then(|config| LoadedConfig::new(dir, Some(path.clone()), config)) {
            Ok(loaded) => Some(Arc::new(loaded)),
            Err(e) => {
                eprintln!("Warning: Failed to load global config: {e}");
                None
            }
        }
    }

    /// The config governing `dir`, or `None` when the directory has none.
    ///
    /// A config file that exists but cannot be loaded is an error. uncomment rewrites
    /// files in place, so a config it cannot understand must stop the run instead of
    /// degrading to built-in defaults — which would empty `preserve_patterns`.
    ///
    /// A directory holding more than one accepted name uses the highest-precedence one outright;
    /// the others contribute nothing. A deprecated name still wins over a lower-precedence one,
    /// and warns.
    fn load_dir_config(dir: &Path) -> Result<Option<Arc<LoadedConfig>>> {
        // The first name that *exists* wins whether or not it parses; otherwise a typo in
        // `.uncomment.toml` would silently promote a deprecated name to authoritative.
        let Some(path) = CONFIG_FILE_NAMES
            .iter()
            .map(|file_name| dir.join(file_name))
            .find(|path| path.is_file())
        else {
            return Ok(None);
        };

        Self::warn_on_legacy_config_name(&path);

        let config = Config::from_file(&path)?;
        let loaded = LoadedConfig::new(dir.to_path_buf(), Some(path.clone()), config)
            .with_context(|| format!("Invalid configuration in: {}", path.display()))?;

        Ok(Some(Arc::new(loaded)))
    }

    /// Tell the user once that a discovered config uses a deprecated name. Nothing is renamed and
    /// the run is unaffected — the old names keep working. `--config` is not covered: that path is
    /// named explicitly and may be called anything.
    fn warn_on_legacy_config_name(path: &Path) {
        let is_legacy = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name != CONFIG_FILE_NAME);
        if !is_legacy {
            return;
        }

        let path = path.display().to_string();
        LEGACY_NAME_NOTICE.call_once(|| {
            eprintln!(
                "Warning: {path} uses a deprecated configuration file name; rename it to {CONFIG_FILE_NAME}. \
                 The old names are still read, for now."
            );
        });
    }

    /// The first failure from a lazily discovered config, once resolution has run.
    ///
    /// `get_config_for_file` is called per file from the parallel processing pass and
    /// cannot return an error, so a caller that rewrites files must check this before
    /// reporting success. Configs on the ancestor chain fail `ConfigManager::new`
    /// outright and never land here.
    pub fn deferred_config_error(&self) -> Option<String> {
        self.deferred_error.read().ok().and_then(|slot| slot.clone())
    }

    fn record_deferred_error(&self, error: anyhow::Error) {
        let message = format!("{error:#}");
        if let Ok(mut slot) = self.deferred_error.write()
            && slot.is_none()
        {
            eprintln!("error: {message}");
            *slot = Some(message);
        }
    }

    fn dir_config(&self, dir: &Path) -> Option<Arc<LoadedConfig>> {
        if let Ok(cache) = self.dir_configs.read()
            && let Some(entry) = cache.get(dir)
        {
            return entry.clone();
        }

        let loaded = match Self::load_dir_config(dir) {
            Ok(loaded) => loaded,
            Err(error) => {
                self.record_deferred_error(error);
                None
            }
        };

        if let Some(loaded) = &loaded
            && !loaded.config.languages.is_empty()
            && !self.is_language_source(&loaded.dir)
        {
            let path = loaded.dir.display().to_string();
            self.lazy_language_warning.call_once(|| {
                eprintln!(
                    "Warning: [languages] in the config under {path} is ignored; the language registry was \
                     already built when that config was reached."
                );
            });
        }

        if let Ok(mut cache) = self.dir_configs.write() {
            cache.insert(dir.to_path_buf(), loaded.clone());
        }

        loaded
    }

    /// Whether `dir`'s config already contributed its `[languages]` to the registry.
    fn is_language_source(&self, dir: &Path) -> bool {
        self.language_sources().any(|loaded| loaded.dir == dir)
    }

    /// The config files that apply to `dir`, outermost first. `dir` must already be
    /// normalized: the walk below is lexical and a stray `..` would climb past the
    /// ceiling.
    fn config_chain(&self, dir: &Path) -> Vec<Arc<LoadedConfig>> {
        if let Some(forced) = &self.forced_config {
            return vec![forced.clone()];
        }

        let mut chain = Vec::new();
        let mut current = Some(dir);
        while let Some(candidate) = current {
            if !paths::is_ancestor_of(&self.ceiling, candidate) {
                break;
            }
            if let Some(loaded) = self.dir_config(candidate) {
                chain.push(loaded);
            }
            current = candidate.parent();
        }
        chain.reverse();

        chain
    }

    /// Resolve the effective configuration for one file.
    ///
    /// Layers are applied outermost first: the user-level config, then one config file per
    /// directory — see [`CONFIG_FILE_NAMES`] — from the search ceiling down to the file's own
    /// directory.
    ///
    /// `[patterns."<glob>"]` sections are applied last. For a **discovered** config each
    /// glob is matched against the file path relative to the directory holding that
    /// config file, so a nested config's globs are anchored at that nested directory
    /// rather than at the invocation root. A config forced with `--config` is anchored at
    /// the **invocation directory** instead of at wherever the file itself lives, so
    /// `--config ../shared/rules.toml` with a `src/**` glob means `src/**` under the
    /// current directory.
    ///
    /// Within one config file the globs are applied in a fixed order — fewer path
    /// components first, ties broken by the glob text — and the last match wins, which
    /// keeps the result independent of `HashMap` iteration order.
    fn resolve_config_for_file(&self, file_path: &Path) -> ResolvedConfig {
        let dir = file_path.parent().unwrap_or(file_path);
        let chain = self.config_chain(dir);

        let mut base_config = Config::default();
        if let Some(global) = &self.global_config {
            base_config = base_config.merge_with(&global.config);
        }
        for loaded in &chain {
            base_config = base_config.merge_with(&loaded.config);
        }

        let mut resolved = ResolvedConfig {
            remove_todos: base_config.global.remove_todos,
            remove_fixme: base_config.global.remove_fixme,
            remove_docs: base_config.global.remove_docs,
            preserve_patterns: base_config.global.preserve_patterns,
            use_default_ignores: base_config.global.use_default_ignores,
            respect_gitignore: base_config.global.respect_gitignore,
            traverse_git_repos: base_config.global.traverse_git_repos,
            language_config: None,
        };

        for loaded in &chain {
            loaded.apply_patterns(file_path, &mut resolved);
        }

        resolved
    }

    pub fn get_config_for_file<P: AsRef<Path>>(&self, file_path: P) -> ResolvedConfig {
        // Normalizing up front makes the cache key canonical and, more importantly, keeps
        // a `..` in the path from reaching a directory the walk must not see.
        let file_path = paths::absolute_normalized(&self.current_dir, file_path.as_ref());

        if let Ok(cache) = self.file_configs.read()
            && let Some(cached) = cache.get(&file_path)
        {
            return cached.clone();
        }

        let resolved = self.resolve_config_for_file(&file_path);
        if let Ok(mut cache) = self.file_configs.write() {
            cache.insert(file_path, resolved.clone());
        }

        resolved
    }

    /// The `[lint]` table in force for one file, layered exactly like the rest of the configuration:
    /// the user-level config first, then every config file from the search ceiling down to the file's
    /// own directory, each one amending the last key by key.
    ///
    /// The second element is the innermost config file that contributed a `[lint]` section, which is
    /// what a table-level mistake is reported against. `None` there means no file did — the built-in
    /// defaults — or that the config came from code rather than from a file.
    ///
    /// Infallible, like [`Self::get_config_for_file`]: a config file below the invocation directory is
    /// discovered here, and a rejected one is recorded for [`Self::deferred_config_error`] rather than
    /// returned. A caller that may rewrite files has to check that before acting on the result.
    pub fn lint_table_for_file<P: AsRef<Path>>(&self, file_path: P) -> (Option<LintTable>, Option<PathBuf>) {
        let file_path = paths::absolute_normalized(&self.current_dir, file_path.as_ref());
        let dir = file_path.parent().unwrap_or(&file_path);

        let mut table: Option<LintTable> = None;
        let mut source: Option<PathBuf> = None;
        for loaded in self.global_config.iter().chain(self.config_chain(dir).iter()) {
            let Some(next) = &loaded.config.lint else {
                continue;
            };
            table = Some(match &table {
                Some(base) => next.layer_over(base),
                None => next.clone(),
            });
            if let Some(path) = &loaded.path {
                source = Some(path.clone());
            }
        }

        (table, source)
    }

    pub fn get_config_for_file_with_language<P: AsRef<Path>>(
        &self,
        file_path: P,
        language_name: &str,
    ) -> ResolvedConfig {
        let mut config = self.get_config_for_file(file_path);

        if let Some(lang_config) = self.get_language_config(language_name) {
            if let Some(remove_todos) = lang_config.remove_todos {
                config.remove_todos = remove_todos;
            }
            if let Some(remove_fixme) = lang_config.remove_fixme {
                config.remove_fixme = remove_fixme;
            }
            if let Some(remove_docs) = lang_config.remove_docs {
                config.remove_docs = remove_docs;
            }
            if let Some(use_default_ignores) = lang_config.use_default_ignores {
                config.use_default_ignores = use_default_ignores;
            }

            config
                .preserve_patterns
                .extend(lang_config.preserve_patterns.iter().cloned());
            config.preserve_patterns.sort();
            config.preserve_patterns.dedup();

            config.language_config = Some(lang_config);
        }

        config
    }

    /// The path exclusions in force for this run: every `[global] exclude` glob from the configs
    /// already loaded, plus `extra` — the `--exclude` flags — anchored at the invocation directory.
    ///
    /// Only a config loaded before collection can contribute: the user-level config, the ancestor
    /// chain, a `--config` file, and whatever [`Self::discover_language_sources`] reached. A config
    /// first seen during per-file resolution is too late by construction — the file it would have
    /// excluded has already been collected — so, as with `[languages]`, the sweep is what makes a
    /// config below the invocation directory count.
    pub fn exclude_set(&self, extra: &[String]) -> Result<ExcludeSet> {
        let mut excludes = ExcludeSet::new(&self.current_dir);
        for loaded in self
            .global_config
            .iter()
            .chain(&self.ancestor_configs)
            .chain(&self.descendant_language_configs)
        {
            excludes.add(&loaded.dir, &loaded.config.global.exclude, exclude::CONFIG_KEY)?;
        }
        excludes.add(&self.current_dir, extra, "--exclude")?;

        Ok(excludes)
    }

    /// Extend the set of configs that may declare custom languages to cover `paths`.
    ///
    /// A custom language has to be known *before* file collection. Collection keeps only the files
    /// whose extension some registry recognises, so a file named solely by a `[languages]` section
    /// is discarded before anything reads that section, and the declaration does nothing at all.
    /// The ancestor chain is loaded eagerly and so is available in time; a config below the
    /// invocation directory is otherwise reached only per file, long after collection.
    ///
    /// This is what separates the two halves of a config file. Language *declarations* are gathered
    /// here, up front, because one registry serves the whole run; every behavioural setting —
    /// `remove_todos`, `preserve_patterns`, `[patterns]`, `[lint]` — stays lazily resolved per
    /// file. That keeps the cost bounded by the number of config files rather than by the number of
    /// directories: the sweep never descends further than collection itself will, prunes `.git`,
    /// honours the same ignore rules, and opens only files named like a config.
    ///
    /// The result is that the configs contributing languages are exactly the ones per-file
    /// resolution could return for the requested paths. Within that set the deepest declaration
    /// wins, and because there is a single registry for the run a language declared in a
    /// subdirectory is recognised for the whole of it.
    ///
    /// A config that cannot be parsed is skipped rather than reported here: it is reported, against
    /// its own path, when resolution reaches it — see [`Self::deferred_config_error`]. Sweeping is
    /// speculative, and a broken config in a subtree the run never touches must not fail the run.
    ///
    /// `--config` replaces discovery entirely, so this does nothing for a manager built from one.
    pub fn discover_language_sources(&mut self, paths: &[String], respect_gitignore: bool) {
        if self.forced_config.is_some() {
            return;
        }

        let mut seen: AHashSet<PathBuf> = self.ancestor_configs.iter().map(|loaded| loaded.dir.clone()).collect();
        let mut found: Vec<Arc<LoadedConfig>> = Vec::new();

        for root in Self::language_scan_roots(&self.root_dir, paths) {
            // A requested path may sit below the root directory, so the configs between the two are
            // not on the eagerly loaded chain either. This walk is bounded by path depth.
            let mut current = Some(root.as_path());
            while let Some(dir) = current {
                if !paths::is_ancestor_of(&self.ceiling, dir) {
                    break;
                }
                self.take_language_source(dir, &mut seen, &mut found);
                current = dir.parent();
            }

            let walker = ignore::WalkBuilder::new(&root)
                .hidden(false)
                .git_ignore(respect_gitignore)
                .git_global(respect_gitignore)
                .git_exclude(respect_gitignore)
                .parents(respect_gitignore)
                .require_git(false)
                .filter_entry(|entry| entry.file_name() != std::ffi::OsStr::new(".git"))
                .build();

            for entry in walker.flatten() {
                let is_config_name = entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| CONFIG_FILE_NAMES.contains(&name));
                if !is_config_name || !entry.file_type().is_some_and(|kind| kind.is_file()) {
                    continue;
                }

                let Some(dir) = entry.path().parent() else {
                    continue;
                };
                if paths::is_ancestor_of(&self.ceiling, dir) {
                    self.take_language_source(dir, &mut seen, &mut found);
                }
            }
        }

        // Deepest first, so that `language_sources` stays innermost-first and the closest
        // declaration still wins. Ties are broken by path so the order is not filesystem-dependent.
        found.sort_by(|left, right| {
            right
                .dir
                .components()
                .count()
                .cmp(&left.dir.components().count())
                .then_with(|| left.dir.cmp(&right.dir))
        });

        self.descendant_language_configs = found;
    }

    /// Load `dir`'s config into `found` unless the directory has already been accounted for.
    fn take_language_source(&self, dir: &Path, seen: &mut AHashSet<PathBuf>, found: &mut Vec<Arc<LoadedConfig>>) {
        if !seen.insert(dir.to_path_buf()) {
            return;
        }
        if let Ok(Some(loaded)) = Self::load_dir_config(dir) {
            found.push(loaded);
        }
    }

    /// The directory subtrees the requested paths can reach.
    ///
    /// A path is taken down to its literal prefix — everything before the first component holding a
    /// glob metacharacter — because the rest only filters what the walk finds. A file's own
    /// directory stands in for it. Roots contained by another root are dropped so no subtree is
    /// walked twice.
    fn language_scan_roots(root_dir: &Path, paths: &[String]) -> Vec<PathBuf> {
        let mut roots: Vec<PathBuf> = Vec::with_capacity(paths.len());
        for pattern in paths {
            // Taken apart with `Path::components` rather than by splitting on `/`: `"/a/b"` split on
            // `/` yields a leading empty segment that collects back into the *relative* `a/b`, which
            // would then be joined onto the root directory and sweep the wrong subtree entirely.
            let literal: PathBuf = Path::new(pattern)
                .components()
                .take_while(|part| {
                    !part
                        .as_os_str()
                        .to_string_lossy()
                        .contains(['*', '?', '[', ']', '{', '}'])
                })
                .collect();
            let mut root = paths::absolute_normalized(root_dir, &literal);
            if root.is_file()
                && let Some(parent) = root.parent()
            {
                root = parent.to_path_buf();
            }
            roots.push(root);
        }

        // Sorted, an enclosing directory always precedes the ones it contains.
        roots.sort();
        roots.dedup();

        let mut minimal: Vec<PathBuf> = Vec::with_capacity(roots.len());
        for root in roots {
            if !minimal.iter().any(|kept| paths::is_ancestor_of(kept, &root)) {
                minimal.push(root);
            }
        }

        minimal
    }

    /// The configs allowed to declare custom languages, innermost first so the
    /// closest one wins. A config below the root is included only once
    /// [`Self::discover_language_sources`] has been told the run will visit it; one
    /// merely stumbled upon during per-file resolution is still excluded, because by
    /// then the registry has been built and the file has already been collected or
    /// dropped.
    fn language_sources(&self) -> impl DoubleEndedIterator<Item = &Arc<LoadedConfig>> {
        self.descendant_language_configs
            .iter()
            .chain(self.ancestor_configs.iter().rev())
            .chain(self.global_config.iter())
    }

    pub fn get_language_config(&self, language_name: &str) -> Option<LanguageConfig> {
        for loaded in self.language_sources() {
            if let Some(lang_config) = loaded.config.languages.get(language_name) {
                return Some(lang_config.clone());
            }

            if let Some((_, lang_config)) = loaded
                .config
                .languages
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(language_name))
            {
                return Some(lang_config.clone());
            }
        }
        None
    }

    pub fn get_all_languages(&self) -> HashMap<String, LanguageConfig> {
        let mut languages = HashMap::new();

        for loaded in self.language_sources().rev() {
            for (name, lang_config) in &loaded.config.languages {
                languages.insert(name.clone(), lang_config.clone());
            }
        }

        languages
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An absolute input path must name itself as the subtree to sweep for `[languages]`, not a
    /// directory of the same name under the invocation directory. Splitting on `/` loses the root
    /// component, and the relative remainder would then be joined onto the root directory — so the
    /// sweep would look in a place that almost never exists and find no config at all.
    #[test]
    fn a_scan_root_from_an_absolute_path_stays_absolute() {
        let roots = ConfigManager::language_scan_roots(Path::new("/repo"), &["/elsewhere/pkg".to_string()]);
        assert_eq!(roots, vec![PathBuf::from("/elsewhere/pkg")]);
    }

    #[test]
    fn a_scan_root_stops_at_the_first_glob_component_and_drops_nested_roots() {
        let roots = ConfigManager::language_scan_roots(
            Path::new("/repo"),
            &[
                "src/**/*.rs".to_string(),
                "src/nested".to_string(),
                "other".to_string(),
                ".".to_string(),
            ],
        );
        // `.` normalizes to the root itself, which contains every other root.
        assert_eq!(roots, vec![PathBuf::from("/repo")]);

        let roots = ConfigManager::language_scan_roots(
            Path::new("/repo"),
            &["src/**/*.rs".to_string(), "src/nested".to_string(), "other".to_string()],
        );
        assert_eq!(roots, vec![PathBuf::from("/repo/other"), PathBuf::from("/repo/src")]);
    }
}
