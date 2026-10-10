use crate::{
    buffers::hls::HlsBuffer,
    error::{InitializationError, StreamingError},
    hls::{
        parser::parse_playlist,
        segment::{HlsPlaylist, HlsPlaylistType},
    },
    multi_downloader::{DownloadDirection, TrackDownloader},
    state::types::{BufferType, DEFAULT_USER_AGENT},
};
use async_trait::async_trait;
use lofty::{prelude::*, probe::Probe};
use reqwest::Client;
use std::{io::Cursor, sync::Arc, time::Duration};

pub struct HlsDownloader {
    playlist_url: String,
    playlist: Option<HlsPlaylist>,
    client: Option<reqwest::Client>,
    buffer: Arc<HlsBuffer>,
    position_ms: u64,
    write_position: u64,
    bandwidth_bps: u64,
    user_agent: Option<String>,
    ogg_analyzer: Option<crate::waveform::StatefulWaveformAnalyzer>,
    next_analyzer_segment: usize,
}

impl HlsDownloader {
    pub fn new(
        playlist_url: String,
        buffer: Arc<HlsBuffer>,
        playlist: Option<HlsPlaylist>,
        bandwidth_bps: u64,
        user_agent: Option<String>,
    ) -> Self {
        Self {
            client: None,
            playlist,
            playlist_url,
            buffer,
            position_ms: 0,
            write_position: 0,
            bandwidth_bps,
            user_agent,
            ogg_analyzer: None,
            next_analyzer_segment: 0,
        }
    }
    pub async fn init(&mut self) -> Result<(), InitializationError> {
        let mut client_builder = Client::builder().timeout(Duration::from_secs(3));

        let ua = self
            .user_agent
            .clone()
            .unwrap_or_else(|| DEFAULT_USER_AGENT.to_string());
        client_builder = client_builder.user_agent(ua);

        let client = client_builder
            .build()
            .map_err(|e| InitializationError::Error(e.to_string()))?;

        let playlist = match self.playlist.take() {
            Some(playlist) => playlist,
            None => {
                let playlist_type = match Self::fetch_and_parse(&client, &self.playlist_url).await {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::error!(error = %e, "failed to fetch playlist");
                        return Err(InitializationError::Error(e.to_string()));
                    }
                };

                match playlist_type {
                    HlsPlaylistType::Master(master) => {
                        tracing::debug!(
                            variants = master.variants.len(),
                            "master playlist detected"
                        );
                        let variant = master.select_variant(self.bandwidth_bps);

                        tracing::debug!(bandwidth = variant.bandwidth, uri = %variant.uri, "selected variant");

                        match Self::fetch_and_parse(&client, &variant.uri).await {
                            Ok(HlsPlaylistType::Media(media)) => media,
                            Ok(HlsPlaylistType::Master(_)) => {
                                return Err(InitializationError::Error(
                                    "ERROR: Nested master playlists not supported".to_string(),
                                ));
                            }
                            Err(e) => {
                                return Err(InitializationError::Error(e.to_string()));
                            }
                        }
                    }
                    HlsPlaylistType::Media(media) => {
                        tracing::debug!("media playlist detected directly");
                        media
                    }
                }
            }
        };

        if let Some(format) = playlist.detect_format() {
            self.buffer.set_container(format);
        }
        let is_contigious = self.buffer.container().as_deref() == Some("ogg")
            || self.buffer.container().as_deref() == Some("opus");

        let mut write_position = 0;

        let mut combined_data = Vec::new();

        if let Some(init_url) = &playlist.init_segment {
            if let Ok(init_data) = Self::download_segment(
                &client,
                &init_url.url,
                init_url.range_offset,
                init_url.range_length,
            )
            .await
            {
                combined_data.extend_from_slice(&init_data);
            }
        }
        self.buffer.init(
            playlist.segments.len(),
            playlist.segments.iter().map(|s| s.duration).collect(),
            if playlist.init_segment.is_some() {
                Some(combined_data.clone())
            } else {
                None
            },
        );

        if let Some(first_segment) = &playlist.segments.first() {
            if let Ok(segment_data) = Self::download_segment(
                &client,
                &first_segment.url,
                first_segment.range_offset,
                first_segment.range_length,
            )
            .await
            {
                combined_data.extend_from_slice(&segment_data);
                self.buffer.write_chunk(write_position, segment_data);

                self.process_waveforms();

                write_position += 1;
            }
        }

