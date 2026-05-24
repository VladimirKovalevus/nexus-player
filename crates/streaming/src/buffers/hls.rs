use std::fs::{self, File};
use std::io::{Cursor, Read, Write, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use lofty::{prelude::*, probe::Probe};
use symphonia::core::io::MediaSource;

pub struct HlsBufferReader {
    buffer: Arc<HlsBuffer>,
    position: u64,
}

impl HlsBufferReader {
    pub fn new(buffer: Arc<HlsBuffer>) -> Self {
        Self {
            buffer,
            position: 0,
        }
    }
}

impl Read for HlsBufferReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.buffer.read_at(self.position, buf) {
            Some(n) => {
                self.position += n as u64;
                Ok(n)
            }
            None => Ok(0),
        }
    }
}

impl Seek for HlsBufferReader {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let size = self
            .buffer
            .total_size()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "Unknown size"))?;

        let new_pos = match pos {
            SeekFrom::Start(p) => p,
            SeekFrom::End(p) => {
                if p >= 0 {
                    size + p as u64
                } else {
                    size.saturating_sub(p.abs() as u64)
                }
            }
            SeekFrom::Current(p) => {
                if p >= 0 {
                    self.position + p as u64
                } else {
                    self.position.saturating_sub(p.abs() as u64)
                }
            }
        };

        self.position = new_pos;
        Ok(new_pos)
    }
}

impl MediaSource for HlsBufferReader {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        self.buffer.total_size()
    }
}

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

        let inner = self.buffer.inner.lock().unwrap();

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
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
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

#[derive(Debug, Clone)]
struct Segment {
    data: Vec<u8>,
    waveform: Option<crate::state::types::Waveform>,
}

struct BufferInner {
    segments: Vec<Option<Segment>>,
    durations: Vec<f64>,
    has_init_segment: bool,
    bitrate: u64,
    container: Option<String>,
}

pub struct HlsBuffer {
    inner: Mutex<BufferInner>,
    waveform_enabled: bool,
}

impl std::fmt::Debug for HlsBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HLS buffer")
            .field("waveform_enabled", &self.waveform_enabled)
            .finish()
    }
}

impl HlsBuffer {
    pub fn new(waveform_enabled: bool) -> Self {
        Self {
            inner: Mutex::new(BufferInner {
                segments: vec![],
                durations: vec![],
                has_init_segment: false,
                bitrate: 0,
                container: None,
            }),
            waveform_enabled,
        }
    }

    pub fn waveform_enabled(&self) -> bool {
        self.waveform_enabled
    }

    pub fn get_waveforms(&self) -> Vec<crate::state::types::Waveform> {
        let inner = self.inner.lock().unwrap();
        inner
            .segments
            .iter()
            .filter_map(|s| s.as_ref().and_then(|seg| seg.waveform.clone()))
            .collect()
    }

    pub fn byte_size(&self) -> u64 {
        let inner = self.inner.lock().unwrap();
        inner
            .segments
            .iter()
            .filter_map(|s| s.as_ref())
            .map(|s| s.data.len() as u64)
            .sum()
    }

    pub fn set_segment_waveform(&self, index: usize, waveform: crate::state::types::Waveform) {
        let mut inner = self.inner.lock().unwrap();
        let init_offset = if inner.has_init_segment { 1 } else { 0 };
        let real_index = index + init_offset;

        if let Some(Some(segment)) = inner.segments.get_mut(real_index) {
            segment.waveform = Some(waveform);
        }
    }
    pub fn init(&self, segment_count: usize, durations: Vec<f64>, init_segment: Option<Vec<u8>>) {
        let mut inner = self.inner.lock().unwrap();

        match init_segment {
            Some(data) => {
                inner.segments.resize(segment_count + 1, None);
                inner.segments[0] = Some(Segment {
                    data,
                    waveform: None,
                });
                inner.has_init_segment = true
            }
            None => {
                inner.segments.resize(segment_count, None);
            }
        };
        inner.durations = durations;
    }
    pub fn set_bitrate(&self, bitrate: u64) {
        let mut inner = self.inner.lock().unwrap();
        inner.bitrate = bitrate;
    }

