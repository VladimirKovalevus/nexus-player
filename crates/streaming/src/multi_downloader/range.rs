use async_trait::async_trait;
use crossbeam::channel::{bounded, Receiver, Sender};
use reqwest::Client;
use std::{sync::Arc, thread, time::Duration, u64};

use crate::{
    buffers::{chunked::ChunkedBuffer, Buffer},
    error::{InitializationError, StreamingError},
    multi_downloader::{DownloadDirection, TrackDownloader},
    state::types::{BufferType, DEFAULT_USER_AGENT},
};

fn with_bandcamp_headers(request: reqwest::RequestBuilder, url: &str) -> reqwest::RequestBuilder {
    if url.contains("bandcamp.com") || url.contains("bcbits.com") {
        return request
            .header("Referer", "https://bandcamp.com/")
            .header("Origin", "https://bandcamp.com")
            .header("Sec-Fetch-Site", "same-site")
            .header("Sec-Fetch-Mode", "cors")
            .header("Sec-Fetch-Dest", "audio");
    }
    request
}

pub struct RangeDownloader {
    url: String,
    buffer: Arc<ChunkedBuffer>,
    client: Option<Client>,
    position: u64,
    user_agent: Option<String>,
}

impl RangeDownloader {
    pub fn new(url: String, buffer: Arc<ChunkedBuffer>, user_agent: Option<String>) -> Self {
        Self {
            client: None,
            url,
            buffer,
            position: 0,
            user_agent,
        }
    }

    pub async fn download_chunk(
        client: &Client,
        url: &str,
        start: u64,
        end: u64,
    ) -> Result<Vec<u8>, StreamingError> {
        let range = format!("bytes={}-{}", start, end);
        let request = client.get(url).header("Range", range);
        let resp = with_bandcamp_headers(request, url)
            .send()
            .await
            .map_err(|e| StreamingError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(StreamingError::Http(format!("Status: {}", resp.status())));
        }
        if let Some(content_type) = resp.headers().get("content-type") {
            if let Ok(content_type_str) = content_type.to_str() {
                let ct = content_type_str.to_ascii_lowercase();
                if ct.contains("text/html") || ct.contains("application/xhtml+xml") {
                    return Err(StreamingError::Http(format!(
                        "Expected audio bytes, got HTML response for URL: {}",
                        url
                    )));
                }
            }
        }

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| StreamingError::Io(e.to_string()))?;

        Ok(bytes.to_vec())
    }
    pub async fn init(&mut self) -> Result<(), InitializationError> {
        let ua = self
            .user_agent
            .clone()
            .unwrap_or_else(|| DEFAULT_USER_AGENT.to_string());

        // Explicitly use rustls to avoid native-tls (Windows SChannel) being selected
        // when both TLS backends are compiled in via feature unification (e.g. another dependency pulls native-tls).
        // SChannel has a known JA3 fingerprint that triggers Cloudflare bot challenges.
        let client = reqwest::Client::builder()
            .use_rustls_tls()
            .timeout(Duration::from_secs(30))
            .user_agent(ua)
            .build()
            .map_err(|e| InitializationError::Error(e.to_string()))?;

        let head_request = client.head(&self.url);
        match with_bandcamp_headers(head_request, &self.url).send().await {
            Ok(response) => {
                if let Some(size) = response.headers().get("content-length") {
                    if let Ok(length_str) = size.to_str() {
                        if let Ok(size) = length_str.parse::<u64>() {
                            self.buffer.set_total_size(size);
                        }
                    }
                }
            }
            Err(e) => return Err(InitializationError::Error(e.to_string())),
        };
        self.client = Some(client);
        Ok(())
    }
    pub async fn download_at(&mut self, offset: u64) -> Result<usize, StreamingError> {
        let chunk_index = offset / self.buffer.chunk_size() as u64;

        if self.buffer.has_chunk(chunk_index) {
            self.position += self.buffer.chunk_size();
        }
        let url = &self.url;
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| StreamingError::Http("HTTP client is not initialised".to_string()))?
            .clone();
        let current_offset = offset;
        let mut retries = 0;
        loop {
            let result = RangeDownloader::download_chunk(
                &client,
                &url,
                current_offset,
                current_offset + self.buffer.chunk_size() - 1,
            )
            .await;
            match result {
                Ok(chunk) => {
                    let len = chunk.len();
                    self.buffer.write_chunk(chunk_index, chunk);

                    if self.buffer.waveform_enabled() {
                        let buffer_clone = self.buffer.clone();
                        let chunk_data = buffer_clone.get_chunk_data(chunk_index);

                        if let Some(data) = chunk_data {
                            // Background analysis (simplified)
                            if let Some(mut waveform) = crate::waveform::analyze_waveform(&data) {
                                waveform.byte_start = Some(chunk_index * buffer_clone.chunk_size());
                                waveform.byte_end =
                                    Some((chunk_index + 1) * buffer_clone.chunk_size());
                                buffer_clone.set_chunk_waveform(chunk_index, waveform);
                            }
                        }
                    }
                    self.position += self.buffer.chunk_size();
                    return Ok(len);
                }
                Err(e) => {
                    if retries >= 3 {
                        return Err(StreamingError::Http(e.to_string()));
                    }
                    retries += 1;
                }
            }
        }
    }
}

#[async_trait]
impl TrackDownloader for RangeDownloader {
    async fn init(&mut self) -> Result<(), InitializationError> {
        self.init().await
    }
    async fn download(&mut self) -> Result<usize, StreamingError> {
        match self
            .buffer
            .first_not_ready_chunk_offset(Some(self.position as usize))
        {
            Some(offset) => self.download_at(offset).await,
            None => Ok(0),
        }
    }
    fn is_complete(&self) -> bool {
        self.buffer.is_complete()
    }
    async fn download_at(&mut self, offset: u64) -> Result<usize, StreamingError> {
        self.download_at(offset).await
    }
    fn seek(&mut self, pos: crate::multi_downloader::SeekPos) {
        use crate::multi_downloader::SeekPos;
        self.position = match pos {
            SeekPos::Bytes(offset) => offset,
            SeekPos::Milliseconds(_) => {
                // RangeDownloader doesn't support time-based seeking
                // Could convert using bitrate if available
                self.position
            }
        };
    }
    fn download_direction(&self) -> DownloadDirection {
        match self
            .buffer
            .first_not_ready_chunk_offset(Some(self.position as usize))
        {
            Some(offset) => {
                if offset >= self.position {
                    DownloadDirection::Forward
                } else {
                    DownloadDirection::Backward
                }
            }
            None => DownloadDirection::None,
        }
    }
    fn buffer(&self) -> BufferType {
        BufferType::Chunked(self.buffer.clone())
    }
}
