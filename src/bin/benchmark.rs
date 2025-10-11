use clap::Parser;
use std::path::PathBuf;
use std::time::Instant;

mod bench {
    include!("../../bench/mod.rs");
}

#[derive(Parser)]
#[command(
    name = "benchmark",
    about = "Benchmark tool for testing uncomment performance on large codebases"
)]
struct BenchmarkCli {
    #[arg(short, long, default_value = "./target/release/uncomment")]
    uncomment_binary: PathBuf,

    #[arg(short, long)]
    target: PathBuf,

    #[arg(short, long)]
    sample_size: Option<usize>,

    #[arg(short, long, default_value = "1")]
    iterations: usize,

    #[arg(short, long)]
    language: Option<String>,

    #[arg(short, long)]
    memory_profile: bool,

    #[arg(short = 'j', long, default_value = "1")]
    threads: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = BenchmarkCli::parse();

    if !cli.uncomment_binary.exists() {
        eprintln!(
            "❌ Uncomment binary not found: {}",
            cli.uncomment_binary.display()
        );
        std::process::exit(1);
    }

    if !cli.target.exists() {
        eprintln!("❌ Target directory not found: {}", cli.target.display());
        std::process::exit(1);
    }

    println!("🎯 UNCOMMENT PERFORMANCE BENCHMARK");
    println!("==================================");
    println!("🔧 Binary: {}", cli.uncomment_binary.display());
    println!("📁 Target: {}", cli.target.display());
    println!("🔄 Iterations: {}", cli.iterations);

    if let Some(lang) = &cli.language {
        println!("🗣️  Language filter: {lang}");
    }

    if cli.memory_profile {
        println!("💾 Memory profiling: enabled");
    }

    let mut results = Vec::new();
    let overall_start = Instant::now();

    for iteration in 1..=cli.iterations {
        println!("\n🏃 Running iteration {}/{}...", iteration, cli.iterations);

        let result = bench::run_benchmark(&cli.uncomment_binary, &cli.target, cli.sample_size)?;

        result.print_summary();
        results.push(result);

        if cli.iterations > 1 && iteration < cli.iterations {
            println!("\n⏸️  Waiting 2s before next iteration...");
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    }

    let total_duration = overall_start.elapsed();

    if cli.iterations > 1 {
        println!("\n📊 AGGREGATE RESULTS ({} iterations)", cli.iterations);
        println!("=====================================");

        let avg_duration = results
            .iter()
            .map(|r| r.duration.as_secs_f64())
            .sum::<f64>()
            / results.len() as f64;

        let avg_files_per_sec =
            results.iter().map(|r| r.files_per_second).sum::<f64>() / results.len() as f64;

        let avg_comments_per_sec =
            results.iter().map(|r| r.comments_per_second).sum::<f64>() / results.len() as f64;

        let total_files = results[0].total_files;
        let total_comments = results
            .iter()
            .map(|r| r.total_comments_removed)
            .sum::<usize>()
            / results.len();

        println!("⏱️  Average duration: {avg_duration:.2}s");
        println!("🚀 Average files/sec: {avg_files_per_sec:.1}");
        println!("💬 Average comments/sec: {avg_comments_per_sec:.1}");
        println!("📂 Total files: {total_files}");
        println!("🗑️  Avg comments removed: {total_comments}");

        let durations: Vec<f64> = results.iter().map(|r| r.duration.as_secs_f64()).collect();
        let min_duration = durations.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_duration = durations.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let variance = (max_duration - min_duration) / avg_duration * 100.0;

        println!("📈 Performance variance: {variance:.1}%");
        println!(
            "⏰ Total benchmark time: {:.2}s",
            total_duration.as_secs_f64()
        );
    }

    println!("\n🔍 PERFORMANCE ANALYSIS");
    println!("=======================");

    let last_result = &results[results.len() - 1];

    if last_result.files_per_second < 10.0 {
        println!(
            "⚠️  Performance concern: Low throughput ({:.1} files/sec)",
            last_result.files_per_second
        );
        println!("💡 Consider optimizations:");
        println!("   • Parallel processing");
        println!("   • I/O buffering improvements");
        println!("   • Parser initialization caching");
    } else if last_result.files_per_second < 100.0 {
        println!(
            "✅ Good performance: {:.1} files/sec",
            last_result.files_per_second
        );
        println!("💡 Potential improvements:");
        println!("   • Multi-threading for large directories");
        println!("   • Memory-mapped file reading");
    } else {
        println!(
            "🚀 Excellent performance: {:.1} files/sec",
            last_result.files_per_second
        );
        println!("🎉 Performance is already optimized!");
    }

    if cli.sample_size.is_some() {
        let estimated_full_time = 850_000.0 / last_result.files_per_second;
        println!("\n📊 FULL CODEBASE ESTIMATE");
        println!("=========================");
        println!(
            "🏢 For ~850k files (Armis-scale): ~{:.1} minutes",
            estimated_full_time / 60.0
        );

        if estimated_full_time > 300.0 {
            println!("⚠️  Consider optimization for large-scale usage");
        }
    }

    println!("\n✅ Benchmark completed successfully!");

    Ok(())
}
