use glob::glob;
use std::path::{Path, PathBuf};
use crate::processing::file::create_gitignore_matcher;

/// Expand glob patterns into a list of file paths while respecting .gitignore
pub fn expand_paths(patterns: &[String]) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    
    for pattern in patterns {
        // Handle non-glob patterns (direct file/directory paths)
        if !pattern.contains('*') && !pattern.contains('?') && !pattern.contains('[') {
            let path = PathBuf::from(pattern);
            
            // If it's a directory, we'll handle it in collect_files
            if path.exists() {
                paths.push(path);
                continue;
            }
        }
        
        // Handle glob patterns
        if let Some(parent) = Path::new(pattern).parent() {
            let gitignore = create_gitignore_matcher(parent);
            
            if let Ok(entries) = glob(pattern) {
                for entry in entries.flatten() {
                    if !gitignore.matched(&entry, false).is_ignore() {
                        paths.push(entry);
                    }
                }
            }
        } else {
            // Handle case with no parent directory
            if let Ok(entries) = glob(pattern) {
                paths.extend(entries.flatten());
            }
        }
    }
    
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_expand_paths_with_gitignore() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();

        // Create test files
        let file1_path = dir_path.join("test1.rs");
        let file2_path = dir_path.join("test2.rs");
        let ignored_path = dir_path.join("ignored.rs");
        
        fs::write(&file1_path, "// test").unwrap();
        fs::write(&file2_path, "// test").unwrap();
        fs::write(&ignored_path, "// test").unwrap();

        // Create .gitignore
        fs::write(dir_path.join(".gitignore"), "ignored.rs\n").unwrap();

        // Test direct file path
        let pattern1 = file1_path.to_str().unwrap().to_string();
        let expanded1 = expand_paths(&[pattern1]);
        assert_eq!(expanded1.len(), 1);
        assert_eq!(expanded1[0], file1_path);

        // Test glob pattern
        let pattern2 = format!("{}/*.rs", dir_path.to_str().unwrap());
        let expanded2 = expand_paths(&[pattern2]);
        assert_eq!(expanded2.len(), 2); // Should not include ignored.rs
        assert!(expanded2.contains(&file1_path));
        assert!(expanded2.contains(&file2_path));
    }
}