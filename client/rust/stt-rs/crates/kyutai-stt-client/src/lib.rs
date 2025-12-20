mod error;

pub mod audio;
pub mod protocol;
pub mod transcript;
pub mod ws;

mod types;

pub use error::{Result, SttError};
pub use types::{SttEvent, Utterance, WordTiming};
pub use ws::{SttClientBuilder, SttSender, SttSession};
