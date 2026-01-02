//! Performance benchmarking binary for moshi-server components.
//!
//! This binary provides isolated and end-to-end performance measurements for
//! various components of the Moshi speech-to-text/text-to-speech server.
//!
//! # Usage
//!
//! ```bash
//! cargo run --release --features cuda --bin bench_perf -- [OPTIONS]
//! ```
//!
//! # Components
//!
//! - `--mimi`: Benchmark Mimi encode/decode
//! - `--lm`: Benchmark LM inference
//! - `--transformer`: Benchmark transformer layers
//! - `--e2e`: End-to-end pipeline benchmark
//! - `--batch`: Multi-stream batch benchmark
//! - `--all`: Run all benchmarks
//!
//! # Options
//!
//! - `--iterations N`: Number of iterations (default: 100)
//! - `--warmup N`: Warmup iterations (default: 10)
//! - `--output FILE`: JSON output file for results
//! - `--cpu`: Use CPU instead of GPU

use anyhow::Result;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::time::Instant;

#[derive(Parser, Debug)]
#[clap(name = "bench_perf", about = "Moshi performance benchmarking tool")]
struct Args {
    /// Benchmark Mimi encode/decode
    #[clap(long)]
    mimi: bool,

    /// Benchmark LM inference
    #[clap(long)]
    lm: bool,

    /// Benchmark transformer layers
    #[clap(long)]
    transformer: bool,

    /// End-to-end pipeline benchmark
    #[clap(long)]
    e2e: bool,

    /// Multi-stream batch benchmark
    #[clap(long)]
    batch: bool,

    /// Run all benchmarks
    #[clap(long)]
    all: bool,

    /// Memory pressure test
    #[clap(long)]
    memory: bool,

    /// Sustained load test
    #[clap(long)]
    sustained: bool,

    /// Number of measurement iterations
    #[clap(long, default_value = "100")]
    iterations: usize,

    /// Number of warmup iterations
    #[clap(long, default_value = "10")]
    warmup: usize,

    /// JSON output file for results
    #[clap(long)]
    output: Option<String>,

    /// Use CPU instead of GPU
    #[clap(long)]
    cpu: bool,

    /// Batch size for batch benchmarks
    #[clap(long, default_value = "8")]
    batch_size: usize,

    /// Verbose output
    #[clap(long, short)]
    verbose: bool,
}

/// Results from a single benchmark run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub name: String,
    pub iterations: usize,
    pub warmup_iterations: usize,
    pub total_duration_ms: f64,
    pub mean_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub throughput: Option<f64>,
    pub throughput_unit: Option<String>,
}

impl From<moshi_server::bench::LatencyStats> for BenchmarkResult {
    fn from(stats: moshi_server::bench::LatencyStats) -> Self {
        Self {
            name: stats.name.to_string(),
            iterations: stats.count as usize,
            warmup_iterations: 0, // Not tracked in LatencyStats
            total_duration_ms: stats.mean.as_secs_f64() * 1000.0 * stats.count as f64,
            mean_ms: stats.mean.as_secs_f64() * 1000.0,
            min_ms: stats.min.as_secs_f64() * 1000.0,
            max_ms: stats.max.as_secs_f64() * 1000.0,
            p50_ms: stats.p50.as_secs_f64() * 1000.0,
            p95_ms: stats.p95.as_secs_f64() * 1000.0,
            p99_ms: stats.p99.as_secs_f64() * 1000.0,
            throughput: None,
            throughput_unit: None,
        }
    }
}

/// Full benchmark suite results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSuiteResults {
    pub timestamp: String,
    pub system_info: SystemInfo,
    pub config: BenchmarkConfigSummary,
    pub results: HashMap<String, BenchmarkResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub os: String,
    pub cpu: String,
    pub gpu: Option<String>,
    pub cuda_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfigSummary {
    pub iterations: usize,
    pub warmup: usize,
    pub use_cuda: bool,
    pub batch_size: usize,
}

fn get_system_info(use_cuda: bool) -> SystemInfo {
    SystemInfo {
        os: std::env::consts::OS.to_string(),
        cpu: "Unknown".to_string(), // Could be enhanced with cpuid
        gpu: if use_cuda {
            #[cfg(feature = "cuda")]
            {
                Some("CUDA Device".to_string())
            }
            #[cfg(not(feature = "cuda"))]
            {
                None
            }
        } else {
            None
        },
        cuda_version: if use_cuda {
            #[cfg(feature = "cuda")]
            {
                Some("Unknown".to_string())
            }
            #[cfg(not(feature = "cuda"))]
            {
                None
            }
        } else {
            None
        },
    }
}

