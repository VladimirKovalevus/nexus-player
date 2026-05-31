use crate::{
    buffers::chunked::{ChunkedBuffer, CHUNK_SIZE},
    downloader::{DownloadCommand, DEFAULT_CAPACITY, MAX_RETRIES},
    error::StreamingError,
    hls::{
        parser::parse_playlist,
        segment::{HlsPlaylist, HlsPlaylistType, HlsSegment},
    },
    state::types::DEFAULT_USER_AGENT,
};
use crossbeam::channel::{bounded, Receiver, Sender};
use reqwest::Client;
use std::{sync::Arc, time::Duration};

pub struct HlsDownloader {
    command_tx: Sender<DownloadCommand>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl HlsDownloader {
    pub fn new(playlist_url: String, buffer: Arc<ChunkedBuffer>, bandwidth_bps: u64) -> Self {
        let (tx, rx) = bounded(DEFAULT_CAPACITY);

        let url_clone = playlist_url.clone();
        let buffer_clone = buffer.clone();

        let thread = std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::error!("Failed to create tokio runtime: {}", e);
                    return;
                }
            };
            rt.block_on(Self::download_loop(
                url_clone,
                buffer_clone,
                rx,
                bandwidth_bps,
            ));
        });

        Self {
            command_tx: tx,
            thread: Some(thread),
        }
    }

    pub fn start(&self) {
        let _ = self.command_tx.send(DownloadCommand::Start);
    }

    pub fn pause(&self) {
        let _ = self.command_tx.send(DownloadCommand::Pause);
    }

    pub fn seek_to(&self, offset: u64) {
        let _ = self.command_tx.send(DownloadCommand::Seek(offset));
    }

    pub fn stop(&self) {
        let _ = self.command_tx.send(DownloadCommand::Stop);
    }

    async fn download_loop(
        playlist_url: String,
        buffer: Arc<ChunkedBuffer>,
        command_rx: Receiver<DownloadCommand>,
        bandwidth_bps: u64,
    ) {
        let client = match Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(DEFAULT_USER_AGENT)
            .build()
        {
            Ok(client) => client,
            Err(e) => {
                tracing::error!("Failed to build HTTP client: {}", e);
                return;
            }
        };

        let playlist_type = match Self::fetch_and_parse(&client, &playlist_url).await {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Failed to fetch playlist: {}", e);
                return;
            }
        };
        let mut playlist = match playlist_type {
            HlsPlaylistType::Master(master) => {
                println!(
                    "Master playlist detected with {} variants",
                    master.variants.len()
                );

                let variant = master.select_variant(bandwidth_bps);

                println!(
                    "Selected variant: bandwidth={}, uri={}",
                    variant.bandwidth, variant.uri
                );

                match Self::fetch_and_parse(&client, &variant.uri).await {
                    Ok(HlsPlaylistType::Media(media)) => media,
                    Ok(HlsPlaylistType::Master(_)) => {
                        eprintln!("ERROR: Nested master playlists not supported");
                        return;
                    }
                    Err(e) => {
                        eprintln!("Failed to fetch media playlist: {}", e);
                        return;
                    }
                }
            }
            HlsPlaylistType::Media(media) => {
                println!("Media playlist detected directly");
                media
            }
        };

        let mut current_byte_offset = 0;
        let estimated_size = Self::estimate_total_size(&playlist);
        buffer.set_total_size(estimated_size);

        if let Some(init_url) = &playlist.init_segment {
            if let Ok(init_data) = Self::download_init_segment(&client, init_url).await {
                current_byte_offset += init_data.len() as u64;
                Self::write_segment_to_buffer(&buffer, 0, init_data);
            }
        }

        let mut current_segment_idx = 0;
        let mut paused = false;
        let mut retries = 0;

        while current_segment_idx < playlist.segments.len() {
            match command_rx.recv_timeout(Duration::from_millis(10)) {
                Ok(DownloadCommand::Stop) => {
                    break;
                }
                Ok(DownloadCommand::Pause) => {
                    paused = true;
                }
                Ok(DownloadCommand::Start) => {
                    paused = false;
                }
                Ok(DownloadCommand::Seek(offset)) => {
                    if let Some(idx) = Self::find_segment_for_offset(&playlist, offset) {
                        current_segment_idx = idx;
                        paused = false;
                    }
                }
                Err(_) => {}
            }

            if paused {
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }

            let segment = &mut playlist.segments[current_segment_idx];

            match Self::download_segment(&client, segment).await {
                Ok(data) => {
                    current_segment_idx += 1;
                    retries = 0;
                    let len = data.len() as u64;
                    Self::write_segment_to_buffer(&buffer, current_byte_offset, data);
                    segment.byte_offset = current_byte_offset;
                    current_byte_offset += len;
                    Self::update_total_size(&buffer, &playlist, current_segment_idx);
                }
                Err(e) => {
                    eprintln!("Failed to download segment {}: {}", current_segment_idx, e);

                    retries += 1;
                    if retries >= MAX_RETRIES {
                        eprintln!("Max retries reached, stopping");
                        break;
                    }

                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }

        println!("HLS download loop finished");
    }
    async fn fetch_and_parse(
        client: &Client,
        url: &str,
    ) -> Result<HlsPlaylistType, StreamingError> {
        let response = client
            .get(url)
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7")
            .header("Accept-Language", "en-US,en;q=0.9")
            .send()
            .await
            .map_err(|e| StreamingError::Http(e.to_string()))?;

        if !response.status().is_success() {
            return Err(StreamingError::Http(format!(
                "Playlist request failed: {}",
                response.status()
            )));
        }

        let content = response
            .text()
            .await
            .map_err(|e| StreamingError::Http(e.to_string()))?;

        println!("HLS FETCH PLAYLIST CONTENT:\n{}", content);

        parse_playlist(&content, url)
    }

    async fn download_init_segment(
        client: &Client,
        init: &crate::hls::segment::InitSegment,
    ) -> Result<Vec<u8>, StreamingError> {
        let mut request = client.get(&init.url);

        if let (Some(start), Some(len)) = (init.range_offset, init.range_length) {
            let range = format!("bytes={}-{}", start, start + len - 1);
            request = request.header("Range", range);
        }

        let response = request
            .send()
            .await
            .map_err(|e| StreamingError::Http(e.to_string()))?;

        if !response.status().is_success() {
            return Err(StreamingError::Http(format!(
                "Init segment request failed: {}",
                response.status()
            )));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| StreamingError::Http(e.to_string()))?;

        Ok(bytes.to_vec())
    }

    async fn download_segment(
        client: &Client,
        segment: &HlsSegment,
    ) -> Result<Vec<u8>, StreamingError> {
        let mut request = client.get(&segment.url);

        if let (Some(start), Some(len)) = (segment.range_offset, segment.range_length) {
            let range = format!("bytes={}-{}", start, start + len - 1);
            request = request.header("Range", range);
        }

        let response = request
            .send()
            .await
            .map_err(|e| StreamingError::Http(e.to_string()))?;

        if !response.status().is_success() {
            return Err(StreamingError::Http(format!(
                "Segment request failed: {}",
                response.status()
            )));
        }

        let data = response
            .bytes()
            .await
            .map_err(|e| StreamingError::Http(e.to_string()))?
            .to_vec();

        Ok(data)
    }

    fn write_segment_to_buffer(buffer: &Arc<ChunkedBuffer>, offset: u64, data: Vec<u8>) {
        let mut current_offset = offset;
        let mut remaining = &data[..];

        while !remaining.is_empty() {
            let chunk_index = current_offset / CHUNK_SIZE;
            let chunk_offset = current_offset % CHUNK_SIZE;

            let available_in_chunk = CHUNK_SIZE - chunk_offset;
            let to_write = remaining.len().min(available_in_chunk as usize);

            let chunk_data = remaining[..to_write].to_vec();

            buffer.write_chunk(chunk_index, chunk_data);

            current_offset += to_write as u64;
            remaining = &remaining[to_write..];
        }
    }

    fn find_segment_for_offset(playlist: &HlsPlaylist, offset: u64) -> Option<usize> {
        playlist.segments.iter().position(|segment| {
            let segment_start = segment.byte_offset;
            let segment_end = segment.byte_offset + segment.byte_size.unwrap_or(u64::MAX);

            offset >= segment_start && offset < segment_end
        })
    }

    fn estimate_total_size(playlist: &HlsPlaylist) -> u64 {
        let total_duration = playlist.total_duration();
        (total_duration * 20_000.0) as u64
    }

    fn update_total_size(
        buffer: &Arc<ChunkedBuffer>,
        playlist: &HlsPlaylist,
        downloaded_count: usize,
    ) {
        if downloaded_count == 0 {
            return;
        }

        let downloaded_bytes = buffer.downloaded_bytes();

        let avg_segment_size = downloaded_bytes / downloaded_count as u64;

        let estimated_total = avg_segment_size * playlist.segments.len() as u64;

        buffer.set_total_size(estimated_total);
    }
}

