use std::fs;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_gitignore_from_subdirectory() {
    // Create a temporary directory for our test
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create directory structure:
    // /
    // ├── .gitignore (containing ".next")
    // └── subfolder/
    //     ├── main.js
    //     └── .next/
    //         └── test.js

    let subfolder = root.join("subfolder");
    let next_folder = subfolder.join(".next");

    fs::create_dir(&subfolder).unwrap();
    fs::create_dir(&next_folder).unwrap();

    // Create .gitignore in root
    fs::write(root.join(".gitignore"), ".next\n").unwrap();

    // Create test files with comments
    fs::write(
        subfolder.join("main.js"),
        "// Main file comment\nconst x = 1;",
    )
    .unwrap();
    fs::write(
        next_folder.join("test.js"),
        "// Test file comment\nconst y = 2;",
    )
    .unwrap();

    // Initialize git repo
    Command::new("git")
        .current_dir(&root)
        .args(&["init"])
        .output()
        .unwrap();

    // Build path to uncomment binary
    let uncomment_path = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("uncomment");

    // Run uncomment from subfolder with dry-run
    let output = Command::new(&uncomment_path)
        .current_dir(&subfolder)
        .args(&[".", "--dry-run"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Debug output
    if !output.status.success() {
        panic!("Command failed with stderr: {}", stderr);
    }

    // Should only process main.js, not test.js in .next folder
    assert!(
        stdout.contains("1 files processed"),
        "Expected to process only 1 file, but got: {}",
        stdout
    );
    assert!(
        stdout.contains("main.js"),
        "Expected to process main.js, but got: {}",
        stdout
    );
    assert!(
        !stdout.contains("test.js"),
        "Should not process test.js in .next folder, but got: {}",
        stdout
    );
}

#[test]
fn test_gitignore_with_no_gitignore_flag() {
    // Create a temporary directory for our test
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create same structure as above
    let subfolder = root.join("subfolder");
    let next_folder = subfolder.join(".next");

    fs::create_dir(&subfolder).unwrap();
    fs::create_dir(&next_folder).unwrap();

    // Create .gitignore in root
    fs::write(root.join(".gitignore"), ".next\n").unwrap();

    // Create test files with comments
    fs::write(
        subfolder.join("main.js"),
        "// Main file comment\nconst x = 1;",
    )
    .unwrap();
    fs::write(
        next_folder.join("test.js"),
        "// Test file comment\nconst y = 2;",
    )
    .unwrap();

    // Initialize git repo
    Command::new("git")
        .current_dir(&root)
        .args(&["init"])
        .output()
        .unwrap();

    // Build path to uncomment binary
    let uncomment_path = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("uncomment");

    // Run uncomment from subfolder with --no-gitignore flag
    let output = Command::new(&uncomment_path)
        .current_dir(&subfolder)
        .args(&[".", "--dry-run", "--no-gitignore"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);

    // With --no-gitignore, should process both files
    assert!(
        stdout.contains("2 files processed"),
        "Expected to process 2 files with --no-gitignore, but got: {}",
        stdout
    );
}
