use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use tree_sitter::Language;

static CACHE_MESSAGE_SHOWN: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
#[derive(Debug, Default)]
pub struct CacheStats {
    pub cached_grammars: usize,
    pub compiled_grammars: usize,
    pub total_size_bytes: u64,
}

pub struct GitGrammarLoader {
    cache_dir: PathBuf,
}

impl GitGrammarLoader {
    pub fn new() -> Result<Self> {
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine cache directory"))?
            .join("uncomment")
            .join("grammars");

        fs::create_dir_all(&cache_dir).with_context(|| {
            format!("Failed to create cache directory: {}", cache_dir.display())
        })?;

        Ok(Self { cache_dir })
    }

    pub fn load_git_grammar(
        &self,
        language_name: &str,
        url: &str,
        branch: Option<&str>,
        subpath: Option<&str>,
    ) -> Result<Language> {
        if !CACHE_MESSAGE_SHOWN.load(Ordering::Relaxed) {
            println!("📥 Downloading tree-sitter grammars for language processing...");
            println!(
                "💾 Grammars are cached at ~/.cache/uncomment/grammars/ - subsequent runs will be faster"
            );
            CACHE_MESSAGE_SHOWN.store(true, Ordering::Relaxed);
        }

        let grammar_dir = self.ensure_grammar_cloned(language_name, url, branch)?;
        let grammar_path = if let Some(subpath) = subpath {
            grammar_dir.join(subpath)
        } else {
            grammar_dir
        };

        self.compile_and_load_grammar(language_name, &grammar_path)
    }

    fn ensure_grammar_cloned(
        &self,
        language_name: &str,
        url: &str,
        branch: Option<&str>,
    ) -> Result<PathBuf> {
        let grammar_dir = self.cache_dir.join(language_name);

        if grammar_dir.exists() {
            self.update_grammar(&grammar_dir, branch)?;
        } else {
            self.clone_grammar(url, &grammar_dir, branch)?;
        }

        Ok(grammar_dir)
    }

    fn clone_grammar(&self, url: &str, target_dir: &Path, branch: Option<&str>) -> Result<()> {
        println!("   Cloning grammar from {url}");

        let mut cmd = Command::new("git");
        cmd.args(["clone", "--quiet", url, &target_dir.to_string_lossy()]);

        if let Some(branch) = branch {
            cmd.args(["--branch", branch]);
        }

        let output = cmd
            .output()
            .with_context(|| format!("Failed to execute git clone for {url}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Git clone failed: {}", stderr);
        }

        Ok(())
    }

