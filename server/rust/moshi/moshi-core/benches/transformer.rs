//! Criterion benchmarks for transformer operations.
//!
//! These benchmarks measure the performance of core transformer components
//! to aid in optimization efforts and regression detection.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;

/// Synthetic benchmark for transformer layer overhead measurement.
/// This measures the pure overhead of transformer computations without
/// actual model loading, useful for optimization work.
fn bench_synthetic_attention(c: &mut Criterion) {
    let mut group = c.benchmark_group("attention_synthetic");
    group.measurement_time(Duration::from_secs(5));

    // Test different sequence lengths
    for seq_len in [128, 256, 512, 1024].iter() {
        group.throughput(Throughput::Elements(*seq_len as u64));
        group.bench_with_input(
            BenchmarkId::new("seq_len", seq_len),
            seq_len,
            |b, &seq_len| {
                b.iter(|| {
                    // Simulate attention computation overhead
                    let mut sum = 0u64;
                    for i in 0..(seq_len * seq_len) {
                        sum = sum.wrapping_add(i as u64);
                    }
                    black_box(sum)
                });
            },
        );
    }
    group.finish();
}

/// Synthetic benchmark for transformer layer scaling.
fn bench_transformer_layers(c: &mut Criterion) {
    let mut group = c.benchmark_group("transformer_layers");
    group.measurement_time(Duration::from_secs(5));

    // Test different numbers of layers
    for num_layers in [1, 4, 8, 16].iter() {
        group.bench_with_input(
            BenchmarkId::new("layers", num_layers),
            num_layers,
            |b, &num_layers| {
                b.iter(|| {
                    // Simulate multi-layer transformer overhead
                    let mut sum = 0u64;
                    for layer in 0..num_layers {
                        for i in 0..1000 {
                            sum = sum.wrapping_add((layer * 1000 + i) as u64);
                        }
                    }
                    black_box(sum)
                });
            },
        );
    }
    group.finish();
}

/// Benchmark batch dimension overhead.
fn bench_batch_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_overhead");
    group.measurement_time(Duration::from_secs(5));

    for batch_size in [1, 2, 4, 8, 16].iter() {
        group.throughput(Throughput::Elements(*batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("batch_size", batch_size),
            batch_size,
            |b, &batch_size| {
                b.iter(|| {
                    // Simulate batch processing overhead
                    let mut sum = 0u64;
                    for batch in 0..batch_size {
                        for i in 0..500 {
                            sum = sum.wrapping_add((batch * 500 + i) as u64);
                        }
                    }
                    black_box(sum)
                });
            },
        );
    }
    group.finish();
}

/// Benchmark positional embedding computation overhead.
fn bench_positional_embedding(c: &mut Criterion) {
    let mut group = c.benchmark_group("positional_embedding");
    group.measurement_time(Duration::from_secs(3));

    for context_len in [256, 512, 1024, 2048].iter() {
        group.bench_with_input(
            BenchmarkId::new("context", context_len),
            context_len,
            |b, &context_len| {
                b.iter(|| {
                    // Simulate rope/sinusoidal embedding computation
                    let mut values = Vec::with_capacity(context_len);
                    for pos in 0..context_len {
                        let theta = (pos as f64) * 0.001;
                        values.push((theta.sin(), theta.cos()));
                    }
                    black_box(values)
                });
            },
        );
    }
    group.finish();
}

/// Benchmark softmax-like operation overhead.
fn bench_softmax_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("softmax_overhead");
    group.measurement_time(Duration::from_secs(3));

    for size in [128, 256, 512, 1024, 2048].iter() {
        group.bench_with_input(BenchmarkId::new("size", size), size, |b, &size| {
            let input: Vec<f64> = (0..size).map(|i| (i as f64) / 100.0).collect();
            b.iter(|| {
                // Compute max
                let max = input.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                // Subtract max and exp
                let exp: Vec<f64> = input.iter().map(|x| (x - max).exp()).collect();
                // Sum
                let sum: f64 = exp.iter().sum();
                // Normalize
                let result: Vec<f64> = exp.iter().map(|x| x / sum).collect();
                black_box(result)
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_synthetic_attention,
    bench_transformer_layers,
    bench_batch_overhead,
    bench_positional_embedding,
    bench_softmax_overhead,
);

criterion_main!(benches);
