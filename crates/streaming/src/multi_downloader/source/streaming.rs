use std::io::{self, Read, Seek, SeekFrom};
use std::sync::Arc;

use crate::buffers::{chunked::ChunkedBuffer, Buffer};
use crate::error::StreamingError;
use crate::multi_downloader::SeekPos;
use crate::state::types::ManagerCommand;

pub trait MediaSource: Read + Seek + Send + Sync {
    fn is_seekable(&self) -> bool;

    fn byte_len(&self) -> Option<u64>;
}

pub struct StreamingSourceV2 {
    position: u64,
    track_id: u64,
    buffer: Arc<ChunkedBuffer>,
    sender: Arc<tokio::sync::mpsc::UnboundedSender<ManagerCommand>>,
}
impl MediaSource for StreamingSourceV2 {
    fn is_seekable(&self) -> bool {
        self.buffer.total_size().is_some()
    }
    fn byte_len(&self) -> Option<u64> {
        self.buffer.total_size()
    }
}

impl StreamingSourceV2 {
    pub fn new(
        buffer: Arc<ChunkedBuffer>,
        track_id: u64,
        sender: Arc<tokio::sync::mpsc::UnboundedSender<ManagerCommand>>,
    ) -> Self {
        Self {
            position: 0,
            buffer,
            track_id,
            sender,
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

    pub async fn prepare(&self) -> Result<(), StreamingError> {
        let timeout = std::time::Duration::from_secs(30);
        let start = std::time::Instant::now();

        loop {
            if self.buffer.has_chunk(0) {
                return Ok(());
            }
            if start.elapsed() > timeout {
                let _ = self.sender.send(ManagerCommand::Cancel(self.track_id));
                return Err(StreamingError::Timeout(
                    "First chunk not ready within 30 seconds".to_string(),
                ));
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }

    pub fn get_waveforms(&self) -> Vec<crate::state::types::Waveform> {
        self.buffer.get_waveforms()
    }
}
impl Read for StreamingSourceV2 {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(total) = self.buffer.total_size() {
            if self.position >= total {
                return Ok(0);
            }
        }

        if !self.buffer.has_offset(self.position) {
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
impl Seek for StreamingSourceV2 {
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

        if !self.buffer.has_offset(new_pos) {
            let _ = self
                .sender
                .send(ManagerCommand::Seek(self.track_id, SeekPos::Bytes(new_pos)));
        }

        Ok(self.position)
    }
}
impl Clone for StreamingSourceV2 {
    fn clone(&self) -> Self {
        Self {
            position: 0,
            buffer: Arc::clone(&self.buffer),
            track_id: self.track_id,
            sender: Arc::clone(&self.sender),
        }
    }
}
impl std::fmt::Debug for StreamingSourceV2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamingSource")
            .field("position", &self.position)
            .field("total_size", &self.total_size())
            .finish()
    }
}
