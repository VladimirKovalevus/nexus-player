use crate::detection::{detect_stream_type, StreamType};
use crate::downloader::hls::HlsDownloader;
use crate::downloader::range::RangeDownloader;
use crate::error::StreamingError;
use crate::{buffers::chunked::ChunkedBuffer, downloader::DownloaderHandle};
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::Arc;

pub trait MediaSource: Read + Seek + Send + Sync {
    fn is_seekable(&self) -> bool;

    fn byte_len(&self) -> Option<u64>;
}

pub struct StreamingSource {
    position: u64,
    buffer: Arc<ChunkedBuffer>,
    downloader: Arc<DownloaderHandle>,
    url: String,
}
impl MediaSource for StreamingSource {
    fn is_seekable(&self) -> bool {
        self.buffer.total_size().is_some()
    }
    fn byte_len(&self) -> Option<u64> {
        self.buffer.total_size()
    }
}

pub async fn get_source(url: String) -> Result<StreamingSource, StreamingError> {
    let stream_type = detect_stream_type(&url, None).await?;
    let source = StreamingSource::new(stream_type);
    Ok(source)
}

impl StreamingSource {
    pub fn new(stream_type: StreamType) -> Self {
        let (downloader, buffer, url) = match stream_type {
            StreamType::Range { url, size } => {
                let buffer = Arc::new(ChunkedBuffer::new(false));
                if let Some(sz) = size {
                    buffer.set_total_size(sz);
                }
                let range = Arc::new(RangeDownloader::new(url.clone(), buffer.clone()));
                (Arc::new(DownloaderHandle::Range(range)), buffer, url)
            }
            StreamType::Hls {
                playlist_url,
                playlist: _,
            } => {
                let buffer = Arc::new(ChunkedBuffer::new(false));
                let range = Arc::new(HlsDownloader::new(playlist_url.clone(), buffer.clone(), 0));
                (Arc::new(DownloaderHandle::Hls(range)), buffer, playlist_url)
            }
            StreamType::Sequential { url } => {
                let buffer = Arc::new(ChunkedBuffer::new(false));
                let range = Arc::new(RangeDownloader::new(url.clone(), buffer.clone()));
                (Arc::new(DownloaderHandle::Range(range)), buffer, url)
            }
        };

        downloader.start();

        Self {
            position: 0,
            buffer,
            downloader,
            url,
        }
    }

    pub fn total_size(&self) -> Option<u64> {
        self.buffer.total_size()
    }
    pub fn save_to_file<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), StreamingError> {
        self.buffer
            .save_to_file(path.as_ref().to_path_buf())
            .map_err(|e| StreamingError::Io(e.to_string()))
    }

    pub async fn download_to_file<P: AsRef<std::path::Path>>(
        &self,
        path: P,
    ) -> Result<(), StreamingError> {
        use tokio::time::{sleep, Duration};

        self.downloader.start();

        loop {
            if self.buffer.is_complete() {
                break;
            }

            sleep(Duration::from_millis(100)).await;
        }

        self.save_to_file(path)?;

        Ok(())
    }
}
impl Read for StreamingSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(total) = self.buffer.total_size() {
            if self.position >= total {
                return Ok(0);
            }
        }

        let chunk_index = self.buffer.chunk_index_for_offset(self.position);

        if !self.buffer.has_chunk(chunk_index) {
            return Err(io::Error::new(io::ErrorKind::WouldBlock, "not ready"));
        }

        match self.buffer.read_at(self.position, buf) {
            Some(0) => Ok(0),
            Some(n) => {
                self.position += n as u64;
                Ok(n)
            }
            None => Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Data not available",
            )),
        }
    }
}
impl Seek for StreamingSource {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(offset) => offset,

            SeekFrom::End(offset) => {
                let total = self.buffer.total_size().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::Unsupported,
                        "File size unknown, cannot seek from end",
                    )
                })?;

                if offset >= 0 {
                    total.saturating_add(offset as u64)
                } else {
                    total.saturating_sub((-offset) as u64)
                }
            }

            SeekFrom::Current(offset) => {
                if offset >= 0 {
                    self.position.saturating_add(offset as u64)
                } else {
                    self.position.saturating_sub((-offset) as u64)
                }
            }
        };
        self.position = new_pos;

        let chunk_index = self.buffer.chunk_index_for_offset(new_pos);
        if !self.buffer.has_chunk(chunk_index) {
            self.downloader.seek_to(new_pos);
        }

        Ok(self.position)
    }
}
impl Clone for StreamingSource {
    fn clone(&self) -> Self {
        Self {
            position: 0,

            buffer: Arc::clone(&self.buffer),
            downloader: Arc::clone(&self.downloader),

            url: self.url.clone(),
        }
    }
}
impl std::fmt::Debug for StreamingSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamingSource")
            .field("url", &self.url)
            .field("position", &self.position)
            .field("total_size", &self.total_size())
            .finish()
    }
}