impl Drop for HlsDownloader {
    fn drop(&mut self) {
        self.stop();

        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use mockito::Server;

    #[tokio::test]
    async fn test_hls_download_simple_playlist() {
        let mut server = Server::new_async().await;

        let playlist_content = r#"#EXTM3U
#EXT-X-VERSION:7
#EXTINF:10.0,
seg0.m4s
#EXTINF:10.0,
seg1.m4s
#EXT-X-ENDLIST
"#;

        let playlist_mock = server
            .mock("GET", "/playlist.m3u8")
            .with_status(200)
            .with_header("Content-Type", "application/vnd.apple.mpegurl")
            .with_body(playlist_content)
            .create_async()
            .await;

        let seg0_mock = server
            .mock("GET", "/seg0.m4s")
            .with_status(200)
            .with_body(vec![0u8; 10000])
            .create_async()
            .await;

        let seg1_mock = server
            .mock("GET", "/seg1.m4s")
            .with_status(200)
            .with_body(vec![1u8; 10000])
            .create_async()
            .await;

        let buffer = Arc::new(ChunkedBuffer::new(false));
        let url = format!("{}/playlist.m3u8", server.url());

        let downloader = HlsDownloader::new(url, buffer.clone(), 0);
        downloader.start();

        tokio::time::sleep(Duration::from_secs(3)).await;

        assert!(buffer.downloaded_bytes() > 0);

        downloader.stop();

        playlist_mock.assert_async().await;
        seg0_mock.assert_async().await;
        seg1_mock.assert_async().await;
    }
}