        if is_contigious && playlist.segments.len() > 1 {
            // Download segments 1 and 2 for fast sequential startup
            for idx in 1..=2usize {
                if let Some(seg) = playlist.segments.get(idx) {
                    if let Ok(data) = Self::download_segment(
                        &client,
                        &seg.url,
                        seg.range_offset,
                        seg.range_length,
                    )
                    .await
                    {
                        combined_data.extend_from_slice(&data);
                        self.buffer.write_chunk(idx as u64, data);
                        self.process_waveforms();
                        write_position = idx as u64 + 1;
                    }
                }
            }

            // Download last two segments for gapless playback at the end
            let last_index = playlist.segments.len() - 1;
            if last_index > 2 {
                if let Some(last_segment) = playlist.segments.last() {
                    if let Ok(last_data) = Self::download_segment(
                        &client,
                        &last_segment.url,
                        last_segment.range_offset,
                        last_segment.range_length,
                    )
                    .await
                    {
                        self.buffer.write_chunk(last_index as u64, last_data);
                    }
                }
                if let Some(seg) = playlist.segments.get(last_index - 1) {
                    if let Ok(data) = Self::download_segment(
                        &client,
                        &seg.url,
                        seg.range_offset,
                        seg.range_length,
                    )
                    .await
                    {
                        self.buffer.write_chunk(last_index as u64 - 1, data);
                    }
                }
            }
        }

        if !combined_data.is_empty() {
            if let Some(bitrate) = Self::extract_bitrate(&combined_data) {
                tracing::debug!(kbps = bitrate, "detected bitrate");
                self.buffer.set_bitrate(bitrate * 1000);
            }
        }

        // Convert bytes to milliseconds using downloaded segments' duration
        let durations = self.buffer.durations();
        let downloaded_duration_ms = durations
            .iter()
            .take(write_position as usize)
            .map(|d| (d * 1000.0) as u64)
            .sum();

