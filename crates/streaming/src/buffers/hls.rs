use super::sync::lock_recover;
use std::fs::{self, File};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use bytes::Bytes;

use lofty::{prelude::*, probe::Probe};

mod readers;
pub use readers::HlsSequentialReader;

#[derive(Debug, Clone)]
struct Segment {
    data: Bytes,
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
    segment_fetches: AtomicU64,
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
            segment_fetches: AtomicU64::new(0),
        }
    }

    pub fn waveform_enabled(&self) -> bool {
        self.waveform_enabled
    }

    pub fn get_waveforms(&self) -> Vec<crate::state::types::Waveform> {
        let inner = lock_recover(&self.inner);
        inner
            .segments
            .iter()
            .filter_map(|s| s.as_ref().and_then(|seg| seg.waveform.clone()))
            .collect()
    }

    pub fn byte_size(&self) -> u64 {
        let inner = lock_recover(&self.inner);
        inner
            .segments
            .iter()
            .filter_map(|s| s.as_ref())
            .map(|s| s.data.len() as u64)
            .sum()
    }

    pub fn set_segment_waveform(&self, index: usize, waveform: crate::state::types::Waveform) {
        let mut inner = lock_recover(&self.inner);
        let init_offset = if inner.has_init_segment { 1 } else { 0 };
        let real_index = index + init_offset;

        if let Some(Some(segment)) = inner.segments.get_mut(real_index) {
            segment.waveform = Some(waveform);
        }
    }
    pub fn init(&self, segment_count: usize, durations: Vec<f64>, init_segment: Option<Vec<u8>>) {
        let mut inner = lock_recover(&self.inner);

        match init_segment {
            Some(data) => {
                inner.segments.resize(segment_count + 1, None);
                inner.segments[0] = Some(Segment {
                    data: Bytes::from(data),
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
        let mut inner = lock_recover(&self.inner);
        inner.bitrate = bitrate;
    }

    pub fn set_container(&self, container: String) {
        let mut inner = lock_recover(&self.inner);
        inner.container = Some(container);
    }

    pub fn container(&self) -> Option<String> {
        let inner = lock_recover(&self.inner);
        inner.container.clone()
    }

    pub fn write_chunk(&self, index: u64, data: Vec<u8>) {
        let mut inner = lock_recover(&self.inner);
        let index = index + if inner.has_init_segment { 1 } else { 0 };

        if let Some(slot) = inner.segments.get_mut(index as usize) {
            match slot {
                Some(existing_chunk) => {
                    let mut joined = Vec::with_capacity(existing_chunk.data.len() + data.len());
                    joined.extend_from_slice(&existing_chunk.data);
                    joined.extend_from_slice(&data);
                    existing_chunk.data = Bytes::from(joined);
                }
                None => {
                    *slot = Some(Segment {
                        data: Bytes::from(data),
                        waveform: None,
                    });
                }
            }
        }
    }

    pub fn write_to<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let inner = lock_recover(&self.inner);
        for segment in inner.segments.iter().flatten() {
            writer.write_all(&segment.data)?;
        }
        Ok(())
    }

    pub fn is_complete(&self) -> bool {
        let inner = lock_recover(&self.inner);
        !inner.segments.is_empty() && inner.segments.iter().all(|s| s.is_some())
    }

    pub fn get_all_chunks(&self) -> Vec<(u64, Vec<u8>)> {
        let inner = lock_recover(&self.inner);
        inner
            .segments
            .iter()
            .enumerate()
            .filter_map(|(index, segment_opt)| {
                segment_opt
                    .as_ref()
                    .map(|segment| (index as u64, segment.data.to_vec()))
            })
            .collect()
    }

    pub fn bitrate(&self) -> u64 {
        let inner = lock_recover(&self.inner);
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
        let inner = lock_recover(&self.inner);

        if inner.durations.is_empty() {
            return None;
        }

        let total_duration: f64 = inner.durations.iter().sum();

        // Use actual bitrate or fallback (128kbps). This must be consistent with
        // seek_to_byte_approx / bitrate() so that byte ↔ segment mapping is correct.
        // Previously returning only downloaded bytes when bitrate==0 caused a mismatch:
        // total_size was ~8x too small, making seek_to_byte_approx compute a near-EOF
        // byte offset as pointing to a middle segment that isn't yet buffered.
        let bitrate = if inner.bitrate > 0 {
            inner.bitrate
        } else {
            128_000
        };
        let total_bits = bitrate as f64 * total_duration;
        let total_bytes = (total_bits / 8.0) as u64;

        Some(total_bytes)
    }

    pub fn progress(&self) -> f64 {
        let inner = lock_recover(&self.inner);
        if inner.segments.is_empty() {
            return 0.0;
        }
        let filled = inner.segments.iter().filter(|s| s.is_some()).count();
        filled as f64 / inner.segments.len() as f64
    }

    pub fn first_not_ready_segment(&self, byte_offset: Option<u64>) -> Option<usize> {
        let inner = lock_recover(&self.inner);

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
        let inner = lock_recover(&self.inner);

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
        lock_recover(&self.inner).durations.len()
    }

    pub fn has_segment(&self, index: usize) -> bool {
        let inner = lock_recover(&self.inner);
        let init_offset = if inner.has_init_segment { 1 } else { 0 };
        let real_index = index + init_offset;

        inner
            .segments
            .get(real_index)
            .and_then(|s| s.as_ref())
            .is_some()
    }

    pub fn durations(&self) -> Vec<f64> {
        let inner = lock_recover(&self.inner);
        inner.durations.clone()
    }

    pub fn get_buffered_ranges(&self) -> Vec<(f64, f64)> {
        let inner = lock_recover(&self.inner);
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
        let inner = lock_recover(&self.inner);
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
            Some((segment.data.to_vec(), duration))
        }
    }

    /// Returns the optional init segment and the media segment as shared `Bytes`
    /// (no copy). `None` when the segment is not stored yet.
    pub fn segment_parts(&self, index: usize) -> Option<(Option<Bytes>, Bytes)> {
        self.segment_fetches.fetch_add(1, Ordering::Relaxed);
        let inner = lock_recover(&self.inner);
        let init_offset = if inner.has_init_segment { 1 } else { 0 };
        let segment = inner.segments.get(index + init_offset)?.as_ref()?;
        let init = if inner.has_init_segment {
            Some(inner.segments[0].as_ref()?.data.clone())
        } else {
            None
        };
        Some((init, segment.data.clone()))
    }

    /// Number of `segment_parts` calls so far.
    pub fn segment_fetch_count(&self) -> u64 {
        self.segment_fetches.load(Ordering::Relaxed)
    }

    pub fn offset_to_segment_index(&self, byte_offset: u64) -> Option<usize> {
        let inner = lock_recover(&self.inner);

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
        let inner = lock_recover(&self.inner);

        if inner.bitrate == 0 {
            // If bitrate is unknown, we can't reliably map offset to segment.
            // Return true to let callers try their best.
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
        use std::fmt::Write as _;
        let mut out = String::new();
        let inner = lock_recover(&self.inner);

        let total_segments = inner.segments.len();
        let filled_segments = inner.segments.iter().filter(|s| s.is_some()).count();

        let total_duration: f64 = inner.durations.iter().sum();
        let downloaded = inner
            .segments
            .iter()
            .filter_map(|s| s.as_ref())
            .map(|s| s.data.len() as u64)
            .sum::<u64>();

        let _ = writeln!(
            out,
            "╔═══════════════════════════════════════════════════════════════╗"
        );
        let _ = writeln!(
            out,
            "║               HLS BUFFER PROFILE                              ║"
        );
        let _ = writeln!(
            out,
            "╠═══════════════════════════════════════════════════════════════╣"
        );
        let _ = writeln!(
            out,
            "║ Total segments:  {:<10} │ Filled: {:<10}              ║",
            total_segments, filled_segments
        );
        let _ = writeln!(
            out,
            "║ Duration:        {:<10.2}s│ Bitrate: {:<10} kbps     ║",
            total_duration,
            inner.bitrate / 1000
        );
        let _ = writeln!(
            out,
            "║ Downloaded:      {:<10} bytes                             ║",
            downloaded
        );
        let _ = writeln!(
            out,
            "║ Progress:        {:.2}%                                        ║",
            if total_segments > 0 {
                (filled_segments as f64 / total_segments as f64) * 100.0
            } else {
                0.0
            }
        );
        let _ = writeln!(
            out,
            "║ Has init seg:    {:<10}                                   ║",
            if inner.has_init_segment { "Yes" } else { "No" }
        );
        let _ = writeln!(
            out,
            "╠═══════════════════════════════════════════════════════════════╣"
        );
        let _ = writeln!(
            out,
            "║ Segment Map (■ = filled, □ = empty):                         ║"
        );
        let _ = writeln!(
            out,
            "╠═══════════════════════════════════════════════════════════════╣"
        );

        const SEGMENTS_PER_ROW: usize = 50;

        for (row_idx, row) in inner.segments.chunks(SEGMENTS_PER_ROW).enumerate() {
            let start_idx = row_idx * SEGMENTS_PER_ROW;
            let _ = write!(
                out,
                "║ {:04}-{:04} │ ",
                start_idx,
                start_idx + row.len() - 1
            );

            for segment_opt in row {
                let symbol = match segment_opt {
                    None => '□',
                    Some(_) => '■',
                };
                let _ = write!(out, "{}", symbol);
            }

            let padding = SEGMENTS_PER_ROW.saturating_sub(row.len());
            for _ in 0..padding {
                let _ = write!(out, " ");
            }

            let _ = writeln!(out, " ║");
        }

        let _ = writeln!(
            out,
            "╠═══════════════════════════════════════════════════════════════╣"
        );
        let _ = writeln!(
            out,
            "║ Detailed Segment Info:                                       ║"
        );
        let _ = writeln!(
            out,
            "╠═════════╦══════════════╦═══════════════╦═════════════════════╣"
        );
        let _ = writeln!(
            out,
            "║  Index  ║    Status    ║  Size (bytes) ║  Duration (s)       ║"
        );
        let _ = writeln!(
            out,
            "╠═════════╬══════════════╬═══════════════╬═════════════════════╣"
        );

        for (idx, segment_opt) in inner.segments.iter().enumerate() {
            let duration = if idx == 0 && inner.has_init_segment {
                0.0
            } else {
                let duration_idx = if inner.has_init_segment { idx - 1 } else { idx };
                inner.durations.get(duration_idx).copied().unwrap_or(0.0)
            };

            match segment_opt {
                None => {
                    let _ = writeln!(
                        out,
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
                    let _ = writeln!(
                        out,
                        "║ {:>7} ║ {:<12} ║ {:>13} ║ {:>7.2}             ║",
                        idx,
                        status,
                        segment.data.len(),
                        duration
                    );
                }
            }
        }

        let _ = writeln!(
            out,
            "╚═════════╩══════════════╩═══════════════╩═════════════════════╝"
        );
        tracing::debug!("{out}");
    }

    pub fn save_to_folder<P: AsRef<Path>>(&self, folder_path: P) -> std::io::Result<()> {
        let inner = lock_recover(&self.inner);

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

                let actual_data = match &init_segment {
                    Some(init) => [init.as_slice(), data.as_slice()].concat(),
                    None => data.clone(),
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
        Some(duration_millis / 1000.0)
    }
}
