use thiserror::Error;

pub type Result<T> = anyhow::Result<T>;

#[derive(Error, Debug)]
pub enum AudioError {
    #[error("Failed to decode audio: {0}")]
    DecodingError(String),

    #[error("Failed to initialize audio output: {0}")]
    OutputError(String),

    #[error("Unsupported audio format")]
    UnsupportedFormat,

    #[error("Failed to build audio stream: {0}")]
    StreamBuildError(String),

    #[error("Resampling error: {0}")]
    ResamplingError(String),

    #[error("Audio stream unavailable")]
    StreamUnavailable,

    #[error("Data not available yet")]
    InProgress,

    #[error("Requested part is outside the buffer")]
    OutsideOfBuffer,

    #[error("Failed to read audio file: {0}")]
    IoError(#[from] std::io::Error),
}
