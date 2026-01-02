// Copyright (c) Kyutai, all rights reserved.
// This source code is licensed under the license found in the
// LICENSE file in the root directory of this source tree.

use lazy_static::lazy_static;
use prometheus::{
    histogram_opts, labels, opts, register_counter, register_gauge, register_histogram,
    register_int_counter,
};
use prometheus::{Counter, Gauge, Histogram, IntCounter};

pub mod asr {
    use super::*;
    lazy_static! {
        pub static ref CONNECT: Counter = register_counter!(opts!(
            "asr_connect",
            "Number of connections to the asr.",
            labels! {"handler" => "all",}
        ))
        .unwrap();
        pub static ref MODEL_STEP_DURATION: Histogram = register_histogram!(histogram_opts!(
            "asr_model_step_duration",
            "ASR model step duration distribution.",
            vec![20e-3, 30e-3, 40e-3, 50e-3, 60e-3, 70e-3, 80e-3],
        ))
        .unwrap();
        pub static ref CONNECTION_NUM_STEPS: Histogram = register_histogram!(histogram_opts!(
            "asr_connection_num_steps",
            "ASR model, distribution of number of steps for a connection.",
            vec![2., 25., 125., 250., 500., 750., 1125., 1500., 2250., 3000., 4500.],
        ))
        .unwrap();
        pub static ref OPEN_CHANNELS: Gauge = register_gauge!(opts!(
            "asr_open_channels",
            "Number of open channels (users currently connected).",
            labels! {"handler" => "all",}
        ))
        .unwrap();
    }
}

pub mod stream {
    use super::*;

    fn parse_env_bool(key: &str) -> bool {
        match std::env::var(key) {
            Ok(v) => matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON"),
            Err(_) => false,
        }
    }

    #[inline(always)]
    pub fn enabled() -> bool {
        *ENABLED
    }

    lazy_static! {
        pub static ref ENABLED: bool = parse_env_bool("MOSHI_STREAM_METRICS");
        pub static ref ASR_WS_IN_BYTES: IntCounter =
            register_int_counter!("asr_ws_in_bytes_total", "Total ASR websocket bytes in.")
                .unwrap();
        pub static ref ASR_WS_IN_MESSAGES: IntCounter =
            register_int_counter!("asr_ws_in_messages_total", "Total ASR websocket messages in.")
                .unwrap();
        pub static ref ASR_WS_OUT_BYTES: IntCounter =
            register_int_counter!("asr_ws_out_bytes_total", "Total ASR websocket bytes out.")
                .unwrap();
        pub static ref ASR_WS_OUT_MESSAGES: IntCounter =
            register_int_counter!("asr_ws_out_messages_total", "Total ASR websocket messages out.")
                .unwrap();
        pub static ref LM_WS_IN_BYTES: IntCounter =
            register_int_counter!("lm_ws_in_bytes_total", "Total LM websocket bytes in.").unwrap();
        pub static ref LM_WS_IN_MESSAGES: IntCounter =
            register_int_counter!("lm_ws_in_messages_total", "Total LM websocket messages in.")
                .unwrap();
        pub static ref LM_WS_OUT_BYTES: IntCounter =
            register_int_counter!("lm_ws_out_bytes_total", "Total LM websocket bytes out.")
                .unwrap();
        pub static ref LM_WS_OUT_MESSAGES: IntCounter =
            register_int_counter!("lm_ws_out_messages_total", "Total LM websocket messages out.")
                .unwrap();
        pub static ref TTS_WS_IN_BYTES: IntCounter =
            register_int_counter!("tts_ws_in_bytes_total", "Total TTS websocket bytes in.")
                .unwrap();
        pub static ref TTS_WS_IN_MESSAGES: IntCounter =
            register_int_counter!("tts_ws_in_messages_total", "Total TTS websocket messages in.")
                .unwrap();
        pub static ref TTS_WS_OUT_BYTES: IntCounter =
            register_int_counter!("tts_ws_out_bytes_total", "Total TTS websocket bytes out.")
                .unwrap();
        pub static ref TTS_WS_OUT_MESSAGES: IntCounter =
            register_int_counter!("tts_ws_out_messages_total", "Total TTS websocket messages out.")
                .unwrap();
    }
}

pub mod warmup {
    use super::*;
    lazy_static! {
        pub static ref DURATION: Histogram = register_histogram!(histogram_opts!(
            "warmup_duration_seconds",
            "Warmup duration across modules.",
            vec![0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0],
        ))
        .unwrap();
        pub static ref SUCCESS: Counter = register_counter!(opts!(
            "warmup_success_total",
            "Number of successful warmup executions."
        ))
        .unwrap();
        pub static ref FAILURE: Counter =
            register_counter!(opts!("warmup_failure_total", "Number of failed warmup executions."))
                .unwrap();
        pub static ref SKIPPED: Counter = register_counter!(opts!(
            "warmup_skipped_total",
            "Number of warmup executions skipped (disabled)."
        ))
        .unwrap();
    }
}