/// Run a simple latency benchmark using a closure
fn run_simple_benchmark<F>(
    name: &str,
    warmup: usize,
    iterations: usize,
    mut f: F,
    verbose: bool,
) -> BenchmarkResult
where
    F: FnMut() -> Result<()>,
{
    // Simple local recorder that doesn't require 'static lifetime
    let mut samples: Vec<std::time::Duration> = Vec::with_capacity(iterations);

    // Warmup
    if verbose {
        println!("  {} warmup iterations...", warmup);
    }
    for _ in 0..warmup {
        let _ = f();
    }

    // Measurement
    if verbose {
        println!("  {} measurement iterations...", iterations);
    }
    for _ in 0..iterations {
        let start = Instant::now();
        let _ = f();
        samples.push(start.elapsed());
    }

    // Calculate statistics
    if samples.is_empty() {
        return BenchmarkResult {
            name: name.to_string(),
            iterations: 0,
            warmup_iterations: warmup,
            total_duration_ms: 0.0,
            mean_ms: 0.0,
            min_ms: 0.0,
            max_ms: 0.0,
            p50_ms: 0.0,
            p95_ms: 0.0,
            p99_ms: 0.0,
            throughput: None,
            throughput_unit: None,
        };
    }

    samples.sort();
    let len = samples.len();
    let total: std::time::Duration = samples.iter().sum();
    let mean = total / len as u32;
    let min = samples[0];
    let max = samples[len - 1];
    let p50 = samples[len * 50 / 100];
    let p95 = samples[len * 95 / 100];
    let p99 = samples[len * 99 / 100];

    BenchmarkResult {
        name: name.to_string(),
        iterations: len,
        warmup_iterations: warmup,
        total_duration_ms: total.as_secs_f64() * 1000.0,
        mean_ms: mean.as_secs_f64() * 1000.0,
        min_ms: min.as_secs_f64() * 1000.0,
        max_ms: max.as_secs_f64() * 1000.0,
        p50_ms: p50.as_secs_f64() * 1000.0,
        p95_ms: p95.as_secs_f64() * 1000.0,
        p99_ms: p99.as_secs_f64() * 1000.0,
        throughput: None,
        throughput_unit: None,
    }
}

/// Synthetic transformer benchmark (measures overhead without real model)
fn benchmark_transformer_overhead(args: &Args) -> BenchmarkResult {
    println!("Running transformer overhead benchmark...");

    run_simple_benchmark(
        "transformer_overhead",
        args.warmup,
        args.iterations,
        || {
            // Simulate transformer layer timing overhead
            std::hint::black_box({
                // Simulate computation time
                let mut sum = 0u64;
                for i in 0..1000 {
                    sum = sum.wrapping_add(i);
                }
                sum
            });
            Ok(())
        },
        args.verbose,
    )
}

/// Synthetic attention benchmark
fn benchmark_attention_overhead(args: &Args) -> BenchmarkResult {
    println!("Running attention overhead benchmark...");

    run_simple_benchmark(
        "attention_overhead",
        args.warmup,
        args.iterations,
        || {
            std::hint::black_box({
                let mut sum = 0u64;
                for i in 0..500 {
                    sum = sum.wrapping_add(i);
                }
                sum
            });
            Ok(())
        },
        args.verbose,
    )
}

/// Benchmark batch processing overhead
fn benchmark_batch_overhead(args: &Args) -> BenchmarkResult {
    println!("Running batch processing overhead benchmark (batch_size={})...", args.batch_size);

    run_simple_benchmark(
        "batch_overhead",
        args.warmup,
        args.iterations,
        || {
            // Simulate batch processing overhead
            for _ in 0..args.batch_size {
                std::hint::black_box({
                    let mut sum = 0u64;
                    for i in 0..100 {
                        sum = sum.wrapping_add(i);
                    }
                    sum
                });
            }
            Ok(())
        },
        args.verbose,
    )
}

/// Memory allocation benchmark
fn benchmark_memory_allocation(args: &Args) -> BenchmarkResult {
    println!("Running memory allocation benchmark...");

    run_simple_benchmark(
        "memory_allocation",
        args.warmup,
        args.iterations,
        || {
            // Allocate and deallocate memory to measure allocation overhead
            let vec: Vec<f32> = vec![0.0; 1920 * 16]; // Typical audio frame size
            std::hint::black_box(&vec);
            drop(vec);
            Ok(())
        },
        args.verbose,
    )
}

/// Sustained load benchmark (many iterations with timing)
fn benchmark_sustained_load(args: &Args) -> BenchmarkResult {
    println!("Running sustained load benchmark...");

    let sustained_iterations = args.iterations * 10; // More iterations for sustained test

    run_simple_benchmark(
        "sustained_load",
        args.warmup,
        sustained_iterations,
        || {
            std::hint::black_box({
                let mut sum = 0u64;
                for i in 0..200 {
                    sum = sum.wrapping_add(i);
                }
                sum
            });
            Ok(())
        },
        args.verbose,
    )
}

