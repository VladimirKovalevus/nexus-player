use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::time::Instant;

use symphonia::core::io::MediaSourceStream;

const DEFAULT_CHUNK_SIZE: usize = 1024 * 1024; // 1MB
const DEFAULT_MAX_CACHED_CHUNKS: usize = 10; // 10MB total cache

#[derive(Debug)]
struct CachedChunk {
    data: Vec<u8>,
    last_accessed: Instant,
}

#[derive(Debug)]
pub struct ChunkedMediaSource {
    client: reqwest::blocking::Client,
    url: String,

    chunks: HashMap<u64, CachedChunk>,
    chunk_size: usize,

    position: u64,
    total_size: Option<u64>,
    max_cached_chunks: usize,
}

impl ChunkedMediaSource {
    pub fn new(url: String) -> std::io::Result<Self> {
        Self::with_config(url, DEFAULT_CHUNK_SIZE, DEFAULT_MAX_CACHED_CHUNKS)
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn with_config(
        url: String,
        chunk_size: usize,
        max_cached_chunks: usize,
    ) -> std::io::Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        let total_size = Self::fetch_content_length(&client, &url)?;

        let mut source = Self {
            client,
            url,
            chunks: HashMap::new(),
            chunk_size,
            position: 0,
            total_size,
            max_cached_chunks,
        };

        source.download_chunk(0)?;

        Ok(source)
    }

    fn fetch_content_length(
        client: &reqwest::blocking::Client,
        url: &str,
    ) -> std::io::Result<Option<u64>> {
        let response = client
            .head(url)
            .send()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        Ok(response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok()))
    }

    fn chunk_index_for_position(&self, pos: u64) -> u64 {
        pos / self.chunk_size as u64
    }

    fn chunk_start(&self, chunk_index: u64) -> u64 {
        chunk_index * self.chunk_size as u64
    }

    fn chunk_end(&self, chunk_index: u64) -> u64 {
        let end = (chunk_index + 1) * self.chunk_size as u64;
        if let Some(total) = self.total_size {
            end.min(total)
        } else {
            end
        }
    }

    fn download_chunk(&mut self, chunk_index: u64) -> std::io::Result<()> {
        if self.chunks.contains_key(&chunk_index) {
            self.chunks.get_mut(&chunk_index).unwrap().last_accessed = Instant::now();
            return Ok(());
        }

        let start = self.chunk_start(chunk_index);
        let end = self.chunk_end(chunk_index) - 1;

        let range_header = format!("bytes={}-{}", start, end);

        let response = self
            .client
            .get(&self.url)
            .header(reqwest::header::RANGE, range_header)
            .send()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        if !response.status().is_success() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("HTTP error: {}", response.status()),
            ));
        }

        let data = response
            .bytes()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
            .to_vec();

        self.chunks.insert(
            chunk_index,
            CachedChunk {
                data,
                last_accessed: Instant::now(),
            },
        );

        self.evict_old_chunks();

        Ok(())
    }

    fn get_chunk(&mut self, chunk_index: u64) -> std::io::Result<&[u8]> {
        if !self.chunks.contains_key(&chunk_index) {
            self.download_chunk(chunk_index)?;
        }

        let chunk = self.chunks.get_mut(&chunk_index).unwrap();
        chunk.last_accessed = Instant::now();
        Ok(&chunk.data)
    }

    fn evict_old_chunks(&mut self) {
        if self.chunks.len() <= self.max_cached_chunks {
            return;
        }

        let current_chunk = self.chunk_index_for_position(self.position);

        let mut chunks_to_remove: Vec<u64> = self
            .chunks
            .iter()
            .filter(|(idx, _)| {
                let distance = if **idx > current_chunk {
                    **idx - current_chunk
                } else {
                    current_chunk - **idx
                };
                distance > 2
            })
            .map(|(idx, chunk)| (*idx, chunk.last_accessed))
            .collect::<Vec<_>>()
            .into_iter()
            .map(|(idx, _)| idx)
            .collect();

        chunks_to_remove.sort_by_key(|idx| {
            self.chunks
                .get(idx)
                .map(|c| c.last_accessed)
                .unwrap_or(Instant::now())
        });

        let to_remove = chunks_to_remove
            .into_iter()
            .take(self.chunks.len() - self.max_cached_chunks);

        for idx in to_remove {
            self.chunks.remove(&idx);
        }
    }

    fn prefetch_next_chunk(&mut self) {
        let next_chunk = self.chunk_index_for_position(self.position) + 1;

        if let Some(total_size) = self.total_size {
            if self.chunk_start(next_chunk) < total_size {
                let _ = self.download_chunk(next_chunk);
            }
        }
    }

    // TODO(human): Add methods to save downloaded chunks to local file
    // This will allow:
    // 1. Avoiding re-downloads when cloning ChunkedMediaSource
    // 2. Progressive caching - save chunks as they download
    // 3. Resume playback from cached data
    //
    // Suggested methods:
    // - pub fn save_to_file(&self, path: impl AsRef<Path>) -> std::io::Result<()>
    // - pub fn load_from_file(url: String, path: impl AsRef<Path>) -> std::io::Result<Self>
    //
    // Consider:
    // - File format: simple sequential write of all chunks, or indexed format?
    // - When to save: after each chunk, on drop, or explicitly?
    // - How to validate cached data matches the URL/content
}

impl Read for ChunkedMediaSource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        if let Some(total) = self.total_size {
            if self.position >= total {
                return Ok(0);
            }
        }

        let chunk_index = self.chunk_index_for_position(self.position);
        let chunk_start = self.chunk_start(chunk_index);

        self.get_chunk(chunk_index)?;

        let chunk_data = &self.chunks.get(&chunk_index).unwrap().data;
        let offset_in_chunk = (self.position - chunk_start) as usize;
        let available_in_chunk = chunk_data.len() - offset_in_chunk;

        let to_read = buf.len().min(available_in_chunk);
        buf[..to_read].copy_from_slice(&chunk_data[offset_in_chunk..offset_in_chunk + to_read]);

        self.position += to_read as u64;

        self.prefetch_next_chunk();

        Ok(to_read)
    }
}

impl Seek for ChunkedMediaSource {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let new_position = match pos {
            SeekFrom::Start(offset) => offset as i64,
            SeekFrom::End(offset) => {
                if let Some(total) = self.total_size {
                    total as i64 + offset
                } else {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Unsupported,
                        "Cannot seek from end without known size",
                    ));
                }
            }
            SeekFrom::Current(offset) => self.position as i64 + offset,
        };

        if new_position < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Cannot seek to negative position",
            ));
        }

        self.position = new_position as u64;

        let chunk_index = self.chunk_index_for_position(self.position);
        if !self.chunks.contains_key(&chunk_index) {
            self.download_chunk(chunk_index)?;
        }

        Ok(self.position)
    }
}

impl symphonia::core::io::MediaSource for ChunkedMediaSource {
    fn is_seekable(&self) -> bool {
        self.total_size.is_some()
    }

    fn byte_len(&self) -> Option<u64> {
        self.total_size
    }
}

impl crate::audio::types::MediaSource for ChunkedMediaSource {
    fn clone_box(&self) -> Box<dyn crate::audio::types::MediaSource> {
        Box::new(
            ChunkedMediaSource::with_config(
                self.url.clone(),
                self.chunk_size,
                self.max_cached_chunks,
            )
            .expect("Failed to recreate ChunkedMediaSource"),
        )
    }
}