pub mod system {
    use super::*;
    lazy_static! {
        pub static ref FREE_VRAM: Gauge =
            register_gauge!(opts!("system_free_vram_bytes", "Free VRAM in bytes.")).unwrap();
        pub static ref USED_VRAM: Gauge =
            register_gauge!(opts!("system_used_vram_bytes", "Used VRAM in bytes.")).unwrap();
        pub static ref TOTAL_VRAM: Gauge =
            register_gauge!(opts!("system_total_vram_bytes", "Total VRAM in bytes.")).unwrap();
        pub static ref GPU_UTILIZATION: Gauge =
            register_gauge!(opts!("system_gpu_utilization_percent", "GPU utilization percentage."))
                .unwrap();
    }
}

pub mod errors {
    use lazy_static::lazy_static;
    use prometheus::{register_int_counter_vec, IntCounterVec};

    lazy_static! {
        /// WebSocket close events by close code.
        /// Labels: code (numeric), reason (category name)
        pub static ref WS_CLOSE_TOTAL: IntCounterVec = register_int_counter_vec!(
            "ws_close_total",
            "Total WebSocket close events by close code.",
            &["code", "reason"]
        )
        .unwrap();

        /// Connection errors by error type.
        /// Labels: error_type (capacity, timeout, protocol, internal)
        pub static ref CONNECTION_ERROR_TOTAL: IntCounterVec = register_int_counter_vec!(
            "connection_error_total",
            "Total connection errors by type.",
            &["error_type", "module"]
        )
        .unwrap();

        /// Authentication errors by error type.
        /// Labels: error_type (invalid_key, expired_token, missing_credentials, jwt_validation_failed)
        pub static ref AUTH_ERROR_TOTAL: IntCounterVec = register_int_counter_vec!(
            "auth_error_total",
            "Total authentication errors by type.",
            &["error_type"]
        )
        .unwrap();
    }

    /// Record a WebSocket close event.
    #[allow(dead_code)]
    pub fn record_ws_close(code: u16, reason: &str) {
        let code_str = code.to_string();
        WS_CLOSE_TOTAL.with_label_values(&[code_str.as_str(), reason]).inc();
    }

    /// Record a connection error.
    pub fn record_connection_error(error_type: &str, module: &str) {
        CONNECTION_ERROR_TOTAL.with_label_values(&[error_type, module]).inc();
    }

    /// Record an authentication error.
    pub fn record_auth_error(error_type: &str) {
        AUTH_ERROR_TOTAL.with_label_values(&[error_type]).inc();
    }
}

/// LM inference performance metrics.
pub mod lm {
    use super::*;
    lazy_static! {
        /// Per-step LM inference latency distribution.
        pub static ref STEP_DURATION: Histogram = register_histogram!(histogram_opts!(
            "lm_step_duration_seconds",
            "LM model step duration distribution.",
            vec![0.005, 0.010, 0.020, 0.030, 0.040, 0.050, 0.075, 0.100, 0.150, 0.200],
        ))
        .unwrap();

        /// Real-time tokens per second throughput.
        pub static ref TOKENS_PER_SECOND: Gauge = register_gauge!(opts!(
            "lm_tokens_per_second",
            "Current LM tokens per second throughput."
        ))
        .unwrap();

        /// Batch utilization ratio (0-1).
        pub static ref BATCH_UTILIZATION: Histogram = register_histogram!(histogram_opts!(
            "lm_batch_utilization",
            "LM batch slot utilization ratio.",
            vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0],
        ))
        .unwrap();

        /// Number of pending requests in queue.
        pub static ref QUEUE_DEPTH: Gauge = register_gauge!(opts!(
            "lm_queue_depth",
            "Number of pending requests in LM queue."
        ))
        .unwrap();

        /// Total inference steps completed.
        pub static ref STEPS_TOTAL: IntCounter =
            register_int_counter!("lm_steps_total", "Total LM inference steps completed.")
                .unwrap();

        /// Number of active LM connections.
        pub static ref ACTIVE_CONNECTIONS: Gauge = register_gauge!(opts!(
            "lm_active_connections",
            "Number of active LM connections."
        ))
        .unwrap();
    }

    /// Record an LM step with its duration.
    #[allow(dead_code)]
    pub fn record_step(duration_secs: f64) {
        STEP_DURATION.observe(duration_secs);
        STEPS_TOTAL.inc();
    }
}

