use super::HlsBuffer;
use crate::buffers::sync::lock_recover;
use std::io::{Read, Seek, SeekFrom};
use std::sync::Arc;

use symphonia::core::io::MediaSource;

pub struct HlsSequentialReader {
    buffer: Arc<HlsBuffer>,
    seg_idx: usize,
    off: usize,
}

impl HlsSequentialReader {
    pub fn new(buffer: Arc<HlsBuffer>) -> Self {
        Self {
            buffer,
            seg_idx: 0,
            off: 0,
        }
    }
}

impl Read for HlsSequentialReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        let inner = lock_recover(&self.buffer.inner);

        // Skip init segment if reading sequentially for OGG (usually OGG HLS doesn't have one,
        // or it's part of the first segment).
        // If we need it, we should handle it, but for OGG waveform we usually want the raw stream.
        let actual_idx = if inner.has_init_segment && self.seg_idx == 0 && self.off == 0 {
            // If we are at the very beginning and there's an init segment,
            // we might want to skip it if it was already processed or handled.
            // But let's assume we want EVERYTHING for the decoder.
            self.seg_idx
        } else {
            self.seg_idx
        };

        if let Some(Some(segment)) = inner.segments.get(actual_idx) {
            if self.off >= segment.data.len() {
                self.seg_idx += 1;
                self.off = 0;
                drop(inner);
                return self.read(buf);
            }

            let available = segment.data.len() - self.off;
            let to_copy = std::cmp::min(available, buf.len());
            buf[..to_copy].copy_from_slice(&segment.data[self.off..self.off + to_copy]);
            self.off += to_copy;
            Ok(to_copy)
        } else {
            // Segment not ready or EOF
            Ok(0)
        }
    }
}

impl Seek for HlsSequentialReader {
    fn seek(&mut self, _pos: SeekFrom) -> std::io::Result<u64> {
        // Sequential reader for waveform doesn't really need seeking,
        // but symphonia might call it. We can return error or current pos.
        Err(std::io::Error::other(
            "Seek not implemented for HlsSequentialReader",
        ))
    }
}

impl MediaSource for HlsSequentialReader {
    fn is_seekable(&self) -> bool {
        false
    }

    fn byte_len(&self) -> Option<u64> {
        None
    }
}
