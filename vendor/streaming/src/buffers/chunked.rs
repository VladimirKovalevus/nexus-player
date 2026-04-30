use std::{fs::File, io::Write, path::PathBuf, sync::Mutex};

use crate::error::StreamingError;
pub const CHUNK_SIZE: u64 = 64 * 1024; // 64kb

#[derive(Debug, Clone)]
pub struct Chunk {
    pub data: Vec<u8>,
    pub index: u64,
    pub waveform: Option<crate::state::types::Waveform>,
}

struct BufferInner {
    chunks: Vec<Option<Chunk>>,
    total_size: Option<u64>,
    error: Option<String>,
}

pub struct ChunkedBuffer {
    inner: Mutex<BufferInner>,
    chunk_size: u64,
    waveform_enabled: bool,
}
impl std::fmt::Debug for ChunkedBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Chunked buffer")
            .field("chunk_size", &self.chunk_size)
            .field("waveform_enabled", &self.waveform_enabled)
            .finish()
    }
}
impl ChunkedBuffer {
    pub fn new(waveform_enabled: bool) -> Self {
        Self::with_chunk_size(CHUNK_SIZE * 3, waveform_enabled)
    }
    pub fn with_chunk_size(chunk_size: u64, waveform_enabled: bool) -> Self {
        Self {
            inner: Mutex::new(BufferInner {
                chunks: Vec::new(),
                total_size: None,
                error: None,
            }),
            chunk_size,
            waveform_enabled,
        }
    }

    pub fn waveform_enabled(&self) -> bool {
        self.waveform_enabled
    }

    pub fn get_waveforms(&self) -> Vec<crate::state::types::Waveform> {
        let inner = self.inner.lock().unwrap();
        inner
            .chunks
            .iter()
            .filter_map(|c| c.as_ref().and_then(|chunk| chunk.waveform.clone()))
            .collect()
    }

    pub fn byte_size(&self) -> u64 {
        let inner = self.inner.lock().unwrap();
        inner
            .chunks
            .iter()
            .filter_map(|c| c.as_ref())
            .map(|c| c.data.len() as u64)
            .sum()
    }

    pub fn get_chunk_data(&self, index: u64) -> Option<Vec<u8>> {
        let inner = self.inner.lock().unwrap();
        inner
            .chunks
            .get(index as usize)?
            .as_ref()
            .map(|c| c.data.clone())
    }

