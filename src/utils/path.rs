use glob::glob;
use std::path::PathBuf;

/// Expand glob patterns into a list of file paths
pub fn expand_paths(patterns: &[String]) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for pattern in patterns {
        if !pattern.contains('*') && !pattern.contains('?') && !pattern.contains('[') {
            let path = PathBuf::from(pattern);
            if path.is_dir() {
                let recursive_pattern = format!("{}/**/*", pattern);
                let expanded = expand_paths(&[recursive_pattern]);
                paths.extend(expanded);
                continue;
            }
        }

        match glob(pattern) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    if entry.is_file() {
                        paths.push(entry);
                    }
                }
            }
            Err(err) => eprintln!("Invalid pattern '{}': {}", pattern, err),
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
    fn test_expand_paths() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();

        let file1_path = dir_path.join("test1.rs");
        let file2_path = dir_path.join("test2.rs");
        let file3_path = dir_path.join("test3.js");

        fs::write(&file1_path, "// test").unwrap();
        fs::write(&file2_path, "// test").unwrap();
        fs::write(&file3_path, "// test").unwrap();

        let pattern1 = file1_path.to_str().unwrap().to_string();
        let expanded1 = expand_paths(&[pattern1]);
        assert_eq!(expanded1.len(), 1);
        assert_eq!(expanded1[0], file1_path);

        let pattern2 = format!("{}/*.rs", dir_path.to_str().unwrap());
        let expanded2 = expand_paths(&[pattern2]);
        assert_eq!(expanded2.len(), 2);
        assert!(expanded2.contains(&file1_path));
        assert!(expanded2.contains(&file2_path));

        let pattern2_clone = format!("{}/*.rs", dir_path.to_str().unwrap()); // Create a new pattern2 clone
        let pattern3 = format!("{}/*.js", dir_path.to_str().unwrap());
        let expanded3 = expand_paths(&[pattern2_clone, pattern3]);
        assert_eq!(expanded3.len(), 3);
        assert!(expanded3.contains(&file1_path));
        assert!(expanded3.contains(&file2_path));
        assert!(expanded3.contains(&file3_path));
    }
}
