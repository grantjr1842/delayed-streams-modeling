//! Benchmarking harness for inference latency measurements.
//!
//! This module provides utilities for measuring and recording inference performance
//! metrics, including CUDA synchronization timing and percentile calculations.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

// ============================================================================
// Configuration
// ============================================================================

/// Enable or disable CUDA synchronization in timing measurements.
/// When enabled, ensures GPU operations complete before recording time.
pub static SYNC_CUDA_FOR_TIMING: AtomicBool = AtomicBool::new(false);

/// Enable or disable detailed event tracking (may impact performance).
pub static ENABLE_EVENT_TRACKING: AtomicBool = AtomicBool::new(true);

// ============================================================================
// Timing Utilities
// ============================================================================

/// A scoped timer that records duration when dropped
pub struct ScopedTimer {
    start: Instant,
    name: &'static str,
    recorder: Option<&'static LatencyRecorder>,
}

impl ScopedTimer {
    /// Create a new scoped timer
    pub fn new(name: &'static str) -> Self {
        Self {
            start: Instant::now(),
            name,
            recorder: None,
        }
    }

    /// Create a scoped timer that records to a LatencyRecorder
    pub fn with_recorder(name: &'static str, recorder: &'static LatencyRecorder) -> Self {
        Self {
            start: Instant::now(),
            name,
            recorder: Some(recorder),
        }
    }

    /// Get elapsed time so far (doesn't stop the timer)
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    /// Manually stop and record the timer, returning the duration
    pub fn stop(self) -> Duration {
        let elapsed = self.start.elapsed();
        if let Some(recorder) = self.recorder {
            recorder.record(elapsed);
        }
        elapsed
    }
}

impl Drop for ScopedTimer {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed();
        if let Some(recorder) = self.recorder {
            recorder.record(elapsed);
        }
        tracing::trace!(name = self.name, elapsed_us = elapsed.as_micros(), "timer");
    }
}

// ============================================================================
// Latency Recording
// ============================================================================

/// Thread-safe latency recorder for collecting timing measurements
pub struct LatencyRecorder {
    name: &'static str,
    samples: Mutex<Vec<Duration>>,
    count: AtomicU64,
    total_ns: AtomicU64,
    min_ns: AtomicU64,
    max_ns: AtomicU64,
}

impl LatencyRecorder {
    /// Create a new latency recorder
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            samples: Mutex::new(Vec::new()),
            count: AtomicU64::new(0),
            total_ns: AtomicU64::new(0),
            min_ns: AtomicU64::new(u64::MAX),
            max_ns: AtomicU64::new(0),
        }
    }

    /// Record a latency sample
    pub fn record(&self, duration: Duration) {
        let ns = duration.as_nanos() as u64;

        self.count.fetch_add(1, Ordering::Relaxed);
        self.total_ns.fetch_add(ns, Ordering::Relaxed);

        // Update min (compare-and-swap loop)
        let mut current_min = self.min_ns.load(Ordering::Relaxed);
        while ns < current_min {
            match self.min_ns.compare_exchange_weak(
                current_min,
                ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(v) => current_min = v,
            }
        }

        // Update max
        let mut current_max = self.max_ns.load(Ordering::Relaxed);
        while ns > current_max {
            match self.max_ns.compare_exchange_weak(
                current_max,
                ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(v) => current_max = v,
            }
        }

        // Store sample for percentile calculation
        if let Ok(mut samples) = self.samples.lock() {
            samples.push(duration);
            // Keep only last 10000 samples to prevent memory growth
            if samples.len() > 10000 {
                samples.drain(0..5000);
            }
        }
    }

    /// Get summary statistics
    pub fn stats(&self) -> LatencyStats {
        let count = self.count.load(Ordering::Relaxed);
        let total_ns = self.total_ns.load(Ordering::Relaxed);
        let min_ns = self.min_ns.load(Ordering::Relaxed);
        let max_ns = self.max_ns.load(Ordering::Relaxed);

        let mean_ns = if count > 0 { total_ns / count } else { 0 };

        // Calculate percentiles from samples
        let (p50, p95, p99) = if let Ok(mut samples) = self.samples.lock() {
            if samples.is_empty() {
                (Duration::ZERO, Duration::ZERO, Duration::ZERO)
            } else {
                samples.sort();
                let len = samples.len();
                let p50 = samples[len * 50 / 100];
                let p95 = samples[len * 95 / 100];
                let p99 = samples[len * 99 / 100];
                (p50, p95, p99)
            }
        } else {
            (Duration::ZERO, Duration::ZERO, Duration::ZERO)
        };

        LatencyStats {
            name: self.name,
            count,
            mean: Duration::from_nanos(mean_ns),
            min: Duration::from_nanos(if min_ns == u64::MAX { 0 } else { min_ns }),
            max: Duration::from_nanos(max_ns),
            p50,
            p95,
            p99,
        }
    }

    /// Reset all statistics
    pub fn reset(&self) {
        self.count.store(0, Ordering::Relaxed);
        self.total_ns.store(0, Ordering::Relaxed);
        self.min_ns.store(u64::MAX, Ordering::Relaxed);
        self.max_ns.store(0, Ordering::Relaxed);
        if let Ok(mut samples) = self.samples.lock() {
            samples.clear();
        }
    }
}