    pub fn set_chunk_waveform(&self, index: u64, waveform: crate::state::types::Waveform) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(Some(chunk)) = inner.chunks.get_mut(index as usize) {
            chunk.waveform = Some(waveform);
        }
    }

    pub fn chunk_size(&self) -> u64 {
        self.chunk_size
    }
    pub fn get_all_chunks(&self) -> Vec<(u64, Vec<u8>)> {
        let inner = self.inner.lock().unwrap();

        let chunks = &inner.chunks;
        chunks
            .iter()
            .enumerate()
            .filter_map(|(index, chunk_opt)| {
                chunk_opt
                    .as_ref()
                    .map(|chunk| (index as u64, chunk.data.clone()))
            })
            .collect()
    }

    pub fn get_chunks_info(&self) -> Vec<(usize, bool, usize)> {
        let inner = self.inner.lock().unwrap();

        inner
            .chunks
            .iter()
            .enumerate()
            .map(|(index, chunk_opt)| {
                let (is_filled, size) = match chunk_opt {
                    Some(chunk) => (true, chunk.data.len()),
                    None => (false, 0),
                };
                (index, is_filled, size)
            })
            .collect()
    }
    pub fn debug(&self) {
        let inner = self.inner.lock().unwrap();
        inner.chunks.iter().enumerate().for_each(|(i, c)| {});
    }

    pub fn is_complete(&self) -> bool {
        let total_size = match self.total_size() {
            Some(s) => s,
            None => return false,
        };

        let inner = self.inner.lock().unwrap();
        let total_chunks = (total_size + self.chunk_size - 1) / self.chunk_size;

        inner
            .chunks
            .iter()
            .take(total_chunks as usize)
            .enumerate()
            .all(|(ind, c)| c.as_ref().is_some_and(|chunk| chunk.data.len() > 0))
    }

    pub fn set_total_size(&self, size: u64) {
        let mut inner = self.inner.lock().unwrap();
        inner.total_size = Some(size);
        let num_chunks = size.div_ceil(self.chunk_size);
        inner.chunks.resize(num_chunks as usize, None);
    }
    pub fn write_chunk(&self, index: u64, data: Vec<u8>) {
        let mut inner = self.inner.lock().unwrap();

        ///Append not rewrite
        if let Some(slot) = inner.chunks.get_mut(index as usize) {
            match slot {
                Some(existing_chunk) => {
                    existing_chunk.data.extend_from_slice(&data);
                }
                None => {
                    *slot = Some(Chunk {
                        data,
                        index,
                        waveform: None,
                    });
                }
            }

            drop(inner);
        }
    }
    pub fn has_chunk(&self, index: u64) -> bool {
        let inner = self.inner.lock().unwrap();

        if let Some(chunk) = inner.chunks.get(index as usize) {
            let is_last_chunk = index as usize == inner.chunks.len().saturating_sub(1);
            chunk.as_ref().is_some_and(|c| {
                if is_last_chunk {
                    // Last chunk can be smaller than chunk_size
                    c.data.len() > 0
                } else {
                    c.data.len() == self.chunk_size as usize
                }
            })
        } else {
            false
        }
    }
    pub fn first_not_ready_chunk(&self, offset: Option<usize>) -> Option<usize> {
        let inner = self.inner.lock().unwrap();
        let to_skip = match offset {
            Some(offset) => offset as u64 / self.chunk_size,
            None => 0,
        };
        let total_size = inner.total_size.unwrap_or_default();
        let total_chunks = (total_size + self.chunk_size - 1) / self.chunk_size;
        inner
            .chunks
            .iter()
            .skip(to_skip as usize)
            .enumerate()
            .find_map(|(i, c)| match c {
                None => Some(i + to_skip as usize),
                Some(chunk) if chunk.data.len() < self.chunk_size as usize => {
                    if i + to_skip as usize + 1 == inner.chunks.len() {
                        None
                    } else {
                        Some(i + to_skip as usize + 1)
                    }
                }
                _ => None,
            })
            .or_else(|| {
                inner.chunks.iter().enumerate().find_map(|(i, c)| match c {
                    None => Some(i),
                    Some(chunk) if chunk.data.len() < self.chunk_size as usize => Some(i + 1),
                    _ => None,
                })
            })
    }
    pub fn first_not_ready_chunk_offset(&self, offset: Option<usize>) -> Option<u64> {
        let index = self.first_not_ready_chunk(offset);
        match index {
            Some(i) => Some(i as u64 * self.chunk_size),
            None => None,
        }
    }
    pub fn read_at(&self, offset: u64, buf: &mut [u8]) -> Option<usize> {
        let inner = self.inner.lock().unwrap();

        let chunk_index = offset / self.chunk_size;
        let chunk_offset = (offset % self.chunk_size) as usize;

        let chunk = inner.chunks.get(chunk_index as usize)?.as_ref()?;

        let available = chunk.data.len().saturating_sub(chunk_offset);
        let to_read = buf.len().min(available);
        if available == 0 {
            return Some(0);
        }

        buf[..to_read].copy_from_slice(&chunk.data[chunk_offset..chunk_offset + to_read]);

        Some(to_read)
    }
    pub fn chunk_index_for_offset(&self, offset: u64) -> u64 {
        offset / self.chunk_size
    }

    pub fn total_size(&self) -> Option<u64> {
        self.inner.lock().unwrap().total_size
    }

    pub fn save_to_file(&self, path: PathBuf) -> std::io::Result<()> {
        let inner = self.inner.lock().unwrap();
        let mut file = File::create(path)?;
        for chunk_opt in &inner.chunks {
            if let Some(chunk) = chunk_opt {
                file.write_all(&chunk.data)?;
            }
        }
        Ok(())
    }
    pub fn progress(&self) -> f64 {
        let total_size = match self.total_size() {
            Some(s) => s,
            None => return 0.0,
        };

        if total_size == 0 {
            return 1.0;
        }

        let inner = self.inner.lock().unwrap();
        let total_chunks = ((total_size + self.chunk_size - 1) / self.chunk_size) as usize;

        let downloaded_chunks = inner
            .chunks
            .iter()
            .take(total_chunks)
            .filter(|c| c.is_some())
            .count();

        downloaded_chunks as f64 / total_chunks as f64
    }

    pub fn downloaded_bytes(&self) -> u64 {
        let inner = self.inner.lock().unwrap();

        let mut total = 0u64;
        for chunk_opt in inner.chunks.iter() {
            if let Some(chunk) = chunk_opt {
                total += chunk.data.len() as u64;
            }
        }

        total
    }

    /// Logs a visual profile of chunk fill levels
    pub fn profile(&self) {
        let inner = self.inner.lock().unwrap();

        let total_chunks = inner.chunks.len();
        let filled_chunks = inner.chunks.iter().filter(|c| c.is_some()).count();
        let total_size = inner.total_size.unwrap_or(0);
        let downloaded = inner
            .chunks
            .iter()
            .filter_map(|c| c.as_ref())
            .map(|c| c.data.len() as u64)
            .sum::<u64>();

        println!("╔═══════════════════════════════════════════════════════════════╗");
        println!("║               BUFFER PROFILE                                  ║");
        println!("╠═══════════════════════════════════════════════════════════════╣");
        println!(
            "║ Total chunks:    {:<10} │ Filled: {:<10}              ║",
            total_chunks, filled_chunks
        );
        println!(
            "║ Total size:      {:<10} │ Downloaded: {:<10}         ║",
            total_size, downloaded
        );
        println!(
            "║ Progress:        {:.2}%                                        ║",
            if total_size > 0 {
                (downloaded as f64 / total_size as f64) * 100.0
            } else {
                0.0
            }
        );
        println!("╠═══════════════════════════════════════════════════════════════╣");
        println!("║ Chunk Map (■ = full, ▓ = partial, □ = empty):                ║");
        println!("╠═══════════════════════════════════════════════════════════════╣");

        // Display chunks in rows of 50
        const CHUNKS_PER_ROW: usize = 50;

        for (row_idx, row) in inner.chunks.chunks(CHUNKS_PER_ROW).enumerate() {
            let start_idx = row_idx * CHUNKS_PER_ROW;
            print!("║ {:04}-{:04} │ ", start_idx, start_idx + row.len() - 1);

            for chunk_opt in row {
                let symbol = match chunk_opt {
                    None => '□',
                    Some(chunk) => {
                        let fill_ratio = chunk.data.len() as f64 / self.chunk_size as f64;
                        if fill_ratio >= 1.0 {
                            '■'
                        } else if fill_ratio > 0.0 {
                            '▓'
                        } else {
                            '□'
                        }
                    }
                };
                print!("{}", symbol);
            }

            // Pad the rest with spaces if needed
            let padding = CHUNKS_PER_ROW.saturating_sub(row.len());
            for _ in 0..padding {
                print!(" ");
            }

            println!(" ║");
        }

        println!("╠═══════════════════════════════════════════════════════════════╣");
        println!("║ Detailed Chunk Info:                                         ║");
        println!("╠═════════╦══════════════╦═══════════════╦═════════════════════╣");
        println!("║  Index  ║    Status    ║  Size (bytes) ║   Fill %            ║");
        println!("╠═════════╬══════════════╬═══════════════╬═════════════════════╣");

        for (idx, chunk_opt) in inner.chunks.iter().enumerate() {
            match chunk_opt {
                None => {
                    println!(
                        "║ {:>7} ║ Empty        ║ {:>13} ║ {:>7.2}%            ║",
                        idx, 0, 0.0
                    );
                }
                Some(chunk) => {
                    let fill_percent = (chunk.data.len() as f64 / self.chunk_size as f64) * 100.0;
                    let status = if chunk.data.len() == self.chunk_size as usize {
                        "Full"
                    } else {
                        "Partial"
                    };
                    println!(
                        "║ {:>7} ║ {:<12} ║ {:>13} ║ {:>7.2}%            ║",
                        idx,
                        status,
                        chunk.data.len(),
                        fill_percent
                    );
                }
            }
        }

        println!("╚═════════╩══════════════╩═══════════════╩═════════════════════╝");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_buffer() {
        let buffer = ChunkedBuffer::new(false);
        assert_eq!(buffer.total_size(), None);
    }

    #[test]
    fn test_set_total_size() {
        let buffer = ChunkedBuffer::new(false);
        buffer.set_total_size(1024);

        assert_eq!(buffer.total_size(), Some(1024));

        assert!(!buffer.has_chunk(0));
    }

    #[test]
    fn test_write_and_read_chunk() {
        let buffer = ChunkedBuffer::new(false);
        buffer.set_total_size(CHUNK_SIZE as u64 * 2);

        let data = vec![42u8; CHUNK_SIZE as usize];
        buffer.write_chunk(0, data);

        assert!(buffer.has_chunk(0));
        assert!(!buffer.has_chunk(1));

        let mut buf = vec![0u8; 100];
        let n = buffer.read_at(0, &mut buf).unwrap();

        assert_eq!(n, 100);
        assert_eq!(buf[0], 42);
    }

    #[test]
    fn test_read_across_chunk_boundary() {
        let buffer = ChunkedBuffer::new(false);
        buffer.set_total_size(CHUNK_SIZE as u64);

        let data = vec![42u8; CHUNK_SIZE as usize];
        buffer.write_chunk(0, data);

        let offset = CHUNK_SIZE - 50;
        let mut buf = vec![0u8; 100];
        let n = buffer.read_at(offset as u64, &mut buf).unwrap();
        assert!(n == 50);
        assert!(buf[32] == 42u8);
        assert!(buf[51] == 0);
    }
}
