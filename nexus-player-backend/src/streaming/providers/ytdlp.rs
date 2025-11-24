use async_trait::async_trait;
use serde::Deserialize;
use std::fs::File;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::io::AsyncBufReadExt;
use tokio::process::Command;
use tracing::info;

use crate::audio::types::MediaSource;
use crate::library::TrackSource;
use crate::streaming::cache::CacheManager;
use crate::streaming::error::{StreamError, StreamResult};
use crate::streaming::provider::StreamProvider;
use crate::streaming::types::{
    AlbumInfo, AudioQuality, ChannelInfo, ContentType, PlaylistInfo, ResolvedContent, SearchResult,
    StreamMetadata, StreamingService, TrackInfo,
};

#[derive(Clone, Debug, Deserialize)]
struct YtDlpThumbnail {
    url: String,
}

#[derive(Clone, Debug, Deserialize)]
struct YtDlpMetadata {
    title: Option<String>,
    artist: Option<String>,
    uploader: Option<String>,
    album: Option<String>,
    duration: Option<f64>,
    thumbnails: Option<Vec<YtDlpThumbnail>>,
    url: Option<String>,
    ext: Option<String>,

    asr: Option<u32>,
    abr: Option<f64>,

    #[serde(rename = "_type")]
    entry_type: Option<String>,

    entries: Option<Vec<YtDlpMetadata>>,

    playlist_title: Option<String>,
    playlist_uploader: Option<String>,
    playlist_count: Option<usize>,

    channel: Option<String>,
    channel_id: Option<String>,
    channel_follower_count: Option<u64>,

    webpage_url: Option<String>,
    id: Option<String>,
    description: Option<String>,
}

pub struct YtDlpProvider {
    quality: AudioQuality,
    executable: String,
    cache: Arc<CacheManager>,
}

impl YtDlpProvider {
    pub fn new(cache: Arc<CacheManager>) -> Self {
        Self {
            quality: AudioQuality::default(),
            executable: "yt-dlp".to_string(),
            cache,
        }
    }

    pub fn with_quality(mut self, quality: AudioQuality) -> Self {
        self.quality = quality;
        self
    }

    pub fn with_executable(mut self, executable: String) -> Self {
        self.executable = executable;
        self
    }

