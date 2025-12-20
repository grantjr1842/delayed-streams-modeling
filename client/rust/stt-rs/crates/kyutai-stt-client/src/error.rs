use thiserror::Error;

pub type Result<T> = std::result::Result<T, SttError>;

#[derive(Debug, Error)]
pub enum SttError {
    #[error("{0}")]
    Message(String),

    #[error("unimplemented: {0}")]
    Unimplemented(&'static str),
}