        self.position_ms = downloaded_duration_ms;
        self.write_position = write_position;
        self.playlist = Some(playlist);
        self.client = Some(client);
        Ok(())
    }

    fn extract_bitrate(data: &[u8]) -> Option<u64> {
        let cursor = Cursor::new(data);
        let probe = Probe::new(cursor).guess_file_type().ok()?.read().ok()?;

        let properties = probe.properties();
        Some(properties.audio_bitrate()? as u64)
    }

    pub fn is_complete(&self) -> bool {
        self.buffer.is_complete()
    }
    pub async fn download(&mut self) -> Result<usize, StreamingError> {
        match self.first_not_ready_segment() {
            Some(segment_index) => self.download_segment_at(segment_index).await,
            None => Ok(0),
        }
    }

    pub async fn download_at(&mut self, offset: u64) -> Result<usize, StreamingError> {
        self.download_segment_at(offset as usize).await
    }

    async fn download_segment_at(&mut self, segment_index: usize) -> Result<usize, StreamingError> {
        if self.buffer.has_segment(segment_index) {
            return Ok(0);
        }

        let playlist = self
            .playlist
            .as_ref()
            .ok_or_else(|| StreamingError::Http("Playlist not initialized".to_string()))?;

        let segment = playlist
            .segments
            .get(segment_index)
            .ok_or_else(|| StreamingError::Http(format!("Segment {} not found", segment_index)))?;

        let client = self
            .client
            .as_ref()
            .ok_or_else(|| StreamingError::Http("Client not initialized".to_string()))?;

        let mut retries = 0;
        loop {
            match Self::download_segment(
                client,
                &segment.url,
                segment.range_offset,
                segment.range_length,
            )
            .await
            {
                Ok(data) => {
                    let len = data.len();
                    self.buffer.write_chunk(segment_index as u64, data);

                    // Update bitrate based on actual downloaded data
                    let inner_durations = self.buffer.durations();
                    let downloaded_bytes = self
                        .buffer
                        .get_all_chunks()
                        .iter()
                        .map(|(_, d)| d.len() as u64)
                        .sum::<u64>();
                    let downloaded_duration: f64 = inner_durations
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| self.buffer.has_segment(*i))
                        .map(|(_, d)| *d)
                        .sum();

                    if downloaded_duration > 0.0 {
                        let new_bitrate =
                            (downloaded_bytes as f64 * 8.0 / downloaded_duration) as u64;
                        self.buffer.set_bitrate(new_bitrate);
                    }

                    if self.buffer.waveform_enabled() {
                        if self.buffer.container().as_deref() == Some("ogg")
                            || self.buffer.container().as_deref() == Some("opus")
                        {
                            self.process_waveforms();
                        } else {
                            if let Some((segment_data, duration)) =
                                self.buffer.get_segment(segment_index)
                            {
                                if let Some(mut waveform) =
                                    crate::waveform::analyze_waveform(&segment_data)
                                {
                                    // Calculate start/end time for the segment
                                    let durations = self.buffer.durations();
                                    let start_time: f64 =
                                        durations.iter().take(segment_index).sum();
                                    waveform.start_time = start_time;
                                    waveform.end_time = start_time + duration;

                                    self.buffer.set_segment_waveform(segment_index, waveform);
                                }
                            }
                        }
                    }

                    self.write_position = (segment_index + 1) as u64;
                    return Ok(len);
                }
                Err(e) => {
                    tracing::warn!(index = segment_index, error = %e, "failed to download segment");
                    retries += 1;
                    if retries >= 3 {
                        tracing::error!(index = segment_index, "max retries reached for segment");
                        return Err(e);
                    }
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }
    }

    fn process_waveforms(&mut self) {
        if !self.buffer.waveform_enabled() {
            return;
        }

        let is_ogg = self.buffer.container().as_deref() == Some("ogg")
            || self.buffer.container().as_deref() == Some("opus");

        if is_ogg {
            if self.ogg_analyzer.is_none() && self.buffer.has_segment(0) {
                use crate::buffers::hls::HlsSequentialReader;
                let reader = HlsSequentialReader::new(self.buffer.clone());
                self.ogg_analyzer =
                    crate::waveform::StatefulWaveformAnalyzer::new(Box::new(reader));
                self.next_analyzer_segment = 0;
            }

            if let Some(analyzer) = &mut self.ogg_analyzer {
                while self.buffer.has_segment(self.next_analyzer_segment) {
                    let durations = self.buffer.durations();
                    if let Some(&duration) = durations.get(self.next_analyzer_segment) {
                        if let Some(mut waveform) = analyzer.analyze_segment(duration) {
                            let start_time: f64 =
                                durations.iter().take(self.next_analyzer_segment).sum();
                            waveform.start_time = start_time;
                            waveform.end_time = start_time + duration;
                            self.buffer
                                .set_segment_waveform(self.next_analyzer_segment, waveform);
                        }
                        self.next_analyzer_segment += 1;
                    } else {
                        break;
                    }
                }
            }
        }
    }

    fn first_not_ready_segment(&self) -> Option<usize> {
        self.buffer
            .first_not_ready_segment_ms(Some(self.position_ms))
    }

    fn first_not_ready_segment_index(&self) -> Option<usize> {
        self.first_not_ready_segment()
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

        tracing::trace!(headers = ?response.headers(), %url, "HLS playlist response");

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

        tracing::trace!(%content, "HLS playlist content");

        parse_playlist(&content, url)
    }

    async fn download_segment(
        client: &Client,
        url: &str,
        range_start: Option<u64>,
        range_length: Option<u64>,
    ) -> Result<Vec<u8>, StreamingError> {
        let mut request = client.get(url);

        if let (Some(start), Some(len)) = (range_start, range_length) {
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
}
#[async_trait]
impl TrackDownloader for HlsDownloader {
    async fn init(&mut self) -> Result<(), InitializationError> {
        self.init().await
    }
    fn buffer(&self) -> BufferType {
        BufferType::Hls(self.buffer.clone())
    }

    async fn download(&mut self) -> Result<usize, StreamingError> {
        self.download().await
    }
    async fn download_at(&mut self, offset: u64) -> Result<usize, StreamingError> {
        self.download_at(offset).await
    }
    fn is_complete(&self) -> bool {
        self.is_complete()
    }
    fn seek(&mut self, pos: crate::multi_downloader::SeekPos) {
        use crate::multi_downloader::SeekPos;
        self.position_ms = match pos {
            SeekPos::Milliseconds(ms) => ms,
            SeekPos::Bytes(bytes) => {
                // Convert bytes to milliseconds as a fraction of the total duration
                let durations = self.buffer.durations();
                let total_duration: f64 = durations.iter().sum();

                if total_duration > 0.0 {
                    let total_bytes = self.buffer.total_size().unwrap_or(0);
                    if total_bytes > 0 {
                        let byte_fraction = bytes as f64 / total_bytes as f64;
                        (total_duration * 1000.0 * byte_fraction) as u64
                    } else {
                        self.position_ms
                    }
                } else {
                    self.position_ms
                }
            }
        };
    }
    fn download_direction(&self) -> DownloadDirection {
        match self.first_not_ready_segment_index() {
            Some(segment_index) => {
                // Convert segment index to milliseconds
                let durations = self.buffer.durations();
                let segment_ms: u64 = durations
                    .iter()
                    .take(segment_index)
                    .map(|d| (d * 1000.0) as u64)
                    .sum();

                if segment_ms >= self.position_ms {
                    DownloadDirection::Forward
                } else {
                    DownloadDirection::Backward
                }
            }
            None => DownloadDirection::None,
        }
    }
}
