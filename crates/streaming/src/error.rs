use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum StreamingError {
    #[error("HTTP error: {0}")]
    Http(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("Download failed: {0}")]
    DownloadFailed(String),

    #[error("HLS parse failed: {0}")]
    HlsParseError(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("File size unknown")]
    UnknownSize,

    #[error("Image decode failed: {0}")]
    ImageDecode(String),
}
#[derive(Error, Debug, Clone)]
pub enum InitializationError {
    #[error("Init err:{0}")]
    Error(String),
}
impl From<StreamingError> for std::io::Error {
    fn from(err: StreamingError) -> Self {
        std::io::Error::new(std::io::ErrorKind::Other, err)
    }
}
