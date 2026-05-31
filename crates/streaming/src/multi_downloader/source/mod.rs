pub mod hls;
pub mod streaming;

pub use hls::HlsSource;
pub use streaming::{MediaSource, StreamingSourceV2};

use crate::error::StreamingError;
use crate::state::types::{BufferType, ManagerCommand};
use std::sync::Arc;

pub enum Source {
    Streaming(StreamingSourceV2),
    Hls(HlsSource),
}

impl Source {
    pub fn save_to_file<P: AsRef<std::path::Path>>(&self, path: P) -> std::io::Result<()> {
        match self {
            Source::Streaming(s) => s
                .save_to_file(path)
                .map_err(|e| std::io::Error::other(e.to_string())),
            Source::Hls(h) => h.save_to_file(path),
        }
    }

    pub async fn prepare(&self) -> Result<(), StreamingError> {
        match self {
            Source::Streaming(s) => s.prepare().await,
            Source::Hls(h) => h.prepare().await,
        }
    }

    pub fn from_buffer_type(
        buffer: Arc<BufferType>,
        track_id: u64,
        sender: Arc<tokio::sync::mpsc::UnboundedSender<ManagerCommand>>,
    ) -> Self {
        match &*buffer {
            BufferType::Chunked(chunked) => {
                Source::Streaming(StreamingSourceV2::new(chunked.clone(), track_id, sender))
            }
            BufferType::Hls(hls) => Source::Hls(HlsSource::new(hls.clone(), track_id, sender)),
        }
    }
}
