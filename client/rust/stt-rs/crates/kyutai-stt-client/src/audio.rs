#[derive(Clone, Debug)]
pub struct AudioChunk {
    pub samples: Vec<f32>,
    pub sample_rate_hz: u32,
}

pub mod level;
pub use level::{AudioLevel, LevelMeter};

#[cfg(feature = "mic")]
pub mod mic;

#[cfg(feature = "mic")]
pub use mic::MicCapture;
