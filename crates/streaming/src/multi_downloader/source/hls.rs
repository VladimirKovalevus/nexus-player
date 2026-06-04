use std::io::{self};
use std::sync::Arc;

use crate::buffers::hls::HlsBuffer;
use crate::error::StreamingError;
use crate::state::types::ManagerCommand;

pub struct HlsSource {
    position_ms: u64,
    track_id: u64,
    buffer: Arc<HlsBuffer>,
    sender: Arc<tokio::sync::mpsc::UnboundedSender<ManagerCommand>>,
    read_segment_idx: usize,
    read_segment_off: usize,
}

impl HlsSource {
    pub fn new(
        buffer: Arc<HlsBuffer>,
        track_id: u64,
        sender: Arc<tokio::sync::mpsc::UnboundedSender<ManagerCommand>>,
    ) -> Self {
        Self {
            position_ms: 0,
            buffer,
            track_id,
            sender,
            read_segment_idx: 0,
            read_segment_off: 0,
        }
    }

    pub fn total_duration_ms(&self) -> u64 {
        let durations = self.buffer.durations();
        durations.iter().map(|d| (d * 1000.0) as u64).sum()
    }

    pub fn is_continuous(&self) -> bool {
        if let Some(container) = self.buffer.container() {
            return container == "opus" || container == "ogg";
        }
        false
    }

    pub fn container(&self) -> Option<String> {
        self.buffer.container()
    }

    pub fn estimate_total_size(&self) -> Option<u64> {
        self.buffer.total_size()
    }

    pub fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.buffer.segment_count() < self.read_segment_idx {
            return Ok(0);
        }

