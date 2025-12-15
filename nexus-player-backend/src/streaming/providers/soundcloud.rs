use async_trait::async_trait;
use rsoundcloud::models::playlist::TrackType;
use rsoundcloud::models::track::BasicTrack;
use std::sync::Arc;
use tokio::sync::OnceCell;

use rsoundcloud::{PlaylistsApi, ResourceId, SearchApi, SoundCloudClient, TracksApi};

use crate::audio::types::MediaSource;
use crate::library::TrackSource;
use crate::streaming::error::{StreamError, StreamResult};
use crate::streaming::provider::StreamProvider;
use crate::streaming::types::{
    ContentType, ResolvedContent, SearchResult, StreamMetadata, StreamingService, TrackInfo,
};

pub struct SoundCloudProvider {
    client: OnceCell<SoundCloudClient>,
    ytdlp: Arc<dyn StreamProvider>,
}

impl SoundCloudProvider {
    pub fn new(ytdlp: Arc<dyn StreamProvider>) -> Self {
        Self {
            client: OnceCell::new(),
            ytdlp,
        }
    }

    async fn get_client(&self) -> StreamResult<&SoundCloudClient> {
        self.client
            .get_or_try_init(|| async {
                SoundCloudClient::default().await.map_err(|e| {
                    StreamError::Other(format!("Failed to create SoundCloud client: {}", e))
                })
            })
            .await
    }

    fn is_soundcloud_url(url: &str) -> bool {
        url.contains("soundcloud.com")
    }

    fn extract_track_id_from_url(url: &str) -> Option<String> {
        url.split('/').last().map(|s| s.to_string())
    }
}

#[async_trait]
impl StreamProvider for SoundCloudProvider {
    fn name(&self) -> &str {
        "SoundCloud"
    }

    async fn resolve(&self, source: &TrackSource) -> StreamResult<String> {
        match source {
            TrackSource::Http(_) => self.ytdlp.resolve(source).await,
            TrackSource::External { system: _, id: _ } => self.ytdlp.resolve(source).await,
            _ => Err(StreamError::NotSupported),
        }
    }

    async fn fetch_metadata(&self, source: &TrackSource) -> StreamResult<StreamMetadata> {
        let url = match source {
            TrackSource::Http(url) => url.clone(),
            TrackSource::External { id, .. } => id.clone(),
            _ => {
                return Err(StreamError::InvalidSource(
                    "Invalid source type for SoundCloud".to_string(),
                ))
            }
        };

        let client = self.get_client().await?;
        let track = client
            .get_track(ResourceId::Url(url))
            .await
            .map_err(|e| StreamError::MetadataError(format!("SoundCloud API error: {}", e)))?;

        Ok(StreamMetadata {
            title: Some(track.track.title),
            artist: Some(track.user.username),
            album: None,
            duration: Some(std::time::Duration::from_millis(
                track.track.duration as u64,
            )),
            artwork_url: track.track.artwork_url,
            sample_rate: None,
            channels: None,
        })
    }

    async fn get_stream(&self, source: &TrackSource) -> StreamResult<Box<dyn MediaSource>> {
        self.ytdlp.get_stream(source).await
    }

    async fn get_stream_v2(&self, source: &ResolvedContent) -> StreamResult<Box<dyn MediaSource>> {
        self.ytdlp.get_stream_v2(source).await
    }

    async fn search_typed(
        &self,
        query: &str,
        content_type: Option<ContentType>,
        limit: usize,
        _offset: usize,
    ) -> StreamResult<Vec<SearchResult>> {
        if query.is_empty() {
            return Ok(Vec::new());
        }

        if content_type.is_some() && content_type != Some(ContentType::Track) {
            return Ok(Vec::new());
        }

        let mut params = rsoundcloud::CollectionParams::default();
        params.limit = Some(limit as u32);

        let client = self.get_client().await?;
        let results = client
            .search_tracks(query.to_string(), params)
            .await
            .map_err(|e| StreamError::MetadataError(format!("SoundCloud search error: {}", e)))?;

        let search_results: Vec<SearchResult> = results
            .into_iter()
            .map(|track| {
                let track_url = track.track.permalink_url.clone();

                SearchResult::Track(TrackInfo {
                    source: TrackSource::External {
                        system: "soundcloud".to_string(),
                        id: track_url,
                    },
                    title: track.track.title,
                    artist: track.user.username,
                    duration: Some(std::time::Duration::from_millis(
                        track.track.duration as u64,
                    )),
                    thumbnail: track.track.artwork_url,
                    description: track.track.description,
                })
            })
            .collect();

        Ok(search_results)
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

        let client = self.get_client().await?;
        let playlist = client
            .get_playlist(ResourceId::Url(url.to_string()))
            .await
            .map_err(|e| StreamError::MetadataError(format!("SoundCloud API error: {}", e)))?;

        let track_sources: Vec<TrackSource> = playlist
            .album_playlist
            .tracks
            .into_iter()
            .map(|track| TrackSource::External {
                system: "soundcloud".to_string(),
                id: match track {
                    TrackType::Basic(t) => t.track.uri,
                    TrackType::Mini(t) => t.id.to_string(),
                },
            })
            .collect();

        Ok(track_sources)
    }

    async fn is_available(&self) -> bool {
        true
    }
}
