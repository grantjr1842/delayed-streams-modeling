//! Criterion benchmarks for KV cache operations.
//!
//! These benchmarks measure the performance of key-value cache operations
//! which are critical for efficient transformer inference.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;

/// Benchmark cache append operations (simulated).
fn bench_cache_append(c: &mut Criterion) {
    let mut group = c.benchmark_group("kv_cache_append");
    group.measurement_time(Duration::from_secs(5));

    for batch_size in [1, 4, 8, 16].iter() {
        for context_len in [256, 512, 1024].iter() {
            let id = format!("batch{}x{}", batch_size, context_len);
            group.throughput(Throughput::Elements(*batch_size as u64));
            group.bench_with_input(
                BenchmarkId::new("config", &id),
                &(*batch_size, *context_len),
                |b, &(batch_size, context_len)| {
                    b.iter(|| {
                        // Simulate cache append operation
                        let mut cache = vec![0u64; batch_size * context_len * 64]; // heads * head_dim
                        for batch_idx in 0..batch_size {
                            for pos in 0..64 {
                                let idx = batch_idx * context_len * 64 + pos;
                                cache[idx] = (batch_idx * pos) as u64;
                            }
                        }
                        black_box(cache)
                    });
                },
            );
        }
    }
    group.finish();
}

/// Benchmark mask generation overhead.
fn bench_mask_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("kv_cache_mask");
    group.measurement_time(Duration::from_secs(3));

    for context_len in [256, 512, 1024, 2048].iter() {
        group.bench_with_input(
            BenchmarkId::new("context", context_len),
            context_len,
            |b, &context_len| {
                b.iter(|| {
                    // Generate causal mask
                    let mut mask = vec![false; context_len * context_len];
                    for i in 0..context_len {
                        for j in 0..=i {
                            mask[i * context_len + j] = true;
                        }
                    }
                    black_box(mask)
                });
            },
        );
    }
    group.finish();
}

/// Benchmark scattered cache index computation.
fn bench_index_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("kv_cache_indices");
    group.measurement_time(Duration::from_secs(3));

    for batch_size in [1, 4, 8, 16].iter() {
        group.bench_with_input(
            BenchmarkId::new("batch", batch_size),
            batch_size,
            |b, &batch_size| {
                let positions: Vec<usize> = (0..batch_size).map(|i| i * 100).collect();
                let batch_mask: Vec<bool> = (0..batch_size).map(|i| i % 2 == 0).collect();

                b.iter(|| {
                    // Compute indices for scattered cache update
                    let mut indices = Vec::with_capacity(batch_size);
                    for (i, (&pos, &active)) in positions.iter().zip(batch_mask.iter()).enumerate() {
                        if active {
                            indices.push((i, pos, pos + 1));
                        }
                    }
                    black_box(indices)
                });
            },
        );
    }
    group.finish();
}

/// Benchmark cache reset operations.
fn bench_cache_reset(c: &mut Criterion) {
    let mut group = c.benchmark_group("kv_cache_reset");
    group.measurement_time(Duration::from_secs(3));

    for cache_size in [1024, 4096, 16384, 65536].iter() {
        group.throughput(Throughput::Bytes((*cache_size * 4) as u64)); // f32 = 4 bytes
        group.bench_with_input(
            BenchmarkId::new("size", cache_size),
            cache_size,
            |b, &cache_size| {
                let mut cache: Vec<f32> = vec![1.0; cache_size];
                b.iter(|| {
                    // Reset cache to zeros
                    for val in cache.iter_mut() {
                        *val = 0.0;
                    }
                    black_box(cache.len())
                });
            },
        );
    }
    group.finish();
}

/// Benchmark position tracking overhead per batch.
fn bench_position_tracking(c: &mut Criterion) {
    let mut group = c.benchmark_group("kv_cache_position");
    group.measurement_time(Duration::from_secs(3));

    for batch_size in [1, 4, 8, 16, 32].iter() {
        group.bench_with_input(
            BenchmarkId::new("batch", batch_size),
            batch_size,
            |b, &batch_size| {
                let mut positions: Vec<usize> = vec![0; batch_size];
                b.iter(|| {
                    // Simulate position advancement with wrapping
                    let context_len = 2048;
                    for pos in positions.iter_mut() {
                        *pos = (*pos + 1) % context_len;
                    }
                    black_box(positions.len())
                });
            },
        );
    }
    group.finish();
}

/// Benchmark key-value retrieval simulation.
fn bench_kv_retrieval(c: &mut Criterion) {
    let mut group = c.benchmark_group("kv_cache_retrieval");
    group.measurement_time(Duration::from_secs(5));

    for context_len in [256, 512, 1024].iter() {
        let num_heads = 8;
        let head_dim = 64;
        let cache_size = context_len * num_heads * head_dim;

        group.throughput(Throughput::Elements(*context_len as u64));
        group.bench_with_input(
            BenchmarkId::new("context", context_len),
            context_len,
            |b, &context_len| {
                // Pre-allocate cache data
                let k_cache: Vec<f32> = (0..cache_size).map(|i| (i as f32) * 0.001).collect();
                let v_cache: Vec<f32> = (0..cache_size).map(|i| (i as f32) * 0.002).collect();

                b.iter(|| {
                    // Simulate K/V retrieval for attention
                    let mut retrieved_k = Vec::with_capacity(context_len * head_dim);
                    let mut retrieved_v = Vec::with_capacity(context_len * head_dim);

                    // First head only for simplicity
                    for pos in 0..context_len {
                        let offset = pos * head_dim;
                        for d in 0..head_dim {
                            retrieved_k.push(k_cache[offset + d]);
                            retrieved_v.push(v_cache[offset + d]);
                        }
                    }
                    black_box((retrieved_k, retrieved_v))
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_cache_append,
    bench_mask_generation,
    bench_index_computation,
    bench_cache_reset,
    bench_position_tracking,
    bench_kv_retrieval,
);

criterion_main!(benches);