/// Mimi encoder/decoder performance metrics.
pub mod mimi {
    use super::*;
    lazy_static! {
        /// Audio encoding latency distribution.
        pub static ref ENCODE_DURATION: Histogram = register_histogram!(histogram_opts!(
            "mimi_encode_duration_seconds",
            "Mimi audio encoding latency distribution.",
            vec![0.001, 0.002, 0.005, 0.010, 0.020, 0.030, 0.050, 0.075, 0.100],
        ))
        .unwrap();

        /// Audio decoding latency distribution.
        pub static ref DECODE_DURATION: Histogram = register_histogram!(histogram_opts!(
            "mimi_decode_duration_seconds",
            "Mimi audio decoding latency distribution.",
            vec![0.001, 0.002, 0.005, 0.010, 0.020, 0.030, 0.050, 0.075, 0.100],
        ))
        .unwrap();

        /// Total frames encoded.
        pub static ref FRAMES_ENCODED: IntCounter =
            register_int_counter!("mimi_frames_encoded_total", "Total audio frames encoded.")
                .unwrap();

        /// Total frames decoded.
        pub static ref FRAMES_DECODED: IntCounter =
            register_int_counter!("mimi_frames_decoded_total", "Total audio frames decoded.")
                .unwrap();

        /// Encode step latency (for batched operations).
        pub static ref BATCH_ENCODE_DURATION: Histogram = register_histogram!(histogram_opts!(
            "mimi_batch_encode_duration_seconds",
            "Mimi batch encoding latency distribution.",
            vec![0.005, 0.010, 0.020, 0.030, 0.050, 0.075, 0.100, 0.150],
        ))
        .unwrap();

        /// Decode step latency (for batched operations).
        pub static ref BATCH_DECODE_DURATION: Histogram = register_histogram!(histogram_opts!(
            "mimi_batch_decode_duration_seconds",
            "Mimi batch decoding latency distribution.",
            vec![0.005, 0.010, 0.020, 0.030, 0.050, 0.075, 0.100, 0.150],
        ))
        .unwrap();
    }

    /// Record an encode operation with its duration.
    #[allow(dead_code)]
    pub fn record_encode(duration_secs: f64, frame_count: u64) {
        ENCODE_DURATION.observe(duration_secs);
        FRAMES_ENCODED.inc_by(frame_count);
    }

    /// Record a decode operation with its duration.
    #[allow(dead_code)]
    pub fn record_decode(duration_secs: f64, frame_count: u64) {
        DECODE_DURATION.observe(duration_secs);
        FRAMES_DECODED.inc_by(frame_count);
    }
}

/// TTS synthesis performance metrics.
pub mod tts {
    use super::*;
    lazy_static! {
        /// Full TTS synthesis latency distribution.
        pub static ref SYNTHESIS_DURATION: Histogram = register_histogram!(histogram_opts!(
            "tts_synthesis_duration_seconds",
            "TTS synthesis latency distribution.",
            vec![0.05, 0.1, 0.2, 0.3, 0.5, 0.75, 1.0, 1.5, 2.0, 3.0, 5.0],
        ))
        .unwrap();

        /// Total audio seconds generated.
        pub static ref AUDIO_DURATION: Counter = register_counter!(opts!(
            "tts_audio_duration_seconds_total",
            "Total audio seconds generated by TTS."
        ))
        .unwrap();

        /// Real-time factor (audio_time / wall_time). Values < 1 mean faster than real-time.
        pub static ref REALTIME_FACTOR: Gauge = register_gauge!(opts!(
            "tts_realtime_factor",
            "TTS real-time factor (audio_time / wall_time)."
        ))
        .unwrap();

        /// Number of active TTS synthesis requests.
        pub static ref ACTIVE_REQUESTS: Gauge = register_gauge!(opts!(
            "tts_active_requests",
            "Number of active TTS synthesis requests."
        ))
        .unwrap();

        /// Total synthesis requests completed.
        pub static ref REQUESTS_TOTAL: IntCounter =
            register_int_counter!("tts_requests_total", "Total TTS synthesis requests completed.")
                .unwrap();

        /// Vocoder (HiFi-GAN/HiFT) latency distribution.
        pub static ref VOCODER_DURATION: Histogram = register_histogram!(histogram_opts!(
            "tts_vocoder_duration_seconds",
            "TTS vocoder latency distribution.",
            vec![0.01, 0.02, 0.05, 0.1, 0.2, 0.3, 0.5, 0.75, 1.0],
        ))
        .unwrap();
    }