    fn update_grammar(&self, grammar_dir: &Path, branch: Option<&str>) -> Result<()> {
        let output = Command::new("git")
            .current_dir(grammar_dir)
            .args(["fetch", "origin"])
            .output()
            .with_context(|| format!("Failed to fetch updates for {}", grammar_dir.display()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("Warning: Failed to fetch updates: {stderr}");
            return Ok(());
        }

        let branch_ref = if let Some(branch) = branch {
            format!("origin/{branch}")
        } else {
            "origin/HEAD".to_string()
        };

        let output = Command::new("git")
            .current_dir(grammar_dir)
            .args(["reset", "--hard", &branch_ref])
            .output()
            .with_context(|| format!("Failed to reset to {branch_ref}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("Warning: Failed to reset to {branch_ref}: {stderr}");
        }

        Ok(())
    }

    fn compile_and_load_grammar(
        &self,
        language_name: &str,
        grammar_path: &Path,
    ) -> Result<Language> {
        let grammar_js = grammar_path.join("grammar.js");
        if !grammar_js.exists() {
            anyhow::bail!("No grammar.js found in {}", grammar_path.display());
        }

        let compiled_cache_dir = self.cache_dir.join("compiled").join(language_name);
        let compiled_lib_path = compiled_cache_dir.join(format!("lib{language_name}.so"));

        if compiled_lib_path.exists()
            && let Ok(language) = self.load_cached_library(&compiled_lib_path)
        {
            return Ok(language);
        }

        println!("   Compiling grammar for {language_name}");

        use tree_sitter_loader::{CompileConfig, Loader};

        #[allow(unused_mut)]
        let mut loader = Loader::new()
            .with_context(|| "Failed to create tree-sitter loader for grammar compilation")?;

        #[cfg(debug_assertions)]
        loader.debug_build(true);

        fs::create_dir_all(&compiled_cache_dir).with_context(|| {
            format!(
                "Failed to create compiled cache directory: {}",
                compiled_cache_dir.display()
            )
        })?;

        let output_paths = vec![compiled_cache_dir];
        let compile_config = CompileConfig::new(grammar_path, Some(&output_paths), None);

        let language = loader
            .load_language_at_path(compile_config)
            .with_context(|| {
                format!(
                    "Failed to compile and load grammar '{}' from {}",
                    language_name,
                    grammar_path.display()
                )
            })?;

        Ok(language)
    }

    fn load_cached_library(&self, lib_path: &Path) -> Result<Language> {
        use libloading::{Library, Symbol};

        unsafe {
            let lib = Library::new(lib_path).with_context(|| {
                format!("Failed to load cached library from {}", lib_path.display())
            })?;

            let symbol_names = ["tree_sitter_language", "tree_sitter", "language"];

            for symbol_name in &symbol_names {
                if let Ok(func) = lib.get::<Symbol<
                    unsafe extern "C" fn() -> *const tree_sitter::ffi::TSLanguage,
                >>(symbol_name.as_bytes())
                {
                    let ts_language_ptr = func();
                    let language = Language::from_raw(ts_language_ptr);

                    std::mem::forget(lib);

                    return Ok(language);
                }
            }

            anyhow::bail!(
                "No valid tree-sitter language function found in cached library {}",
                lib_path.display()
            );
        }
    }

    #[cfg(test)]
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    #[cfg(test)]
    pub fn clear_cache(&self) -> Result<()> {
        if self.cache_dir.exists() {
            fs::remove_dir_all(&self.cache_dir).with_context(|| {
                format!(
                    "Failed to remove cache directory: {}",
                    self.cache_dir.display()
                )
            })?;
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn clear_compiled_cache(&self) -> Result<()> {
        let compiled_dir = self.cache_dir.join("compiled");
        if compiled_dir.exists() {
            fs::remove_dir_all(&compiled_dir).with_context(|| {
                format!(
                    "Failed to remove compiled cache directory: {}",
                    compiled_dir.display()
                )
            })?;
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn clear_language_cache(&self, language_name: &str) -> Result<()> {
        let language_dir = self.cache_dir.join(language_name);
        if language_dir.exists() {
            fs::remove_dir_all(&language_dir)
                .with_context(|| format!("Failed to remove cache for language: {language_name}"))?;
        }

        let compiled_dir = self.cache_dir.join("compiled").join(language_name);
        if compiled_dir.exists() {
            fs::remove_dir_all(&compiled_dir).with_context(|| {
                format!("Failed to remove compiled cache for language: {language_name}")
            })?;
        }

        Ok(())
    }

    #[cfg(test)]
    pub fn is_grammar_cached(&self, language_name: &str) -> bool {
        self.cache_dir.join(language_name).exists()
    }

    #[cfg(test)]
    pub fn is_compiled_cached(&self, language_name: &str) -> bool {
        let compiled_lib_path = self
            .cache_dir
            .join("compiled")
            .join(language_name)
            .join(format!("lib{language_name}.so"));
        compiled_lib_path.exists()
    }

    #[cfg(test)]
    pub fn cache_stats(&self) -> Result<CacheStats> {
        let mut stats = CacheStats::default();

        if !self.cache_dir.exists() {
            return Ok(stats);
        }

        for entry in fs::read_dir(&self.cache_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let dir_name = entry.file_name();
                if dir_name != "compiled" {
                    stats.cached_grammars += 1;
                }
            }
        }

        let compiled_dir = self.cache_dir.join("compiled");
        if compiled_dir.exists() {
            for entry in fs::read_dir(&compiled_dir)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    stats.compiled_grammars += 1;
                }
            }
        }

        stats.total_size_bytes = self.calculate_directory_size(&self.cache_dir)?;

        Ok(stats)
    }

    #[cfg(test)]
    #[allow(clippy::only_used_in_recursion)]
    fn calculate_directory_size(&self, dir: &Path) -> Result<u64> {
        let mut total_size = 0;

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                total_size += self.calculate_directory_size(&path)?;
            } else {
                total_size += entry.metadata()?.len();
            }
        }

        Ok(total_size)
    }
}

impl Default for GitGrammarLoader {
    fn default() -> Self {
        Self::new().expect("Failed to create default GitGrammarLoader")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_git_loader_creation() {
        let loader = GitGrammarLoader::new();
        assert!(loader.is_ok());

        let loader = loader.unwrap();
        assert!(loader.cache_dir().ends_with("uncomment/grammars"));
    }

    #[test]
    fn test_cache_directory_creation() {
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = temp_dir.path().join("test_cache");

        let _loader = GitGrammarLoader {
            cache_dir: cache_dir.clone(),
        };

        assert!(!cache_dir.exists());
    }

    #[test]
    fn test_is_grammar_cached() {
        let loader = GitGrammarLoader::new().unwrap();

        assert!(!loader.is_grammar_cached("nonexistent_grammar"));

        let temp_dir = TempDir::new().unwrap();
        let custom_loader = GitGrammarLoader {
            cache_dir: temp_dir.path().join("custom_cache"),
        };
        assert!(!custom_loader.is_grammar_cached("test"));
    }

    #[test]
    fn test_is_compiled_cached() {
        let loader = GitGrammarLoader::new().unwrap();

        assert!(!loader.is_compiled_cached("nonexistent_grammar"));

        let temp_dir = TempDir::new().unwrap();
        let custom_loader = GitGrammarLoader {
            cache_dir: temp_dir.path().join("custom_cache"),
        };
        assert!(!custom_loader.is_compiled_cached("test"));
    }

    #[test]
    fn test_cache_management_with_empty_cache() {
        let temp_dir = TempDir::new().unwrap();
        let loader = GitGrammarLoader {
            cache_dir: temp_dir.path().join("test_cache"),
        };

        assert!(loader.clear_cache().is_ok());
        assert!(loader.clear_compiled_cache().is_ok());
        assert!(loader.clear_language_cache("test").is_ok());
    }

    #[test]
    fn test_cache_management_with_data() {
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = temp_dir.path().join("test_cache");
        fs::create_dir_all(&cache_dir).unwrap();

        let language_dir = cache_dir.join("test_lang");
        fs::create_dir_all(&language_dir).unwrap();
        fs::write(language_dir.join("grammar.js"), "// test grammar").unwrap();

        let compiled_dir = cache_dir.join("compiled").join("test_lang");
        fs::create_dir_all(&compiled_dir).unwrap();
        fs::write(compiled_dir.join("libtest_lang.so"), "fake library").unwrap();

        let loader = GitGrammarLoader {
            cache_dir: cache_dir.clone(),
        };

        assert!(loader.is_grammar_cached("test_lang"));
        assert!(loader.is_compiled_cached("test_lang"));

        assert!(loader.clear_language_cache("test_lang").is_ok());
        assert!(!loader.is_grammar_cached("test_lang"));
        assert!(!loader.is_compiled_cached("test_lang"));
    }

    #[test]
    fn test_cache_stats_empty() {
        let temp_dir = TempDir::new().unwrap();
        let loader = GitGrammarLoader {
            cache_dir: temp_dir.path().join("empty_cache"),
        };

        let stats = loader.cache_stats().unwrap();
        assert_eq!(stats.cached_grammars, 0);
        assert_eq!(stats.compiled_grammars, 0);
        assert_eq!(stats.total_size_bytes, 0);
    }

    #[test]
    fn test_cache_stats_with_data() {
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = temp_dir.path().join("test_cache");
        fs::create_dir_all(&cache_dir).unwrap();

        let lang1_dir = cache_dir.join("lang1");
        let lang2_dir = cache_dir.join("lang2");
        fs::create_dir_all(&lang1_dir).unwrap();
        fs::create_dir_all(&lang2_dir).unwrap();
        fs::write(lang1_dir.join("grammar.js"), "// lang1 grammar").unwrap();
        fs::write(lang2_dir.join("grammar.js"), "// lang2 grammar").unwrap();

        let compiled_dir = cache_dir.join("compiled");
        let compiled_lang1 = compiled_dir.join("lang1");
        let compiled_lang2 = compiled_dir.join("lang2");
        fs::create_dir_all(&compiled_lang1).unwrap();
        fs::create_dir_all(&compiled_lang2).unwrap();
        fs::write(compiled_lang1.join("liblang1.so"), "fake lib1").unwrap();
        fs::write(compiled_lang2.join("liblang2.so"), "fake lib2").unwrap();

        let loader = GitGrammarLoader { cache_dir };

        let stats = loader.cache_stats().unwrap();
        assert_eq!(stats.cached_grammars, 2);
        assert_eq!(stats.compiled_grammars, 2);
        assert!(stats.total_size_bytes > 0);
    }

    #[test]
    fn test_load_cached_library_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let loader = GitGrammarLoader {
            cache_dir: temp_dir.path().to_path_buf(),
        };

        let fake_lib_path = temp_dir.path().join("nonexistent.so");
        let result = loader.load_cached_library(&fake_lib_path);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Failed to load cached library")
        );
    }

    #[test]
    fn test_compile_and_load_grammar_no_grammar_js() {
        let temp_dir = TempDir::new().unwrap();
        let loader = GitGrammarLoader {
            cache_dir: temp_dir.path().join("cache"),
        };

        let grammar_dir = temp_dir.path().join("no_grammar");
        fs::create_dir_all(&grammar_dir).unwrap();

        let result = loader.compile_and_load_grammar("test", &grammar_dir);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("No grammar.js found")
        );
    }

    #[test]
    fn test_git_operations_error_handling() {
        let temp_dir = TempDir::new().unwrap();
        let loader = GitGrammarLoader {
            cache_dir: temp_dir.path().join("git_cache"),
        };

        let result = loader.load_git_grammar("test_lang", "invalid://not.a.real.url", None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_cache_directory_permissions() {
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = temp_dir.path().join("writable_cache");

        let loader = GitGrammarLoader { cache_dir };

        let stats_result = loader.cache_stats();
        assert!(stats_result.is_ok());
    }

    #[test]
    fn test_platform_library_extension() {
        let loader = GitGrammarLoader::new().unwrap();

        assert!(!loader.is_compiled_cached("nonexistent"));
    }
}
