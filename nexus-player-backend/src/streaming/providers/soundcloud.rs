use async_trait::async_trait;
use rsoundcloud::models::playlist::TrackType;
use tokio::sync::RwLock;

use rsoundcloud::models::SearchItem;
use rsoundcloud::{MiscApi, PlaylistsApi, ResourceId, SearchApi, SoundCloudClient, TracksApi};

use crate::audio::types::MediaSource;
use crate::library::TrackSource;
use crate::streaming::error::{StreamError, StreamResult};
use crate::streaming::provider::StreamProvider;
use crate::streaming::types::{
    AlbumInfo, ContentType, PlaylistInfo, ResolveOptions, ResolvedContent, SearchResult,
    StreamMetadata, TrackInfo,
};

pub struct SoundCloudProvider {
    client: RwLock<Option<SoundCloudClient>>,
}

impl SoundCloudProvider {
    pub fn new(client_id: Option<String>) -> Self {
        let client = client_id.map(SoundCloudClient::with_client_id);
        Self {
            client: RwLock::new(client),
        }
    }

    async fn get_client(&self) -> StreamResult<SoundCloudClient> {
        let client = self.client.read().await;
        if let Some(c) = &*client {
            return Ok(c.clone());
        }
        drop(client);

        let mut client_lock = self.client.write().await;
        if let Some(c) = &*client_lock {
            return Ok(c.clone());
        }

        let new_client = SoundCloudClient::default().await.map_err(|e| {
            let msg = format!("{}", e);
            if Self::is_auth_error(&msg) || msg.contains("client id generation failed") {
                StreamError::ChallengeRequired {
                    provider: "soundcloud".to_string(),
                    url: "https://soundcloud.com".to_string(),
                }
            } else {
                StreamError::Other(format!("Failed to create SoundCloud client: {}", e))
            }
        })?;

        *client_lock = Some(new_client.clone());
        Ok(new_client)
    }

    pub async fn set_client_id(&self, client_id: String) {
        tracing::info!(
            "SoundCloudProvider: updating client_id (len={})",
            client_id.len()
        );
        let mut client_lock = self.client.write().await;
        if let Some(c) = &mut *client_lock {
            c.set_client_id(client_id);
        } else {
            *client_lock = Some(SoundCloudClient::with_client_id(client_id));
        }
    }

    fn is_soundcloud_url(url: &str) -> bool {
        url.contains("soundcloud.com")
    }

    /// 401 = stale/invalid client_id, 403 = blocked; both are fixed by re-solving the challenge.
    fn is_auth_error(msg: &str) -> bool {
        let lower = msg.to_lowercase();
        msg.contains("401")
            || msg.contains("403")
            || lower.contains("unauthorized")
            || lower.contains("forbidden")
    }

    fn map_error(&self, e: impl std::fmt::Display, url: String) -> StreamError {
        let msg = format!("{}", e);
        if msg.contains("404") || msg.to_lowercase().contains("not found") {
            StreamError::Unavailable("Track not found (404)".to_string())
        } else if Self::is_auth_error(&msg) {
            StreamError::ChallengeRequired {
                provider: "soundcloud".to_string(),
                url,
            }
        } else {
            StreamError::MetadataError(format!("SoundCloud API error: {}", e))
        }
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
            .get_track(ResourceId::Url(url.clone()))
            .await
            .map_err(|e| self.map_error(e, url.clone()))?;

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
            } else if Self::is_auth_error(&msg) {
                StreamError::ChallengeRequired {
                    provider: "soundcloud".to_string(),
                    url,
                }
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
                duration: Some(std::time::Duration::from_millis(
                    track.track.duration as u64,
                )),
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
            .get_track(ResourceId::Url(url.clone()))
            .await
            .map_err(|e| self.map_error(e, url))?;

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
                        self.map_error(e, "https://soundcloud.com/search/tracks".to_string())
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
                        self.map_error(e, "https://soundcloud.com/search/playlists".to_string())
                    })?;

                let mut search_results = Vec::new();
                for item in results {
                    let tracks: Vec<TrackInfo> = item
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
                        service: crate::streaming::types::StreamingService::SoundCloud,
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
                        self.map_error(e, "https://soundcloud.com/search/albums".to_string())
                    })?;

                let mut search_results = Vec::new();
                for item in results {
                    let tracks: Vec<TrackInfo> = item
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
                        service: crate::streaming::types::StreamingService::SoundCloud,
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
            .map_err(|e| self.map_error(e, url.to_string()))?;

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

        tracing::debug!("SoundCloud returned {} tracks", track_infos.len());
        Ok(track_infos)
    }

    async fn direct_link_resolve(&self, url: &str) -> StreamResult<SearchResult> {
        if !Self::is_soundcloud_url(url) {
            return Err(StreamError::NotSupported);
        }

        let client = self.get_client().await?;
        let item = client
            .resolve(url)
            .await
            .map_err(|e| self.map_error(e, url.to_string()))?;

        match item {
            SearchItem::Track(track) => Ok(SearchResult::Track(TrackInfo {
                source: TrackSource::External {
                    system: "soundcloud".to_string(),
                    id: track.track.permalink_url.clone(),
                },
                title: track.track.title,
                artist: track.user.username,
                duration: Some(std::time::Duration::from_millis(
                    track.track.duration as u64,
                )),
                thumbnail: track.track.artwork_url,
                description: track.track.description,
            })),
            SearchItem::AlbumPlaylist(item) => {
                let tracks: Vec<TrackInfo> = item
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

                if item.album_playlist.is_album {
                    Ok(SearchResult::Album(AlbumInfo {
                        id: item.album_playlist.permalink_url.clone(),
                        url: item.album_playlist.permalink_url,
                        service: crate::streaming::types::StreamingService::SoundCloud,
                        title: item.album_playlist.title,
                        artist: item.user.user.username,
                        tracks,
                        track_count: item.album_playlist.track_count as usize,
                        year: None,
                        thumbnail: item.album_playlist.artwork_url,
                        description: item.album_playlist.description,
                    }))
                } else {
                    Ok(SearchResult::Playlist(PlaylistInfo {
                        id: item.album_playlist.permalink_url.clone(),
                        url: item.album_playlist.permalink_url,
                        service: crate::streaming::types::StreamingService::SoundCloud,
                        title: item.album_playlist.title,
                        creator: item.user.user.username,
                        tracks,
                        track_count: item.album_playlist.track_count as usize,
                        thumbnail: item.album_playlist.artwork_url,
                        description: item.album_playlist.description,
                    }))
                }
            }
            SearchItem::User(_) => Err(StreamError::NotSupported),
        }
    }

    async fn is_available(&self) -> bool {
        true
    }

    async fn set_credentials(&self, credentials: Option<String>) {
        if let Some(client_id) = credentials {
            self.set_client_id(client_id).await;
        }
    }
}