    /// Record a TTS synthesis with its duration and audio length.
    #[allow(dead_code)]
    pub fn record_synthesis(wall_time_secs: f64, audio_duration_secs: f64) {
        SYNTHESIS_DURATION.observe(wall_time_secs);
        AUDIO_DURATION.inc_by(audio_duration_secs);
        if wall_time_secs > 0.0 {
            REALTIME_FACTOR.set(audio_duration_secs / wall_time_secs);
        }
        REQUESTS_TOTAL.inc();
    }
}

/// Memory and allocation performance metrics.
pub mod memory {
    use super::*;
    lazy_static! {
        /// Total tensor allocations count.
        pub static ref TENSOR_ALLOCATIONS: IntCounter =
            register_int_counter!("memory_tensor_allocations_total", "Total tensor allocations.")
                .unwrap();

        /// Peak VRAM usage in bytes.
        pub static ref PEAK_VRAM_BYTES: Gauge = register_gauge!(opts!(
            "memory_peak_vram_bytes",
            "Peak VRAM usage in bytes."
        ))
        .unwrap();

        /// Current VRAM usage in bytes.
        pub static ref CURRENT_VRAM_BYTES: Gauge = register_gauge!(opts!(
            "memory_current_vram_bytes",
            "Current VRAM usage in bytes."
        ))
        .unwrap();

        /// Total bytes allocated on GPU.
        pub static ref GPU_BYTES_ALLOCATED: Counter = register_counter!(opts!(
            "memory_gpu_bytes_allocated_total",
            "Total bytes allocated on GPU."
        ))
        .unwrap();

        /// Total bytes deallocated on GPU.
        pub static ref GPU_BYTES_DEALLOCATED: Counter = register_counter!(opts!(
            "memory_gpu_bytes_deallocated_total",
            "Total bytes deallocated on GPU."
        ))
        .unwrap();
    }

    /// Update VRAM usage metrics.
    #[allow(dead_code)]
    pub fn update_vram(current_bytes: f64, peak_bytes: f64) {
        CURRENT_VRAM_BYTES.set(current_bytes);
        let current_peak = PEAK_VRAM_BYTES.get();
        if peak_bytes > current_peak {
            PEAK_VRAM_BYTES.set(peak_bytes);
        }
    }
}

/// Pipeline efficiency metrics.
pub mod pipeline {
    use super::*;
    lazy_static! {
        /// Total pipeline stall events.
        pub static ref STALLS: IntCounter =
            register_int_counter!("pipeline_stalls_total", "Total pipeline stall events.")
                .unwrap();

        /// Mimi/LM overlap efficiency ratio (0-1).
        pub static ref OVERLAP_EFFICIENCY: Histogram = register_histogram!(histogram_opts!(
            "pipeline_overlap_efficiency",
            "Mimi/LM overlap efficiency ratio.",
            vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0],
        ))
        .unwrap();

        /// Pre-processing stage duration.
        pub static ref PREPROCESS_DURATION: Histogram = register_histogram!(histogram_opts!(
            "pipeline_preprocess_duration_seconds",
            "Pre-processing stage duration distribution.",
            vec![0.001, 0.002, 0.005, 0.010, 0.020, 0.030, 0.050],
        ))
        .unwrap();

        /// Post-processing stage duration.
        pub static ref POSTPROCESS_DURATION: Histogram = register_histogram!(histogram_opts!(
            "pipeline_postprocess_duration_seconds",
            "Post-processing stage duration distribution.",
            vec![0.001, 0.002, 0.005, 0.010, 0.020, 0.030, 0.050],
        ))
        .unwrap();

        /// Channel queue depth (pending audio frames).
        pub static ref CHANNEL_QUEUE_DEPTH: Gauge = register_gauge!(opts!(
            "pipeline_channel_queue_depth",
            "Average channel queue depth (pending audio frames)."
        ))
        .unwrap();

        /// Batch processing duration.
        pub static ref BATCH_DURATION: Histogram = register_histogram!(histogram_opts!(
            "pipeline_batch_duration_seconds",
            "Full batch processing duration distribution.",
            vec![0.010, 0.020, 0.030, 0.040, 0.050, 0.060, 0.080, 0.100, 0.150],
        ))
        .unwrap();
    }

    /// Record a pipeline stall event.
    #[allow(dead_code)]
    pub fn record_stall() {
        STALLS.inc();
    }

    /// Record overlap efficiency for a batch.
    #[allow(dead_code)]
    pub fn record_overlap(efficiency: f64) {
        OVERLAP_EFFICIENCY.observe(efficiency);
    }
}
