use anyhow::{Context, anyhow};
use saphyr::{LoadableYamlNode, Yaml};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};
use walkdir::WalkDir;

const EXTENSIONS: &[(&str, &str)] = &[(".js", "js"), (".ts", "ts"), (".py", "py")];

struct RepoList {
    repos: Vec<RepoEntry>,
}

struct RepoEntry {
    url: String,
}

#[test]
#[ignore = "Network dependent test - run manually with --ignored"]
fn integration_test_uncomment_on_real_repos() {
    let repos = read_repos_yaml("fixtures/repos/repos.yaml").expect("Failed to read repos.yaml");
    let work_dir = Path::new("tests/integration_test/repos_cache");
    fs::create_dir_all(work_dir).expect("Failed to create cache directory");

    let mut total_files = 0;
    let mut failed_files = Vec::new();
    let mut skipped_files = Vec::new();

    for repo in repos.repos {
        println!("\n=== Processing repo: {} ===", repo.url);
        let repo_name = repo.url.split('/').last().unwrap().replace(".git", "");
        let repo_path = work_dir.join(&repo_name);

        if !repo_path.exists() {
            println!("Cloning {}...", repo.url);
            assert!(
                clone_repo(&repo.url, &repo_path),
                "Failed to clone {}",
                repo.url
            );
            // Wait for files to stabilize after clone
            assert!(
                wait_for_files_to_stabilize(&repo_path, 10),
                "Repo files did not stabilize after clone"
            );
        } else {
            println!("Repo already cloned: {}", repo_name);
        }

        let files = find_source_files(&repo_path);
        println!("Found {} source files", files.len());

        for file in files {
            if !file.exists() {
                eprintln!("  [SKIP] File does not exist: {}", file.display());
                skipped_files.push(file.display().to_string());
                continue;
            }
            total_files += 1;
            println!("Testing file: {}", file.display());
            let lang = file
                .extension()
                .and_then(|ext| ext.to_str())
                .and_then(|ext| {
                    EXTENSIONS
                        .iter()
                        .find(|(e, _)| *e == format!(".{ext}"))
                        .map(|(_, l)| *l)
                })
                .unwrap_or("unknown");

            // Parse AST before uncomment
            if !parse_ast(&file, lang) {
                eprintln!(
                    "  [SKIP] Could not parse AST before uncomment: {}",
                    file.display()
                );
                skipped_files.push(file.display().to_string());
                continue;
            }

            // Run uncomment
            assert!(
                run_uncomment(&file),
                "[FAIL] Uncomment failed: {}",
                file.display()
            );

            // Parse AST after uncomment
            if !parse_ast(&file, lang) {
                eprintln!(
                    "  [FAIL] AST parse failed after uncomment: {}",
                    file.display()
                );
                failed_files.push(file.display().to_string());
            } else {
                println!("  [PASS]");
            }
        }
    }

    // Report skipped files
    if !skipped_files.is_empty() {
        eprintln!("\nThe following files were skipped (could not parse before uncomment):");
        for f in &skipped_files {
            eprintln!("  - {}", f);
        }
    }

    // Write failing files to fixtures/failing_files.txt (overwrite each run)
    if !failed_files.is_empty() {
        // File::create will overwrite the file if it exists
        let mut file = std::fs::File::create("fixtures/failing_files.txt")
            .expect("Could not create failing_files.txt");
        for f in &failed_files {
            writeln!(file, "{}", f).expect("Could not write to failing_files.txt");
        }
    }

    // Assert that there are no failed files
    if !failed_files.is_empty() {
        eprintln!("\nThe following files failed AST parsing after uncomment:");
        for f in &failed_files {
            eprintln!("  - {}", f);
        }
    }
    assert!(
        failed_files.is_empty(),
        "{} files failed AST parsing after uncomment",
        failed_files.len()
    );
    assert!(total_files > 0, "No source files were tested");
}

fn read_repos_yaml<P: AsRef<Path>>(path: P) -> anyhow::Result<RepoList> {
    let content = fs::read_to_string(path)?;
    let docs = Yaml::load_from_str(&content).context("Failed to parse YAML repo list")?;
    let doc = docs
        .first()
        .ok_or_else(|| anyhow!("repos.yaml does not contain any documents"))?;

    let repos_yaml = doc["repos"]
        .as_vec()
        .ok_or_else(|| anyhow!("repos.yaml missing 'repos' array"))?;

    let repos = repos_yaml
        .iter()
        .map(|entry| {
            let url = entry["url"]
                .as_str()
                .ok_or_else(|| anyhow!("repo entry missing 'url' field"))?;
            Ok(RepoEntry {
                url: url.to_string(),
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(RepoList { repos })
}

fn clone_repo(repo_url: &str, dest: &Path) -> bool {
    Command::new("git")
        .args(&["clone", "--depth=1", repo_url, dest.to_str().unwrap()])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn find_source_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in WalkDir::new(dir).into_iter().filter_map(Result::ok) {
        if entry.file_type().is_file() {
            let path = entry.path();
            if EXTENSIONS
                .iter()
                .any(|(ext, _)| path.extension().map_or(false, |e| e == &ext[1..]))
            {
                files.push(path.to_path_buf());
            }
        }
    }
    files
}

fn parse_ast(file: &Path, lang: &str) -> bool {
    match lang {
        "py" => parse_python_ast(file),
        "js" | "ts" => parse_js_ts_ast(file),
        _ => false,
    }
}

fn parse_python_ast(file: &Path) -> bool {
    let code = "import ast, sys; ast.parse(open(sys.argv[1]).read())";
    let output = Command::new("python3")
        .arg("-c")
        .arg(&code)
        .arg(file)
        .output();

    match output {
        Ok(out) if out.status.success() => true,
        Ok(out) => {
            eprintln!(
                "    [PY AST ERROR] {}",
                String::from_utf8_lossy(&out.stderr)
            );
            false
        }
        Err(e) => {
            eprintln!("    [PY AST ERROR] {e}");
            false
        }
    }
}

fn parse_js_ts_ast(file: &Path) -> bool {
    let code = r#"
        const fs = require('fs');
        const esprima = require('esprima');
        try {
            esprima.parseScript(fs.readFileSync(process.argv[1], 'utf8'));
        } catch (e) {
            console.error(e.message);
            process.exit(1);
        }
    "#;
    let output = Command::new("node").arg("-e").arg(code).arg(file).output();

    match output {
        Ok(out) if out.status.success() => true,
        Ok(out) => {
            eprintln!(
                "    [JS/TS AST ERROR] {}",
                String::from_utf8_lossy(&out.stderr)
            );
            false
        }
        Err(e) => {
            eprintln!("    [JS/TS AST ERROR] {e}");
            false
        }
    }
}

fn run_uncomment(file: &Path) -> bool {
    let output = Command::new("uncomment").arg(file).output();

    match output {
        Ok(out) if out.status.success() => true,
        Ok(out) => {
            eprintln!(
                "    [UNCOMMENT ERROR] {}",
                String::from_utf8_lossy(&out.stderr)
            );
            false
        }
        Err(e) => {
            eprintln!("    [UNCOMMENT ERROR] {e}");
            false
        }
    }
}

fn wait_for_files_to_stabilize(dir: &Path, timeout_secs: u64) -> bool {
    use std::time::Instant;
    let start = Instant::now();
    let mut last_count = 0;
    loop {
        let count = walkdir::WalkDir::new(dir)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .count();
        if count == last_count && count > 0 {
            return true;
        }
        last_count = count;
        if start.elapsed().as_secs() > timeout_secs {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}
