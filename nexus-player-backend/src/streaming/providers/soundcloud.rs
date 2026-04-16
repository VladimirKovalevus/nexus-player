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
    AlbumInfo, ContentType, PlaylistInfo, ResolveOptions, ResolvedContent, SearchResult,
    StreamMetadata, StreamingService, TrackInfo,
};

pub struct SoundCloudProvider {
    client: OnceCell<SoundCloudClient>,
}

impl SoundCloudProvider {
    pub fn new() -> Self {
        Self {
            client: OnceCell::new(),
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

    async fn resolve(
        &self,
        source: &TrackSource,
        _options: Option<ResolveOptions>,
    ) -> StreamResult<(String, StreamMetadata)> {
        let url = match source {
            TrackSource::Http(url) => url.clone(),
            TrackSource::External { id, .. } => id.clone(),
            _ => return Err(StreamError::NotSupported),
        };

        let client = self.get_client().await?;
        let track = client
            .get_track(ResourceId::Url(url))
            .await
            .map_err(|e| {
                let msg = format!("{}", e);
                if msg.contains("404") || msg.to_lowercase().contains("not found") {
                    StreamError::Unavailable("Track not found (404)".to_string())
                } else if msg.contains("403") || msg.to_lowercase().contains("forbidden") {
                    StreamError::Unavailable("Track forbidden (403)".to_string())
                } else {
                    StreamError::MetadataError(format!("SoundCloud API error: {}", e))
                }
            })?;

        // Find HLS transcoding
        let transcoding = track
            .track
            .media
            .transcodings
            .iter()
            .find(|t| t.format.protocol == "hls" && t.format.mime_type.contains("opus"))
            .or_else(|| {
                track
                    .track
                    .media
                    .transcodings
                    .iter()
                    .find(|t| t.format.protocol == "hls")
            })
            .or_else(|| track.track.media.transcodings.first())
            .ok_or_else(|| StreamError::NoStreamAvailable("No transcodings found".to_string()))?;

        let stream_url = client.get_stream_url(&transcoding.url).await.map_err(|e| {
            let msg = format!("{}", e);
            if msg.contains("404") || msg.to_lowercase().contains("not found") {
                StreamError::Unavailable("Stream URL not found (404)".to_string())
            } else if msg.contains("403") || msg.to_lowercase().contains("forbidden") {
                StreamError::Unavailable("Stream URL forbidden (403)".to_string())
            } else {
                StreamError::Other(format!("Failed to get SoundCloud stream URL: {}", e))
            }
        })?;

        Ok((
            stream_url,
            StreamMetadata {
                title: Some(track.track.title),
                artist: Some(track.user.username),
                album: None,
                duration: Some(std::time::Duration::from_millis(track.track.duration as u64)),
                artwork_url: track.track.artwork_url,
                sample_rate: None,
                channels: None,
            },
        ))
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
            .map_err(|e| {
                let msg = format!("{}", e);
                if msg.contains("404") || msg.to_lowercase().contains("not found") {
                    StreamError::Unavailable("Track not found (404)".to_string())
                } else if msg.contains("403") || msg.to_lowercase().contains("forbidden") {
                    StreamError::Unavailable("Track forbidden (403)".to_string())
                } else {
                    StreamError::MetadataError(format!("SoundCloud API error: {}", e))
                }
            })?;

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

    async fn get_stream(&self, _source: &TrackSource) -> StreamResult<Box<dyn MediaSource>> {
        Err(StreamError::NotSupported)
    }

    async fn get_stream_v2(&self, _source: &ResolvedContent) -> StreamResult<Box<dyn MediaSource>> {
        Err(StreamError::NotSupported)
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

        let collection_params =
            rsoundcloud::CollectionParams::new(Some(limit as u32), Some(_offset as u32));

        let query = query.to_string();
        let client = self.get_client().await?;

        let content_type = content_type.unwrap_or(ContentType::Track);

        match content_type {
            ContentType::Track => {
                let results = client
                    .search_tracks(query, collection_params)
                    .await
                    .map_err(|e| {
                        StreamError::MetadataError(format!("SoundCloud search error: {}", e))
                    })?;

                let search_results: Vec<SearchResult> = results
                    .into_iter()
                    .map(|item| {
                        SearchResult::Track(TrackInfo {
                            source: TrackSource::External {
                                system: "soundcloud".to_string(),
                                id: item.track.permalink_url.clone(),
                            },
                            title: item.track.title,
                            artist: item.user.username,
                            duration: Some(std::time::Duration::from_millis(
                                item.track.duration as u64,
                            )),
                            thumbnail: item.track.artwork_url,
                            description: item.track.description,
                        })
                    })
                    .collect();
                Ok(search_results)
            }
            ContentType::Playlist => {
                let results = client
                    .search_playlists(query, collection_params)
                    .await
                    .map_err(|e| {
                        StreamError::MetadataError(format!("SoundCloud search error: {}", e))
                    })?;

                let mut search_results = Vec::new();
                for item in results {
                    let mut tracks: Vec<TrackInfo> = item
                        .album_playlist
                        .tracks
                        .into_iter()
                        .filter_map(|tt| match tt {
                            TrackType::Basic(t) => Some(TrackInfo {
                                source: TrackSource::External {
                                    system: "soundcloud".to_string(),
                                    id: t.track.permalink_url,
                                },
                                title: t.track.title,
                                artist: t.user.username,
                                duration: Some(std::time::Duration::from_millis(
                                    t.track.duration as u64,
                                )),
                                thumbnail: t.track.artwork_url,
                                description: t.track.description,
                            }),
                            TrackType::Mini(_) => None,
                        })
                        .collect();

                    tracing::info!("SoundCloud Playlist tracks: {:?}", tracks);

                    search_results.push(SearchResult::Playlist(PlaylistInfo {
                        id: item.album_playlist.permalink_url.clone(),
                        url: item.album_playlist.permalink_url,
                        service: StreamingService::SoundCloud,
                        title: item.album_playlist.title,
                        creator: item.user.user.username,
                        tracks,
                        track_count: item.album_playlist.track_count as usize,
                        thumbnail: item.album_playlist.artwork_url,
                        description: item.album_playlist.description,
                    }));
                }
                Ok(search_results)
            }
            ContentType::Album => {
                let results = client
                    .search_albums(query, collection_params)
                    .await
                    .map_err(|e| {
                        StreamError::MetadataError(format!("SoundCloud search error: {}", e))
                    })?;

                let mut search_results = Vec::new();
                for item in results {
                    let mut tracks: Vec<TrackInfo> = item
                        .album_playlist
                        .tracks
                        .into_iter()
                        .filter_map(|tt| match tt {
                            TrackType::Basic(t) => Some(TrackInfo {
                                source: TrackSource::External {
                                    system: "soundcloud".to_string(),
                                    id: t.track.permalink_url,
                                },
                                title: t.track.title,
                                artist: t.user.username,
                                duration: Some(std::time::Duration::from_millis(
                                    t.track.duration as u64,
                                )),
                                thumbnail: t.track.artwork_url,
                                description: t.track.description,
                            }),
                            TrackType::Mini(_) => None,
                        })
                        .collect();

                    tracing::info!("SoundCloud Album tracks: {:?}", tracks);

                    search_results.push(SearchResult::Album(AlbumInfo {
                        id: item.album_playlist.permalink_url.clone(),
                        tracks,
                        url: item.album_playlist.permalink_url,
                        service: StreamingService::SoundCloud,
                        title: item.album_playlist.title,
                        artist: item.user.user.username,
                        track_count: item.album_playlist.track_count as usize,
                        year: None,
                        thumbnail: item.album_playlist.artwork_url,
                        description: item.album_playlist.description,
                    }));
                }
                Ok(search_results)
            }
            ContentType::Channel => Ok(Vec::new()),
        }
    }

    async fn expand_collection(&self, collection: SearchResult) -> StreamResult<Vec<TrackInfo>> {
        let url = match &collection {
            SearchResult::Playlist(info) => &info.url,
            SearchResult::Album(info) => &info.url,
            _ => return Err(StreamError::NotSupported),
        };

        let client = self.get_client().await?;
        let tracks = client
            .get_playlist_tracks(ResourceId::Url(url.clone()))
            .await
            .map_err(|e| StreamError::MetadataError(format!("SoundCloud API error: {}", e)))?;

        let track_infos: Vec<TrackInfo> = tracks
            .into_iter()
            .map(|t| TrackInfo {
                source: TrackSource::External {
                    system: "soundcloud".to_string(),
                    id: t.track.permalink_url,
                },
                title: t.track.title,
                artist: t.user.username,
                duration: Some(std::time::Duration::from_millis(t.track.duration as u64)),
                thumbnail: t.track.artwork_url,
                description: t.track.description,
            })
            .collect();

        println!("TRACKS LEN {:?}", track_infos.len());
        Ok(track_infos)
    }

    async fn is_available(&self) -> bool {
        true
    }
}
