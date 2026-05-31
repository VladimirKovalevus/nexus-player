use std::sync::Arc;

use crate::{
    buffers::{chunked::ChunkedBuffer, hls::HlsBuffer},
    detection::StreamType,
    error::{InitializationError, StreamingError},
    multi_downloader::{hls::HlsDownloader, range::RangeDownloader},
    state::types::BufferType,
};
use async_trait::async_trait;

pub mod hls;
pub mod range;
pub mod source;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadDirection {
    Forward,
    Backward,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekPos {
    Milliseconds(u64),
    Bytes(u64),
}

pub struct TaskDownloader {}
#[async_trait]
pub trait TrackDownloader: Send + Sync {
    async fn init(&mut self) -> Result<(), InitializationError>;
    async fn download(&mut self) -> Result<usize, StreamingError>;
    async fn download_at(&mut self, offset: u64) -> Result<usize, StreamingError>;
    fn seek(&mut self, pos: SeekPos);
    fn is_complete(&self) -> bool;
    fn download_direction(&self) -> DownloadDirection;
    fn buffer(&self) -> BufferType;
}

impl TaskDownloader {
    pub fn create(
        stream_type: StreamType,
        bandwidth_bps: u64,
        user_agent: Option<String>,
        waveform_enabled: bool,
        chunk_size: u64,
    ) -> (Box<dyn TrackDownloader>, Arc<BufferType>) {
        match stream_type {
            StreamType::Hls {
                playlist_url,
                playlist,
            } => {
                let buffer = Arc::new(HlsBuffer::new(waveform_enabled));
                let buffer_type = Arc::new(BufferType::Hls(buffer.clone()));
                (
                    Box::new(HlsDownloader::new(
                        playlist_url,
                        buffer,
                        playlist,
                        bandwidth_bps,
                        user_agent,
                    )),
                    buffer_type,
                )
            }
            StreamType::Range { url, size: _ } => {
                let buffer = Arc::new(ChunkedBuffer::with_chunk_size(chunk_size, waveform_enabled));
                let buffer_type = Arc::new(BufferType::Chunked(buffer.clone()));
                (
                    Box::new(RangeDownloader::new(url, buffer, user_agent)),
                    buffer_type,
                )
            }
            _ => todo!(),
        }
    }
}
