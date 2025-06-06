#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use tempfile::tempdir;

    #[test]
    fn test_gitignore_respected() {
        let dir = tempdir().unwrap();
        
        // Create a file to ignore
        File::create(dir.path().join("ignored.txt")).unwrap();
        
        // Create .gitignore
        fs::write(dir.path().join(".gitignore"), "ignored.txt").unwrap();
        
        // Create a file to process
        File::create(dir.path().join("processed.txt")).unwrap();
        
        let files = collect_files(&[dir.path().to_path_buf()]);
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("processed.txt"));
    }
}