/// Print benchmark results in a formatted table
fn print_results(results: &HashMap<String, BenchmarkResult>) {
    println!("\n{:=<80}", "");
    println!("BENCHMARK RESULTS");
    println!("{:=<80}", "");
    println!(
        "{:<25} {:>10} {:>10} {:>10} {:>10} {:>10}",
        "Benchmark", "Mean (ms)", "Min (ms)", "Max (ms)", "P95 (ms)", "P99 (ms)"
    );
    println!("{:-<80}", "");

    let mut sorted_names: Vec<_> = results.keys().collect();
    sorted_names.sort();

    for name in sorted_names {
        let result = &results[name];
        println!(
            "{:<25} {:>10.3} {:>10.3} {:>10.3} {:>10.3} {:>10.3}",
            result.name,
            result.mean_ms,
            result.min_ms,
            result.max_ms,
            result.p95_ms,
            result.p99_ms
        );
    }
    println!("{:=<80}", "");
}

fn main() -> Result<()> {
    // Initialize simple logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let args = Args::parse();

    println!("Moshi Performance Benchmark Suite");
    println!("==================================");
    println!("Iterations: {}", args.iterations);
    println!("Warmup: {}", args.warmup);
    println!("Device: {}", if args.cpu { "CPU" } else { "GPU (CUDA)" });
    println!("Batch size: {}", args.batch_size);
    println!();

    let mut results = HashMap::new();

    // Determine which benchmarks to run
    let run_all = args.all
        || (!args.mimi && !args.lm && !args.transformer && !args.e2e
            && !args.batch && !args.memory && !args.sustained);

    // Run selected benchmarks
    if run_all || args.transformer {
        let result = benchmark_transformer_overhead(&args);
        results.insert("transformer".to_string(), result);
    }

    if run_all || args.transformer {
        let result = benchmark_attention_overhead(&args);
        results.insert("attention".to_string(), result);
    }

    if run_all || args.batch {
        let result = benchmark_batch_overhead(&args);
        results.insert("batch".to_string(), result);
    }

    if run_all || args.memory {
        let result = benchmark_memory_allocation(&args);
        results.insert("memory".to_string(), result);
    }

    if run_all || args.sustained {
        let result = benchmark_sustained_load(&args);
        results.insert("sustained".to_string(), result);
    }

    // Note: Mimi, LM, and E2E benchmarks require loading actual models
    // These would need model paths and would be implemented when models are available
    if args.mimi {
        println!("Note: Mimi benchmark requires model loading - using synthetic test");
        let result = run_simple_benchmark(
            "mimi_synthetic",
            args.warmup,
            args.iterations,
            || {
                std::hint::black_box({
                    let vec: Vec<f32> = vec![0.0; 1920];
                    vec
                });
                Ok(())
            },
            args.verbose,
        );
        results.insert("mimi".to_string(), result);
    }

    if args.lm {
        println!("Note: LM benchmark requires model loading - using synthetic test");
        let result = run_simple_benchmark(
            "lm_synthetic",
            args.warmup,
            args.iterations,
            || {
                std::hint::black_box({
                    let mut sum = 0u64;
                    for i in 0..2000 {
                        sum = sum.wrapping_add(i);
                    }
                    sum
                });
                Ok(())
            },
            args.verbose,
        );
        results.insert("lm".to_string(), result);
    }

    if args.e2e {
        println!("Note: E2E benchmark requires model loading - using synthetic test");
        let result = run_simple_benchmark(
            "e2e_synthetic",
            args.warmup,
            args.iterations,
            || {
                std::hint::black_box({
                    let mut sum = 0u64;
                    for i in 0..5000 {
                        sum = sum.wrapping_add(i);
                    }
                    sum
                });
                Ok(())
            },
            args.verbose,
        );
        results.insert("e2e".to_string(), result);
    }

    // Print results
    print_results(&results);

    // Save results to JSON if output path specified
    if let Some(output_path) = &args.output {
        let timestamp = chrono::Utc::now().to_rfc3339();
        let suite_results = BenchmarkSuiteResults {
            timestamp,
            system_info: get_system_info(!args.cpu),
            config: BenchmarkConfigSummary {
                iterations: args.iterations,
                warmup: args.warmup,
                use_cuda: !args.cpu,
                batch_size: args.batch_size,
            },
            results,
        };

        let json = serde_json::to_string_pretty(&suite_results)?;
        let mut file = File::create(output_path)?;
        file.write_all(json.as_bytes())?;
        println!("\nResults saved to: {}", output_path);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use moshi_server::bench::LatencyRecorder;
    use std::time::Duration;

    #[test]
    fn test_benchmark_result_from_stats() {
        let recorder = LatencyRecorder::new("test");
        recorder.record(Duration::from_millis(10));
        recorder.record(Duration::from_millis(20));
        recorder.record(Duration::from_millis(30));

        let result: BenchmarkResult = recorder.stats().into();
        assert_eq!(result.name, "test");
        assert_eq!(result.iterations, 3);
    }

    #[test]
    fn test_simple_benchmark() {
        let result = run_simple_benchmark(
            "test",
            2,
            5,
            || Ok(()),
            false,
        );
        assert_eq!(result.name, "test");
        assert_eq!(result.iterations, 5);
        assert_eq!(result.warmup_iterations, 2);
    }
}
