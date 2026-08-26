use thiserror::Error;

#[derive(Error, Debug)]
pub enum StreamError {
    #[error("No suitable provider found for the source")]
    NoProviderFound,

    #[error("Unsupported source: {0}")]
    InvalidSource(String),

    #[error("Stream unavailable: {0}")]
    NoStreamAvailable(String),

    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Feature not supported")]
    NotSupported,

    #[error("Metadata parsing error: {0}")]
    MetadataError(String),

    #[error("Authentication error: {0}")]
    AuthenticationError(String),

    #[error("Request timed out")]
    Timeout,

    #[error("Track unavailable: {0}")]
    Unavailable(String),

    #[error("Challenge required for {provider}: {url}")]
    ChallengeRequired { provider: String, url: String },

    #[error("Streaming error: {0}")]
    Other(String),
}

pub type StreamResult<T> = Result<T, StreamError>;
