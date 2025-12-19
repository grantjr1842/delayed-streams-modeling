// Copyright (c) Kyutai, all rights reserved.
// This source code is licensed under the license found in the
// LICENSE file in the root directory of this source tree.

use lazy_static::lazy_static;
use prometheus::{
    histogram_opts, labels, opts, register_counter, register_counter_vec, register_gauge,
    register_histogram, register_int_counter,
};
use prometheus::{Counter, CounterVec, Gauge, Histogram, IntCounter};

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
        pub static ref PY_HTTP_WAV_OUT_BYTES: IntCounter = register_int_counter!(
            "py_http_wav_out_bytes_total",
            "Total py-module HTTP WAV bytes out."
        )
        .unwrap();
        pub static ref PY_HTTP_WAV_OUT_CHUNKS: IntCounter = register_int_counter!(
            "py_http_wav_out_chunks_total",
            "Total py-module HTTP WAV chunks out."
        )
        .unwrap();
    }
}

pub mod py {
    use super::*;
    lazy_static! {
        pub static ref CONNECT: Counter = register_counter!(opts!(
            "py_connect",
            "Number of connections to the py-module.",
            labels! {"handler" => "all",}
        ))
        .unwrap();
        pub static ref TOTAL_STEPS: Counter = register_counter!(opts!(
            "py_total_steps",
            "Total number of times the python callback was called.",
            labels! {"handler" => "all",}
        ))
        .unwrap();
        pub static ref ACTIVE_STEPS: Counter = register_counter!(opts!(
            "py_active_steps",
            "Number of times the python callback was called with some active users.",
            labels! {"handler" => "all",}
        ))
        .unwrap();
        pub static ref MISSING_WORDS_STEPS: Counter = register_counter!(opts!(
            "py_missing_words_steps",
            "Number of times the user failed to send words fast enough.",
            labels! {"handler" => "all",}
        ))
        .unwrap();
        pub static ref COULD_HAVE_RUN_STEPS: Counter = register_counter!(opts!(
            "py_could_have_run_steps",
            "Number of times we ran the callback with enough words for a user.",
            labels! {"handler" => "all",}
        ))
        .unwrap();
        pub static ref MODEL_STEP_DURATION: Histogram = register_histogram!(histogram_opts!(
            "py_model_step_duration",
            "py module step duration distribution.",
            vec![10e-3, 15e-3, 20e-3, 30e-3, 40e-3, 50e-3, 80e-3],
        ))
        .unwrap();
        pub static ref CONNECTION_NUM_STEPS: Histogram = register_histogram!(histogram_opts!(
            "py_model_connection_num_steps",
            "py module number of steps with data being generated.",
            vec![2., 25., 62.5, 125., 250., 500., 750.],
        ))
        .unwrap();
        pub static ref OPEN_CHANNELS: Gauge = register_gauge!(opts!(
            "py_open_channels",
            "Number of open channels (users currently connected).",
            labels! {"handler" => "all",}
        ))
        .unwrap();
    }
}

pub mod py_post {
    use super::*;
    lazy_static! {
        pub static ref CONNECT: Counter = register_counter!(opts!(
            "py_post_connect",
            "Number of connections to the py_post module.",
            labels! {"handler" => "all",}
        ))
        .unwrap();
        pub static ref MODEL_DURATION: Histogram = register_histogram!(histogram_opts!(
            "py_post_model_duration",
            "py-post model duration distribution.",
            vec![20e-3, 30e-3, 40e-3, 50e-3, 60e-3, 70e-3, 80e-3],
        ))
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