    pub fn set_container(&self, container: String) {
        let mut inner = self.inner.lock().unwrap();
        inner.container = Some(container);
    }

    pub fn container(&self) -> Option<String> {
        let inner = self.inner.lock().unwrap();
        inner.container.clone()
    }

    pub fn write_chunk(&self, index: u64, data: Vec<u8>) {
        let mut inner = self.inner.lock().unwrap();
        let index = index + if inner.has_init_segment { 1 } else { 0 };

        if let Some(slot) = inner.segments.get_mut(index as usize) {
            match slot {
                Some(existing_chunk) => {
                    existing_chunk.data.extend_from_slice(&data);
                }
                None => {
                    *slot = Some(Segment {
                        data,
                        waveform: None,
                    });
                }
            }
        }
    }

    pub fn write_to<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let inner = self.inner.lock().unwrap();
        for segment_opt in &inner.segments {
            if let Some(segment) = segment_opt {
                writer.write_all(&segment.data)?;
            }
        }
        Ok(())
    }

    pub fn is_complete(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        !inner.segments.is_empty() && inner.segments.iter().all(|s| s.is_some())
    }

    pub fn get_all_chunks(&self) -> Vec<(u64, Vec<u8>)> {
        let inner = self.inner.lock().unwrap();
        inner
            .segments
            .iter()
            .enumerate()
            .filter_map(|(index, segment_opt)| {
                segment_opt
                    .as_ref()
                    .map(|segment| (index as u64, segment.data.clone()))
            })
            .collect()
    }

    pub fn read_at(&self, offset: u64, buf: &mut [u8]) -> Option<usize> {
        let inner = self.inner.lock().unwrap();

        if inner.segments.is_empty() {
            return None;
        }

        // 1. Determine the layout of segments in the virtual file
        let init_len = if inner.has_init_segment {
            inner.segments[0]
                .as_ref()
                .map(|s| s.data.len() as u64)
                .unwrap_or(0)
        } else {
            0
        };

        // If offset is within init segment
        if inner.has_init_segment && offset < init_len {
            if let Some(init_seg) = &inner.segments[0] {
                let available = init_seg.data.len() as u64 - offset;
                let to_copy = (buf.len() as u64).min(available) as usize;
                buf[..to_copy]
                    .copy_from_slice(&init_seg.data[offset as usize..offset as usize + to_copy]);
                return Some(to_copy);
            }
        }

        // 2. Map offset to media segments (after init)
        let media_offset = if inner.has_init_segment {
            offset.saturating_sub(init_len)
        } else {
            offset
        };

        // Use bitrate estimation to find which segment we are in
        let bitrate = if inner.bitrate > 0 {
            inner.bitrate
        } else {
            128_000
        }; // Fallback to 128kbps

        let mut accumulated_time = 0.0;
        let mut accumulated_bytes = 0u64;
        let init_offset = if inner.has_init_segment { 1 } else { 0 };

        for (i, &duration) in inner.durations.iter().enumerate() {
            let seg_idx = i + init_offset;
            let estimated_seg_size = (duration * bitrate as f64 / 8.0) as u64;

            // If the segment is downloaded, we know its actual size.
            // BUT: using actual size here would make the offset of subsequent segments unstable.
            // For a seekable "file", we must use a STABLE mapping.
            // So we use estimated size for all media segments to define their boundaries.

            let seg_start = accumulated_bytes;
            let seg_end = seg_start + estimated_seg_size;

            if media_offset >= seg_start && media_offset < seg_end {
                // We found the segment!
                if let Some(Some(segment)) = inner.segments.get(seg_idx) {
                    let internal_offset = media_offset - seg_start;
                    if internal_offset < segment.data.len() as u64 {
                        let available = segment.data.len() as u64 - internal_offset;
                        let to_copy = (buf.len() as u64).min(available) as usize;
                        buf[..to_copy].copy_from_slice(
                            &segment.data
                                [internal_offset as usize..internal_offset as usize + to_copy],
                        );
                        return Some(to_copy);
                    } else {
                        // We are in the "padding" area of the estimated segment size.
                        // If the buffer is not complete, we should wait for the next segment
                        // rather than returning 0 (EOF).
                        if !self.is_complete() {
                            return None; // Wait for data
                        }
                        return Some(0);
                    }
                } else {
                    // Segment not yet downloaded - WAIT
                    return None;
                }
            }

            accumulated_bytes = seg_end;
            accumulated_time += duration;
        }

        // If we reached here, the offset is beyond the last segment
        if self.is_complete() {
            Some(0) // Real EOF
        } else {
            None // Might be more data coming
        }
    }

    pub fn bitrate(&self) -> u64 {
        let inner = self.inner.lock().unwrap();
        if inner.bitrate > 0 {
            inner.bitrate
        } else {
            128_000 // Fallback
        }
    }

    pub fn save_to_file(&self, path: PathBuf) -> std::io::Result<()> {
        let mut file = File::create(path)?;
        self.write_to(&mut file)
    }

    pub fn total_size(&self) -> Option<u64> {
        let inner = self.inner.lock().unwrap();

        if inner.durations.is_empty() {
            return None;
        }

        let total_duration: f64 = inner.durations.iter().sum();

        // Use actual bitrate or fallback (128kbps). This must be consistent with
        // seek_to_byte_approx / bitrate() so that byte ↔ segment mapping is correct.
        // Previously returning only downloaded bytes when bitrate==0 caused a mismatch:
        // total_size was ~8x too small, making seek_to_byte_approx compute a near-EOF
        // byte offset as pointing to a middle segment that isn't yet buffered.
        let bitrate = if inner.bitrate > 0 { inner.bitrate } else { 128_000 };
        let total_bits = bitrate as f64 * total_duration;
        let total_bytes = (total_bits / 8.0) as u64;

        Some(total_bytes)
    }

    pub fn progress(&self) -> f64 {
        let inner = self.inner.lock().unwrap();
        if inner.segments.is_empty() {
            return 0.0;
        }
        let filled = inner.segments.iter().filter(|s| s.is_some()).count();
        filled as f64 / inner.segments.len() as f64
    }

    pub fn first_not_ready_segment(&self, byte_offset: Option<u64>) -> Option<usize> {
        let inner = self.inner.lock().unwrap();

        let start_index = if let Some(offset) = byte_offset {
            if inner.bitrate == 0 || inner.durations.is_empty() {
                0
            } else {
                let bits = offset * 8;
                let seconds = bits as f64 / inner.bitrate as f64;

                let mut accumulated_duration = 0.0;
                let mut segment_index = 0;

                for (i, duration) in inner.durations.iter().enumerate() {
                    accumulated_duration += duration;
                    if seconds < accumulated_duration {
                        segment_index = i;
                        break;
                    }
                    segment_index = i + 1;
                }

                segment_index
            }
        } else {
            0
        };

        let init_offset = if inner.has_init_segment { 1 } else { 0 };
        let real_start = start_index + init_offset;

        inner
            .segments
            .iter()
            .enumerate()
            .skip(real_start)
            .find_map(|(i, seg)| {
                if seg.is_none() {
                    Some(i - init_offset)
                } else {
                    None
                }
            })
            .or_else(|| {
                inner
                    .segments
                    .iter()
                    .enumerate()
                    .skip(init_offset)
                    .find_map(|(i, seg)| {
                        if seg.is_none() {
                            Some(i - init_offset)
                        } else {
                            None
                        }
                    })
            })
    }

    pub fn first_not_ready_segment_ms(&self, ms_offset: Option<u64>) -> Option<usize> {
        let inner = self.inner.lock().unwrap();

        let start_index = if let Some(ms) = ms_offset {
            if inner.durations.is_empty() {
                0
            } else {
                let seconds = ms as f64 / 1000.0;
                let mut accumulated_duration = 0.0;
                let mut segment_index = 0;

                for (i, duration) in inner.durations.iter().enumerate() {
                    accumulated_duration += duration;
                    if seconds < accumulated_duration {
                        segment_index = i;
                        break;
                    }
                    segment_index = i + 1;
                }

                segment_index
            }
        } else {
            0
        };

        let init_offset = if inner.has_init_segment { 1 } else { 0 };
        let real_start = start_index + init_offset;

        inner
            .segments
            .iter()
            .enumerate()
            .skip(real_start)
            .find_map(|(i, seg)| {
                if seg.is_none() {
                    Some(i - init_offset)
                } else {
                    None
                }
            })
            .or_else(|| {
                inner
                    .segments
                    .iter()
                    .enumerate()
                    .skip(init_offset)
                    .find_map(|(i, seg)| {
                        if seg.is_none() {
                            Some(i - init_offset)
                        } else {
                            None
                        }
                    })
            })
    }

    pub fn segment_count(&self) -> usize {
        self.inner.lock().unwrap().durations.len()
    }

    pub fn has_segment(&self, index: usize) -> bool {
        let inner = self.inner.lock().unwrap();
        let init_offset = if inner.has_init_segment { 1 } else { 0 };
        let real_index = index + init_offset;

        inner
            .segments
            .get(real_index)
            .and_then(|s| s.as_ref())
            .is_some()
    }

    pub fn durations(&self) -> Vec<f64> {
        let inner = self.inner.lock().unwrap();
        inner.durations.clone()
    }

    pub fn get_buffered_ranges(&self) -> Vec<(f64, f64)> {
        let inner = self.inner.lock().unwrap();
        let mut ranges = Vec::new();
        let mut current_range: Option<(f64, f64)> = None;
        let mut accumulated_time = 0.0;

        let init_offset = if inner.has_init_segment { 1 } else { 0 };

        for (i, &duration) in inner.durations.iter().enumerate() {
            let seg_idx = i + init_offset;
            let is_ready = inner
                .segments
                .get(seg_idx)
                .and_then(|s| s.as_ref())
                .is_some();

            if is_ready {
                match current_range {
                    Some((_, ref mut end)) => {
                        *end = accumulated_time + duration;
                    }
                    None => {
                        current_range = Some((accumulated_time, accumulated_time + duration));
                    }
                }
            } else {
                if let Some(range) = current_range.take() {
                    ranges.push(range);
                }
            }
            accumulated_time += duration;
        }

        if let Some(range) = current_range {
            ranges.push(range);
        }

        ranges
    }

    //Already concatenated with init if exist
    pub fn get_segment(&self, index: usize) -> Option<(Vec<u8>, f64)> {
        let inner = self.inner.lock().unwrap();
        let init_offset = if inner.has_init_segment { 1 } else { 0 };
        let real_index = index + init_offset;

        let segment = inner.segments.get(real_index)?.as_ref()?;

        let duration = inner.durations.get(index).copied().unwrap_or(0.0);

        if inner.has_init_segment {
            if let Some(init) = inner.segments[0].as_ref() {
                let mut result = Vec::with_capacity(init.data.len() + segment.data.len());
                result.extend_from_slice(&init.data);
                result.extend_from_slice(&segment.data);
                Some((result, duration))
            } else {
                None
            }
        } else {
            Some((segment.data.clone(), duration))
        }
    }

    pub fn offset_to_segment_index(&self, byte_offset: u64) -> Option<usize> {
        let inner = self.inner.lock().unwrap();

        if inner.bitrate == 0 || inner.durations.is_empty() {
            return None;
        }

        let bits = byte_offset * 8;
        let seconds = bits as f64 / inner.bitrate as f64;

        let mut accumulated_duration = 0.0;
        for (i, duration) in inner.durations.iter().enumerate() {
            accumulated_duration += duration;

            if seconds < accumulated_duration {
                return Some(i);
            }
        }

        Some(inner.durations.len().saturating_sub(1))
    }

    pub fn has_offset(&self, byte_offset: u64) -> bool {
        let inner = self.inner.lock().unwrap();

        if inner.bitrate == 0 {
            // If bitrate is unknown, we can't reliably map offset to segment.
            // Return true to allow read_at to try its best.
            return true;
        }

        let bits = byte_offset * 8;
        let seconds = bits as f64 / inner.bitrate as f64;

        let mut accumulated_duration = 0.0;
        let init_offset = if inner.has_init_segment { 1 } else { 0 };

        for (i, duration) in inner.durations.iter().enumerate() {
            accumulated_duration += duration;

            if seconds < accumulated_duration {
                let segment_index = i + init_offset;
                return inner
                    .segments
                    .get(segment_index)
                    .and_then(|s| s.as_ref())
                    .is_some();
            }
        }

        false
    }

    pub fn profile(&self) {
        let inner = self.inner.lock().unwrap();

        let total_segments = inner.segments.len();
        let filled_segments = inner.segments.iter().filter(|s| s.is_some()).count();

        let total_duration: f64 = inner.durations.iter().sum();
        let downloaded = inner
            .segments
            .iter()
            .filter_map(|s| s.as_ref())
            .map(|s| s.data.len() as u64)
            .sum::<u64>();

        println!("╔═══════════════════════════════════════════════════════════════╗");
        println!("║               HLS BUFFER PROFILE                              ║");
        println!("╠═══════════════════════════════════════════════════════════════╣");
        println!(
            "║ Total segments:  {:<10} │ Filled: {:<10}              ║",
            total_segments, filled_segments
        );
        println!(
            "║ Duration:        {:<10.2}s│ Bitrate: {:<10} kbps     ║",
            total_duration,
            inner.bitrate / 1000
        );
        println!(
            "║ Downloaded:      {:<10} bytes                             ║",
            downloaded
        );
        println!(
            "║ Progress:        {:.2}%                                        ║",
            if total_segments > 0 {
                (filled_segments as f64 / total_segments as f64) * 100.0
            } else {
                0.0
            }
        );
        println!(
            "║ Has init seg:    {:<10}                                   ║",
            if inner.has_init_segment { "Yes" } else { "No" }
        );
        println!("╠═══════════════════════════════════════════════════════════════╣");
        println!("║ Segment Map (■ = filled, □ = empty):                         ║");
        println!("╠═══════════════════════════════════════════════════════════════╣");

        const SEGMENTS_PER_ROW: usize = 50;

        for (row_idx, row) in inner.segments.chunks(SEGMENTS_PER_ROW).enumerate() {
            let start_idx = row_idx * SEGMENTS_PER_ROW;
            print!("║ {:04}-{:04} │ ", start_idx, start_idx + row.len() - 1);

            for segment_opt in row {
                let symbol = match segment_opt {
                    None => '□',
                    Some(_) => '■',
                };
                print!("{}", symbol);
            }

            let padding = SEGMENTS_PER_ROW.saturating_sub(row.len());
            for _ in 0..padding {
                print!(" ");
            }

            println!(" ║");
        }

        println!("╠═══════════════════════════════════════════════════════════════╣");
        println!("║ Detailed Segment Info:                                       ║");
        println!("╠═════════╦══════════════╦═══════════════╦═════════════════════╣");
        println!("║  Index  ║    Status    ║  Size (bytes) ║  Duration (s)       ║");
        println!("╠═════════╬══════════════╬═══════════════╬═════════════════════╣");

        for (idx, segment_opt) in inner.segments.iter().enumerate() {
            let duration = if idx == 0 && inner.has_init_segment {
                0.0
            } else {
                let duration_idx = if inner.has_init_segment { idx - 1 } else { idx };
                inner.durations.get(duration_idx).copied().unwrap_or(0.0)
            };

            match segment_opt {
                None => {
                    println!(
                        "║ {:>7} ║ Empty        ║ {:>13} ║ {:>7.2}             ║",
                        idx, 0, duration
                    );
                }
                Some(segment) => {
                    let status = if idx == 0 && inner.has_init_segment {
                        "Init"
                    } else {
                        "Filled"
                    };
                    println!(
                        "║ {:>7} ║ {:<12} ║ {:>13} ║ {:>7.2}             ║",
                        idx,
                        status,
                        segment.data.len(),
                        duration
                    );
                }
            }
        }

        println!("╚═════════╩══════════════╩═══════════════╩═════════════════════╝");
    }

    pub fn save_to_folder<P: AsRef<Path>>(&self, folder_path: P) -> std::io::Result<()> {
        let inner = self.inner.lock().unwrap();

        fs::create_dir_all(&folder_path)?;

        for (index, segment_opt) in inner.segments.iter().enumerate() {
            if let Some(segment) = segment_opt {
                let file_path = if index == 0 && inner.has_init_segment {
                    folder_path.as_ref().join("init.m4a")
                } else {
                    let segment_index = if inner.has_init_segment {
                        index - 1
                    } else {
                        index
                    };
                    folder_path
                        .as_ref()
                        .join(format!("segment{}.m4a", segment_index))
                };

                fs::write(file_path, &segment.data)?;
            }
        }

        Ok(())
    }

    pub fn load_from_folder<P: AsRef<Path>>(&self, folder_path: P) -> std::io::Result<()> {
        let folder = folder_path.as_ref();

        let init_path = folder.join("init.m4a");
        let init_segment = if init_path.exists() {
            Some(fs::read(init_path.clone())?)
        } else {
            None
        };

        let mut segments_data: Vec<Vec<u8>> = Vec::new();
        let mut durations: Vec<f64> = Vec::new();
        let mut segment_index = 0;

        loop {
            let segment_path = folder.join(format!("segment{}.m4a", segment_index));
            if segment_path.exists() {
                let data = fs::read(&segment_path)?;

                let actual_data = if init_path.exists() {
                    [init_segment.clone().unwrap(), data.clone()].concat()
                } else {
                    data.clone()
                };

                if let Some(duration) = Self::extract_duration(&actual_data) {
                    if duration > 0.0 {
                        durations.push(duration);
                    } else {
                        durations.push(10.0);
                    }
                } else {
                    durations.push(10.0);
                }

                segments_data.push(data);
                segment_index += 1;
            } else {
                break;
            }
        }

        if segments_data.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No segment files found in folder",
            ));
        }

        let bitrate = {
            let mut combined_data = Vec::new();
            if let Some(ref init_data) = init_segment {
                combined_data.extend_from_slice(init_data);
            }
            if let Some(first_segment) = segments_data.first() {
                combined_data.extend_from_slice(first_segment);
            }

            if !combined_data.is_empty() {
                Self::extract_bitrate(&combined_data).unwrap_or(0)
            } else {
                0
            }
        };

        let segment_count = segments_data.len();
        self.init(segment_count, durations, init_segment);

        for (idx, data) in segments_data.into_iter().enumerate() {
            self.write_chunk(idx as u64, data);
        }

        if bitrate > 0 {
            self.set_bitrate(bitrate * 1000);
        }

        Ok(())
    }

    fn extract_bitrate(data: &[u8]) -> Option<u64> {
        let cursor = Cursor::new(data);
        let probe = Probe::new(cursor).guess_file_type().ok()?.read().ok()?;

        let properties = probe.properties();
        Some(properties.audio_bitrate()? as u64)
    }

    fn extract_duration(data: &[u8]) -> Option<f64> {
        let cursor = Cursor::new(data);
        let probe = Probe::new(cursor).guess_file_type().ok()?.read().ok()?;

        let properties = probe.properties();
        let duration_millis = properties.duration().as_millis() as f64;
        println!("lofty properties: {:?}", properties);
        Some(duration_millis / 1000.0)
    }
}
