use serde::{Deserialize, Serialize};

/// Incoming message types (received from server)
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum InMsg {
    Audio {
        pcm: Vec<f32>,
    },
    Text {
        text: String,
        start_s: f64,
        stop_s: f64,
    },
    OggOpus {
        data: Vec<u8>,
    },
    Error {
        message: String,
    },
    Ready,
}

/// Outgoing message types (not explicitly used in tts-rs yet, but good for symmetry)
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum OutMsg {
    Text { text: String },
}
