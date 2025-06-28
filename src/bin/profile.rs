use clap::Parser;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(name = "profile", about = "Profile uncomment performance")]
struct ProfileCli {
    /// Path to test
    #[arg(short, long)]
    path: PathBuf,

    /// Number of warmup runs
    #[arg(short, long, default_value = "3")]
    warmup: usize,

    /// Number of measurement runs
    #[arg(short, long, default_value = "10")]
    runs: usize,

    /// Show per-file statistics
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = ProfileCli::parse();

    println!("🔬 UNCOMMENT PERFORMANCE PROFILER");
    println!("=================================");
    println!("📁 Target: {}", cli.path.display());
    println!("🔥 Warmup runs: {}", cli.warmup);
    println!("📊 Measurement runs: {}", cli.runs);
    println!();

    // Count files first
    let files = collect_files(&cli.path)?;
    println!("📂 Found {} files to process", files.len());

    if files.is_empty() {
        println!("❌ No supported files found!");
        return Ok(());
    }

    // Show file type breakdown
    let mut type_counts = std::collections::HashMap::new();
    for file in &files {
        if let Some(ext) = file.extension() {
            *type_counts
                .entry(ext.to_string_lossy().to_string())
                .or_insert(0) += 1;
        }
    }

    println!("\n📊 File type breakdown:");
    for (ext, count) in type_counts.iter() {
        println!("   • .{ext}: {count} files");
    }

    // Calculate total size
    let total_size: u64 = files
        .iter()
        .filter_map(|f| fs::metadata(f).ok())
        .map(|m| m.len())
        .sum();

    println!("\n💾 Total size: {:.2} MB", total_size as f64 / 1_048_576.0);

    // Warmup runs
    println!("\n🔥 Running {} warmup iterations...", cli.warmup);
    for i in 1..=cli.warmup {
        print!("   Warmup {}/{}... ", i, cli.warmup);
        let duration = run_uncomment(&files)?;
        println!("{:.3}s", duration.as_secs_f64());
    }

    // Measurement runs
    println!("\n📊 Running {} measurement iterations...", cli.runs);
    let mut durations = Vec::new();
    let mut file_counts = Vec::new();

    for i in 1..=cli.runs {
        print!("   Run {}/{}... ", i, cli.runs);
        let start = Instant::now();
        let result = run_uncomment_with_stats(&files)?;
        let duration = start.elapsed();

        println!(
            "{:.3}s ({} files modified)",
            duration.as_secs_f64(),
            result.modified_files
        );

        durations.push(duration);
        file_counts.push(result);
    }

    // Calculate statistics
    let avg_duration =
        durations.iter().map(|d| d.as_secs_f64()).sum::<f64>() / durations.len() as f64;

    let min_duration = durations
        .iter()
        .map(|d| d.as_secs_f64())
        .fold(f64::INFINITY, |a, b| a.min(b));

    let max_duration = durations
        .iter()
        .map(|d| d.as_secs_f64())
        .fold(f64::NEG_INFINITY, |a, b| a.max(b));

    let files_per_second = files.len() as f64 / avg_duration;
    let mb_per_second = (total_size as f64 / 1_048_576.0) / avg_duration;

    // Print results
    println!("\n📈 PERFORMANCE RESULTS");
    println!("=====================");
    println!("⏱️  Timing:");
    println!("   • Average: {avg_duration:.3}s");
    println!("   • Min: {min_duration:.3}s");
    println!("   • Max: {max_duration:.3}s");
    println!(
        "   • Variance: {:.1}%",
        (max_duration - min_duration) / avg_duration * 100.0
    );

    println!("\n🚀 Throughput:");
    println!("   • Files/sec: {files_per_second:.1}");
    println!("   • MB/sec: {mb_per_second:.2}");
    println!(
        "   • μs/file: {:.1}",
        avg_duration * 1_000_000.0 / files.len() as f64
    );

    // Performance analysis
    println!("\n🔍 PERFORMANCE ANALYSIS");
    println!("=======================");

