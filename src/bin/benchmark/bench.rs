use anyhow::{Context, Result};
use std::path::Path;
use std::time::{Duration, Instant};

pub struct BenchmarkResult {
    pub total_files: usize,
    pub processed_files: usize,
    pub modified_files: usize,
    pub total_comments_removed: usize,
    pub duration: Duration,
    pub files_per_second: f64,
    pub comments_per_second: f64,
}

impl BenchmarkResult {
    pub fn print_summary(&self) {
        println!("\n🚀 BENCHMARK RESULTS");
        println!("==================");
        println!("📊 Files:");
        println!("  • Total found: {}", self.total_files);
        println!("  • Processed: {}", self.processed_files);
        println!("  • Modified: {}", self.modified_files);
        println!(
            "  • Modification rate: {:.1}%",
            (self.modified_files as f64 / self.processed_files as f64) * 100.0
        );

        println!("\n💬 Comments:");
        println!("  • Total removed: {}", self.total_comments_removed);
        println!(
            "  • Avg per file: {:.1}",
            self.total_comments_removed as f64 / self.processed_files as f64
        );

        println!("\n⚡ Performance:");
        println!("  • Duration: {:.2}s", self.duration.as_secs_f64());
        println!("  • Files/sec: {:.1}", self.files_per_second);
        println!("  • Comments/sec: {:.1}", self.comments_per_second);

        println!("\n📈 Throughput:");
        if self.duration.as_secs() > 0 {
            let mb_per_sec =
                (self.processed_files as f64 * 5.0) / 1024.0 / self.duration.as_secs_f64();
            println!("  • Est. ~{mb_per_sec:.1} MB/sec");
        }
    }
}

pub fn run_benchmark(
    uncomment_binary: &Path,
    target_dir: &Path,
    threads: usize,
) -> Result<BenchmarkResult> {
    let start_time = Instant::now();

    println!("🔍 Starting uncomment benchmark...");
    println!("📁 Target: {}", target_dir.display());

    let mut cmd = std::process::Command::new(uncomment_binary);
    cmd.arg(target_dir).arg("--dry-run").arg("--verbose");

    if threads > 0 {
        cmd.arg("--threads").arg(threads.to_string());
    }

    println!("🚀 Running benchmark...\n");

    let output = cmd
        .output()
        .context("failed to execute benchmark command")?;
    let duration = start_time.elapsed();

    if !output.status.success() {
        anyhow::bail!(
            "benchmark command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut total_files = 0;
    let mut processed_files = 0;
    let mut modified_files = 0;
    let mut total_comments_removed = 0;

    for line in stdout.lines() {
        if line.contains("[DRY RUN] Would modify:") {
            modified_files += 1;
            processed_files += 1;
        } else if line.contains("✓ No changes needed:") {
            processed_files += 1;
        } else if line.contains("Removed")
            && line.contains("comment(s)")
            && let Some(parts) = line.split("Removed ").nth(1)
            && let Some(count_str) = parts.split(" comment").next()
            && let Ok(count) = count_str.trim().parse::<usize>()
        {
            total_comments_removed += count;
        }
    }

    for line in stdout.lines() {
        if line.contains("Summary:")
            && line.contains("files processed")
            && let Some(summary_part) = line.split("Summary: ").nth(1)
        {
            if let Some(files_part) = summary_part.split(" files processed").next()
                && let Ok(count) = files_part.trim().parse::<usize>()
            {
                total_files = count;
                processed_files = count;
            }

            if let Some(modified_part) = summary_part.split(", ").nth(1)
                && let Some(modified_str) = modified_part.split(' ').next()
                && let Ok(count) = modified_str.trim().parse::<usize>()
            {
                modified_files = count;
            }
        }
    }

    if total_files == 0 {
        total_files = processed_files;
    }

    let files_per_second = if duration.as_secs_f64() > 0.0 {
        processed_files as f64 / duration.as_secs_f64()
    } else {
        0.0
    };

    let comments_per_second = if duration.as_secs_f64() > 0.0 {
        total_comments_removed as f64 / duration.as_secs_f64()
    } else {
        0.0
    };

    Ok(BenchmarkResult {
        total_files,
        processed_files,
        modified_files,
        total_comments_removed,
        duration,
        files_per_second,
        comments_per_second,
    })
}
