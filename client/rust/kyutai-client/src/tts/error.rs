use thiserror::Error;

#[derive(Debug, Error)]
pub enum TtsError {
    #[error("WebSocket error: {0}")]
    Ws(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Message(String),
}

pub type Result<T> = std::result::Result<T, TtsError>;
