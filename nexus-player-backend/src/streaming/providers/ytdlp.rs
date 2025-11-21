use async_trait::async_trait;
use serde::Deserialize;
use std::fs::File;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::AsyncBufReadExt;
use tokio::process::Command;
use tracing::info;

use crate::audio::types::MediaSource;
use crate::library::TrackSource;
use crate::streaming::cache::CacheManager;
use crate::streaming::error::{StreamError, StreamResult};
use crate::streaming::provider::StreamProvider;
use crate::streaming::types::{
    AlbumInfo, AudioQuality, ChannelInfo, ContentType, PlaylistInfo, SearchResult, StreamMetadata,
    StreamingService, TrackInfo,
};

#[derive(Clone, Debug, Deserialize)]
struct YtDlpMetadata {
    title: Option<String>,
    artist: Option<String>,
    uploader: Option<String>,
    album: Option<String>,
    duration: Option<f64>,
    thumbnail: Option<String>,
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

    fn get_format_selector(&self) -> String {
        "bestaudio/best".to_string()
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

    fn metadata_to_track_info(&self, meta: YtDlpMetadata, source_url: &str) -> Option<TrackInfo> {
        let title = meta.title?;
        let artist = meta
            .artist
            .or(meta.uploader)
            .unwrap_or_else(|| "Unknown".to_string());

        let duration = meta.duration.map(|d| std::time::Duration::from_secs_f64(d));

        let source = TrackSource::Http(source_url.to_string());

        Some(TrackInfo {
            source,
            title,
            artist,
            duration,
            thumbnail: meta.thumbnail,
            description: meta.description,
        })
    }
}

#[async_trait]
impl StreamProvider for YtDlpProvider {
    fn name(&self) -> &str {
        "YtDlp"
    }

    fn resolve(&self, source: &TrackSource) -> bool {
        match source {
            TrackSource::Http(url) => Self::extract_service(url).is_some(),
            TrackSource::External { system, .. } => system == "ytdlp",
            _ => false,
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

        Ok(StreamMetadata {
            title: meta.title,
            artist: meta.artist.or(meta.uploader),
            album: meta.album,
            duration: meta.duration.map(|d| std::time::Duration::from_secs_f64(d)),
            artwork_url: meta.thumbnail,
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

        Err(StreamError::NotSupported)
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

            info!("fo Watafo {:?} {:?}", entry_type, content_type);
            match (entry_type, &content_type) {
                (Some("playlist"), None | Some(ContentType::Playlist)) => {
                    if let Some(playlist) = self.metadata_to_playlist_info(entry.clone(), url) {
                        results.push(SearchResult::Playlist(playlist));
                    }
                }
                (Some("url") | None, None | Some(ContentType::Track)) => {
                    if let Some(track) = self.metadata_to_track_info(entry.clone(), url) {
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
    fn metadata_to_playlist_info(&self, meta: YtDlpMetadata, url: &str) -> Option<PlaylistInfo> {
        let service = Self::extract_service(url)?;
        let id = meta.id?;
        let title = meta.playlist_title.or(meta.title)?;
        let creator = meta
            .playlist_uploader
            .or(meta.uploader)
            .unwrap_or_else(|| "Unknown".to_string());

        Some(PlaylistInfo {
            id,
            url: url.to_string(),
            service,
            title,
            creator,
            track_count: meta.playlist_count.unwrap_or(0),
            thumbnail: meta.thumbnail,
            description: meta.description,
        })
    }
}