/// Summary statistics for latency measurements
#[derive(Debug, Clone)]
pub struct LatencyStats {
    pub name: &'static str,
    pub count: u64,
    pub mean: Duration,
    pub min: Duration,
    pub max: Duration,
    pub p50: Duration,
    pub p95: Duration,
    pub p99: Duration,
}

impl std::fmt::Display for LatencyStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: count={}, mean={:.2}ms, min={:.2}ms, max={:.2}ms, p50={:.2}ms, p95={:.2}ms, p99={:.2}ms",
            self.name,
            self.count,
            self.mean.as_secs_f64() * 1000.0,
            self.min.as_secs_f64() * 1000.0,
            self.max.as_secs_f64() * 1000.0,
            self.p50.as_secs_f64() * 1000.0,
            self.p95.as_secs_f64() * 1000.0,
            self.p99.as_secs_f64() * 1000.0,
        )
    }
}

// ============================================================================
// Global Recorders
// ============================================================================

/// Inference step latency (model forward pass)
pub static INFERENCE_LATENCY: LatencyRecorder = LatencyRecorder::new("inference");

/// Audio encoding latency (mimi encode)
pub static ENCODE_LATENCY: LatencyRecorder = LatencyRecorder::new("encode");

/// Audio decoding latency (mimi decode)
pub static DECODE_LATENCY: LatencyRecorder = LatencyRecorder::new("decode");

/// Total request latency (end-to-end)
pub static REQUEST_LATENCY: LatencyRecorder = LatencyRecorder::new("request");

/// Get all latency statistics
pub fn all_stats() -> Vec<LatencyStats> {
    vec![
        INFERENCE_LATENCY.stats(),
        ENCODE_LATENCY.stats(),
        DECODE_LATENCY.stats(),
        REQUEST_LATENCY.stats(),
    ]
}

/// Reset all latency recorders
pub fn reset_all() {
    INFERENCE_LATENCY.reset();
    ENCODE_LATENCY.reset();
    DECODE_LATENCY.reset();
    REQUEST_LATENCY.reset();
}

/// Log all statistics
pub fn log_stats() {
    for stat in all_stats() {
        if stat.count > 0 {
            tracing::info!("{}", stat);
        }
    }
}

// ============================================================================
// Benchmark Configuration
// ============================================================================

/// Configuration for running benchmarks
#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    /// Number of warmup iterations before measuring
    pub warmup_iterations: usize,
    /// Number of iterations to measure
    pub measure_iterations: usize,
    /// Whether to synchronize CUDA after each operation
    pub sync_cuda: bool,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            warmup_iterations: 10,
            measure_iterations: 100,
            sync_cuda: true,
        }
    }
}

/// Benchmark result
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub name: String,
    pub config: BenchmarkConfig,
    pub stats: LatencyStats,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_recorder() {
        let recorder = LatencyRecorder::new("test");
        recorder.record(Duration::from_millis(10));
        recorder.record(Duration::from_millis(20));
        recorder.record(Duration::from_millis(30));

        let stats = recorder.stats();
        assert_eq!(stats.count, 3);
        assert!(stats.mean.as_millis() >= 19 && stats.mean.as_millis() <= 21);
        assert_eq!(stats.min.as_millis(), 10);
        assert_eq!(stats.max.as_millis(), 30);
    }

    #[test]
    fn test_scoped_timer() {
        let recorder = LatencyRecorder::new("test_scoped");
        {
            let _timer = ScopedTimer::new("test");
            std::thread::sleep(Duration::from_millis(10));
        }
        // Timer dropped but not recorded to this recorder
        assert_eq!(recorder.stats().count, 0);
    }
}
