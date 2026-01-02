mod error;
pub mod protocol;
pub mod ws;

pub use error::{Result, TtsError};
pub use protocol::{InMsg, OutMsg};
pub use ws::{TtsClientBuilder, TtsSession};