    async fn run_ytdlp(&self, args: &[&str]) -> StreamResult<String> {
        info!("Запуск ytdlp {:?}", args);
        let output = Command::new(&self.executable)
            .args(args)
            .output()
            .await
            .map_err(|e| StreamError::Other(format!("Failed to execute yt-dlp: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(StreamError::Other(format!("yt-dlp error: {}", stderr)));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn fetch_json(&self, url: &str, extract_flat: bool) -> StreamResult<YtDlpMetadata> {
        let mut args = vec!["-J", "--no-warnings"];

        if extract_flat {
            args.push("--flat-playlist");
        }

        args.push(url);

        let json_output = self.run_ytdlp(&args).await?;

        serde_json::from_str(&json_output)
            .map_err(|e| StreamError::MetadataError(format!("Failed to parse yt-dlp JSON: {}", e)))
    }
    pub async fn get_audio_url(&self, url: &str, quality: AudioQuality) -> StreamResult<String> {
        tracing::debug!("Получение аудио URL: {} (качество: {:?})", url, quality);

        let format_selector = self.get_format_selector();

        let output = tokio::time::timeout(
            Duration::from_secs_f64(10 as f64),
            Command::new(&self.executable)
                .args(&[
                    "--get-url",
                    "--format",
                    &format_selector,
                    "--no-playlist",
                    url,
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output(),
        )
        .await
        .map_err(|_| StreamError::Timeout)?
        .map_err(|e| StreamError::Other(e.to_string()))?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(StreamError::MetadataError(error.to_string()));
        }

        let audio_url = String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .ok_or_else(|| StreamError::NotSupported)?
            .trim()
            .to_string();

        tracing::debug!("Получен аудио URL: {}", audio_url);
        Ok(audio_url)
    }

    fn get_format_selector(&self) -> String {
        "bestaudio/best".to_string()
    }

    async fn get_direct_url(&self, url: &str) -> StreamResult<String> {
        let meta = self.fetch_json(url, false).await?;

        meta.url
            .ok_or_else(|| StreamError::NoStreamAvailable("No direct URL found".to_string()))
    }

    async fn download_file(&self, url: &str, output_path: &std::path::Path) -> StreamResult<()> {
        info!("Скачивание файла: {} -> {:?}", url, output_path);

        let format_selector = self.get_format_selector();
        let output_template = output_path.to_string_lossy().to_string();

        let output = Command::new(&self.executable)
            .args(&[
                "-x",
                "--audio-format",
                "mp3",
                "--format",
                &format_selector,
                "--output",
                &output_template,
                "--no-playlist",
                url,
            ])
            .output()
            .await
            .map_err(|e| StreamError::Other(format!("Failed to execute yt-dlp: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(StreamError::Other(format!(
                "yt-dlp download error: {}",
                stderr
            )));
        }

        info!("Файл успешно скачан: {:?}", output_path);
        Ok(())
    }

    fn extract_service(url: &str) -> Option<StreamingService> {
        if url.contains("youtube.com") || url.contains("youtu.be") {
            Some(StreamingService::YouTube)
        } else if url.contains("soundcloud.com") {
            Some(StreamingService::SoundCloud)
        } else if url.contains("bandcamp.com") {
            Some(StreamingService::Bandcamp)
        } else {
            None
        }
    }

    fn metadata_to_track_info(&self, meta: YtDlpMetadata) -> Option<TrackInfo> {
        let title = meta.title?;
        let artist = meta
            .artist
            .or(meta.uploader)
            .unwrap_or_else(|| "Unknown".to_string());

        let duration = meta.duration.map(|d| std::time::Duration::from_secs_f64(d));

        let source_url = meta.webpage_url.as_deref().or(meta.url.as_deref());
        if source_url.is_none() {
            return None;
        }

        let source = TrackSource::Http(source_url.unwrap().to_string());
        let thumbnail = meta
            .thumbnails
            .and_then(|t| t.first().map(|t| t.url.clone()));

        Some(TrackInfo {
            source,
            title,
            artist,
            duration,
            thumbnail: thumbnail,
            description: meta.description,
        })
    }
}

#[async_trait]
impl StreamProvider for YtDlpProvider {
    fn name(&self) -> &str {
        "YtDlp"
    }

    async fn resolve(&self, source: &TrackSource) -> StreamResult<String> {
        match source {
            TrackSource::Http(url) => self.get_audio_url(url, AudioQuality::Best).await,
            TrackSource::External { id, system } if system == "ytdlp" => {
                self.get_audio_url(id, AudioQuality::Best).await
            }
            _ => Err(StreamError::NotSupported),
        }
    }

    async fn fetch_metadata(&self, source: &TrackSource) -> StreamResult<StreamMetadata> {
        let url = match source {
            TrackSource::Http(url) => url.as_str(),
            TrackSource::External { id, .. } => id.as_str(),
            _ => {
                return Err(StreamError::InvalidSource(
                    "Invalid source type".to_string(),
                ))
            }
        };

        let meta = self.fetch_json(url, false).await?;
        let thumbnail = meta
            .thumbnails
            .and_then(|t| t.first().map(|t| t.url.clone()));

        Ok(StreamMetadata {
            title: meta.title,
            artist: meta.artist.or(meta.uploader),
            album: meta.album,
            duration: meta.duration.map(|d| std::time::Duration::from_secs_f64(d)),
            artwork_url: thumbnail,
            sample_rate: meta.asr,
            channels: None,
        })
    }

    async fn get_stream(&self, source: &TrackSource) -> StreamResult<Box<dyn MediaSource>> {
        let url = match source {
            TrackSource::Http(url) => url.as_str(),
            TrackSource::External { id, .. } => id.as_str(),
            _ => {
                return Err(StreamError::InvalidSource(
                    "Invalid source type".to_string(),
                ))
            }
        };

        if let Some(cached_path) = self.cache.get(url).await {
            info!("Используем кэшированный файл для: {}", url);
            let file = File::open(&cached_path).map_err(|e| StreamError::IoError(e))?;
            return Ok(Box::new(file));
        }

        info!("Скачиваем файл для: {}", url);

        let output_path = self.cache.reserve(url).await?;

        self.download_file(url, &output_path).await?;

        let file_metadata = std::fs::metadata(&output_path).map_err(|e| StreamError::IoError(e))?;
        let file_size = file_metadata.len();

        self.cache.put(url, file_size).await?;

        let file = File::open(&output_path).map_err(|e| StreamError::IoError(e))?;
        info!("file {:?}", file.metadata());
        Ok(Box::new(file))
    }

    async fn get_stream_v2(&self, content: &ResolvedContent) -> StreamResult<Box<dyn MediaSource>> {
        match content {
            ResolvedContent::Track(track) => self.get_stream(&track.source).await,
            ResolvedContent::Playlist(playlist) => {
                if let Some(first_track) = playlist.tracks().first() {
                    self.get_stream(&first_track.source).await
                } else {
                    Err(StreamError::Other("Playlist is empty".to_string()))
                }
            }
        }
    }

    async fn search_typed(
        &self,
        query: &str,
        content_type: Option<ContentType>,
        limit: usize,
        _offset: usize,
    ) -> StreamResult<Vec<SearchResult>> {
        let search_query = format!("scsearch{}:{}", limit, query);

        let meta = self.fetch_json(&search_query, true).await?;

        let entries = meta
            .entries
            .ok_or_else(|| StreamError::MetadataError("No search results found".to_string()))?;

        let mut results = Vec::new();

        for entry in entries {
            let entry_type = entry.entry_type.as_deref();
            let url = entry.webpage_url.as_deref().or(entry.url.as_deref());

            if url.is_none() {
                continue;
            }

            let url = url.unwrap();

            match (entry_type, &content_type) {
                (Some("playlist"), None | Some(ContentType::Playlist)) => {
                    if let Some(playlist) = self.metadata_to_playlist_info(entry.clone()) {
                        results.push(SearchResult::Playlist(playlist));
                    }
                }
                (Some("url") | None, None | Some(ContentType::Track)) => {
                    if let Some(track) = self.metadata_to_track_info(entry.clone()) {
                        results.push(SearchResult::Track(track));
                    }
                }
                _ => {}
            }
        }

        Ok(results)
    }

    async fn expand_collection(&self, source: &TrackSource) -> StreamResult<Vec<TrackSource>> {
        let url = match source {
            TrackSource::Http(url) => url.as_str(),
            TrackSource::External { id, .. } => id.as_str(),
            _ => {
                return Err(StreamError::InvalidSource(
                    "Invalid source type".to_string(),
                ))
            }
        };

        let meta = self.fetch_json(url, true).await?;

        let entries = meta.entries.ok_or_else(|| {
            StreamError::MetadataError("Not a collection (playlist/album)".to_string())
        })?;

        let track_sources: Vec<TrackSource> = entries
            .into_iter()
            .filter_map(|entry| {
                entry
                    .webpage_url
                    .or(entry.url)
                    .map(|url| TrackSource::Http(url))
            })
            .collect();

        Ok(track_sources)
    }

    async fn is_available(&self) -> bool {
        Command::new(&self.executable)
            .arg("--version")
            .output()
            .await
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}

impl YtDlpProvider {
    fn metadata_to_playlist_info(&self, meta: YtDlpMetadata) -> Option<PlaylistInfo> {
        let id = meta.id?;
        let title = meta.playlist_title.or(meta.title)?;
        let creator = meta
            .playlist_uploader
            .or(meta.uploader)
            .unwrap_or_else(|| "Unknown".to_string());

        let thumbnail = meta
            .thumbnails
            .and_then(|t| t.first().map(|t| t.url.clone()));

        let source_url = meta.webpage_url.as_deref().or(meta.url.as_deref());
        if source_url.is_none() {
            return None;
        }
        let service = Self::extract_service(source_url.unwrap())?;
        let tracks = meta
            .entries
            .and_then(|entries| {
                entries
                    .iter()
                    .map(|e| self.metadata_to_track_info(e.clone()))
                    .collect()
            })
            .unwrap_or_default();
        Some(PlaylistInfo {
            id,
            url: source_url.unwrap().to_string(),
            service,
            title,
            creator,
            tracks: tracks,
            track_count: meta.playlist_count.unwrap_or(0),
            thumbnail: thumbnail,
            description: meta.description,
        })
    }
}