        // Try to get the current segment data
        match self.buffer.get_segment(self.read_segment_idx) {
            Some((data, _)) => {
                if self.read_segment_off >= data.len() {
                    // We exhausted this segment, move to next
                    self.read_segment_idx += 1;
                    self.read_segment_off = 0;
                    // Tail call / loop to try next segment immediately
                    return self.read(buf);
                }

                let available = data.len() - self.read_segment_off;
                let to_copy = std::cmp::min(available, buf.len());

                buf[..to_copy]
                    .copy_from_slice(&data[self.read_segment_off..self.read_segment_off + to_copy]);
                self.read_segment_off += to_copy;

                // println!("HlsSource read: {} bytes from seg {} (off {})", to_copy, self.read_segment_idx, self.read_segment_off);

                Ok(to_copy)
            }
            None => {
                if self.buffer.is_complete() {
                    let durations = self.buffer.durations();
                    // Assuming get_segment(idx) uses logical index 0..durations.len() (plus init segment logic internal)
                    // HlsBuffer logic seems to imply segment count matches durations.len() (or +1 with init).
                    // If buffer is complete and we can't get this segment, we must be at EOF.
                    // But buffer might have `None` inside if failed? Assuming sequential download for now or retry logic elsewhere.
                    // If it's complete, any None usually means hole or EOF.
                    // Let's assume EOF if index is high enough.

                    if self.read_segment_idx >= durations.len() {
                        return Ok(0);
                    }
                }

                // Not ready yet
                Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "Segment not ready",
                ))
            }
        }
    }

    // Kept for compatibility but deprecated for this use case
    pub fn read_at(&self, offset: u64, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.buffer.read_at(offset, buf) {
            Some(n) => Ok(n),
            None => {
                if self.buffer.is_complete() {
                    return Ok(0);
                }
                Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "Segment not ready",
                ))
            }
        }
    }

    pub fn seek_to_byte_approx(
        &mut self,
        byte_offset: u64,
    ) -> Result<u64, Box<dyn std::error::Error>> {
        let total_size = self.estimate_total_size().unwrap_or(0);

        if total_size == 0 {
            return Ok(0);
        }

        // Use the buffer's bitrate for accurate mapping
        let total_duration_sec = self.total_duration_ms() as f64 / 1000.0;
        let bitrate_bps = if total_duration_sec > 0.0 {
            (total_size * 8) as f64 / total_duration_sec
        } else {
            160_000.0
        };

        let durations = self.buffer.durations();
        let mut accumulated_bytes = 0u64;
        let mut target_idx = 0;
        let mut local_offset = 0usize;

        for (i, &duration) in durations.iter().enumerate() {
            let seg_size = (duration * bitrate_bps / 8.0) as u64;
            let next_boundary = accumulated_bytes + seg_size;

            if byte_offset < next_boundary {
                target_idx = i;
                local_offset = (byte_offset - accumulated_bytes) as usize;
                break;
            }

            accumulated_bytes = next_boundary;
            target_idx = i + 1;
        }

        // If we went past the end (e.g. slight calculation error or EOF), clamp to last
        if target_idx >= durations.len() && !durations.is_empty() {
            target_idx = durations.len() - 1;
            local_offset = 0;
        }

        println!(
            "HlsSource seek_to_byte_approx: byte_off={} -> seg_idx={}, local_off={}",
            byte_offset, target_idx, local_offset
        );

        // Update read state ONLY. Do NOT trigger download seek here.
        // Download seek will be triggered by `read()` when it actually needs data.
        if target_idx > self.buffer.segment_count() {
            return Ok(0);
        }
        self.read_segment_idx = target_idx;
        self.read_segment_off = local_offset;

        Ok(byte_offset)
    }

    pub fn seek(&self, offset_ms: u64) -> Result<(), Box<dyn std::error::Error>> {
        let seconds = offset_ms as f64 / 1000.0;
        let durations = self.buffer.durations();

        if durations.is_empty() {
            return Ok(());
        }

        let mut accumulated_duration = 0.0;
        let mut segment_index = 0;

        for (i, duration) in durations.iter().enumerate() {
            accumulated_duration += duration;
            if seconds < accumulated_duration {
                segment_index = i;
                break;
            }
            segment_index = i + 1;
        }

        if !self.buffer.has_segment(segment_index) {
            // self.sender.send(ManagerCommand::Seek(
            //     self.track_id,
            //     SeekPos::Milliseconds(offset_ms),
            // ))?;
        }

        Ok(())
    }

    pub fn durations(&self) -> Vec<f64> {
        self.buffer.durations()
    }

    pub fn get_buffered_ranges(&self) -> Vec<(f64, f64)> {
        self.buffer.get_buffered_ranges()
    }

    pub fn get_segment(&self, index: usize) -> Option<(Vec<u8>, f64)> {
        self.buffer.get_segment(index)
    }

    pub fn save_to_file<P: AsRef<std::path::Path>>(&self, path: P) -> std::io::Result<()> {
        self.buffer.save_to_file(path.as_ref().to_path_buf())
    }

    pub async fn prepare(&self) -> Result<(), StreamingError> {
        let timeout = std::time::Duration::from_secs(30);
        let start = std::time::Instant::now();

        loop {
            if self.is_continuous()
                && self.buffer.has_segment(2)
                && self.buffer.has_segment(self.buffer.segment_count() - 2)
                || (!self.is_continuous() && self.buffer.has_segment(0))
            {
                return Ok(());
            }
            if start.elapsed() > timeout {
                let _ = self.sender.send(ManagerCommand::Cancel(self.track_id));
                return Err(StreamingError::Timeout(
                    "First segment not ready within 30 seconds".to_string(),
                ));
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }

    pub fn get_waveforms(&self) -> Vec<crate::state::types::Waveform> {
        self.buffer.get_waveforms()
    }
}

impl Clone for HlsSource {
    fn clone(&self) -> Self {
        Self {
            position_ms: 0,
            buffer: Arc::clone(&self.buffer),
            track_id: self.track_id,
            sender: Arc::clone(&self.sender),
            read_segment_idx: self.read_segment_idx,
            read_segment_off: self.read_segment_off,
        }
    }
}

impl std::fmt::Debug for HlsSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HlsSource")
            .field("position_ms", &self.position_ms)
            .field("total_duration_ms", &self.total_duration_ms())
            .field("read_segment_idx", &self.read_segment_idx)
            .field("read_segment_off", &self.read_segment_off)
            .finish()
    }
}