    if files_per_second < 100.0 {
        println!("⚠️  Low throughput detected!");
        println!("\n🔧 Optimization opportunities:");
        println!("   1. Parser initialization caching");
        println!("   2. Parallel file processing");
        println!("   3. Memory-mapped I/O for large files");
        println!("   4. Batch small files together");
    } else if files_per_second < 1000.0 {
        println!("✅ Good performance");
        println!("\n💡 Possible improvements:");
        println!("   1. Thread pool for I/O operations");
        println!("   2. SIMD string operations");
        println!("   3. Zero-copy parsing");
    } else {
        println!("🚀 Excellent performance!");
        println!("   The tool is well-optimized for this workload.");
    }

    // Memory usage estimate
    println!("\n💾 Resource usage:");
    println!(
        "   • Estimated memory per file: ~{:.1} KB",
        estimate_memory_per_file(&files)
    );
    println!("   • Parser overhead: ~{:.1} MB per language", 2.5);

    Ok(())
}

struct ProcessResult {
    modified_files: usize,
    #[allow(dead_code)]
    total_comments: usize,
}

fn collect_files(path: &PathBuf) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut files = Vec::new();

    if path.is_file() {
        files.push(path.clone());
    } else if path.is_dir() {
        use walkdir::WalkDir;

        for entry in WalkDir::new(path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                if let Some(ext) = entry.path().extension() {
                    let ext_str = ext.to_string_lossy();
                    let supported = matches!(
                        ext_str.as_ref(),
                        "py" | "pyw"
                            | "pyi"
                            | "js"
                            | "jsx"
                            | "mjs"
                            | "cjs"
                            | "ts"
                            | "tsx"
                            | "mts"
                            | "cts"
                            | "rs"
                            | "go"
                            | "java"
                            | "c"
                            | "h"
                            | "cpp"
                            | "cc"
                            | "cxx"
                            | "hpp"
                            | "hxx"
                            | "rb"
                            | "rake"
                    );

                    if supported {
                        files.push(entry.path().to_path_buf());
                    }
                }
            }
        }
    }

    Ok(files)
}

fn run_uncomment(files: &[PathBuf]) -> Result<Duration, Box<dyn std::error::Error>> {
    let start = Instant::now();

    let mut processor = uncomment::processor::Processor::new();
    let options = uncomment::processor::ProcessingOptions {
        remove_todo: false,
        remove_fixme: false,
        remove_doc: false,
        custom_preserve_patterns: vec![],
        use_default_ignores: true,
        dry_run: true,
        respect_gitignore: false,
        traverse_git_repos: false,
    };

    for file in files {
        let _ = processor.process_file(file, &options);
    }

    Ok(start.elapsed())
}

fn run_uncomment_with_stats(
    files: &[PathBuf],
) -> Result<ProcessResult, Box<dyn std::error::Error>> {
    let mut processor = uncomment::processor::Processor::new();
    let options = uncomment::processor::ProcessingOptions {
        remove_todo: false,
        remove_fixme: false,
        remove_doc: false,
        custom_preserve_patterns: vec![],
        use_default_ignores: true,
        dry_run: true,
        respect_gitignore: false,
        traverse_git_repos: false,
    };

    let mut modified_files = 0;
    let mut total_comments = 0;

    for file in files {
        if let Ok(result) = processor.process_file(file, &options) {
            if result.original_content != result.processed_content {
                modified_files += 1;
                total_comments += result.comments_removed;
            }
        }
    }

    Ok(ProcessResult {
        modified_files,
        total_comments,
    })
}

fn estimate_memory_per_file(files: &[PathBuf]) -> f64 {
    // Estimate based on average file size
    let total_size: u64 = files
        .iter()
        .take(100) // Sample first 100 files
        .filter_map(|f| fs::metadata(f).ok())
        .map(|m| m.len())
        .sum();

    let avg_size = total_size as f64 / files.len().min(100) as f64;

    // Memory usage is roughly:
    // - File content (1x)
    // - AST nodes (~2x content size)
    // - Comment tracking (~0.5x)
    // Total: ~3.5x file size

    (avg_size * 3.5) / 1024.0
}
