use async_trait::async_trait;
use tokio::sync::RwLock;
use vk_api::{Playlist, VkClient};

use crate::audio::types::MediaSource;
use crate::library::TrackSource;
use crate::streaming::error::{StreamError, StreamResult};
use crate::streaming::provider::StreamProvider;
use crate::streaming::types::{
    ContentType, PlaylistInfo, ResolveOptions, ResolvedContent, SearchResult, StreamMetadata,
    StreamingService, TrackInfo,
};

pub struct VkProvider {
    client: RwLock<Option<VkClient>>,
}

impl VkProvider {
    pub fn new(token: Option<String>) -> Self {
        let client = token.map(VkClient::new);
        Self {
            client: RwLock::new(client),
        }
    }

    pub async fn update_token(&self, token: String) {
        let mut client = self.client.write().await;

        *client = Some(VkClient::new(token));
    }

    async fn get_client(&self) -> StreamResult<VkClient> {
        let client = self.client.read().await;
        client
            .clone()
            .ok_or_else(|| StreamError::Other("VK token not set".to_string()))
    }
}

#[async_trait]
impl StreamProvider for VkProvider {
    fn name(&self) -> &str {
        "VK"
    }

    async fn resolve(
        &self,
        source: &TrackSource,
        _options: Option<ResolveOptions>,
    ) -> StreamResult<String> {
        match source {
            TrackSource::External { id, system } if system == "vk" => {
                let client = self.get_client().await?;
                // В VK ID трека обычно в формате owner_id_audio_id
                let tracks = client
                    .get_by_id(vec![id.clone()])
                    .await
                    .map_err(|e| StreamError::Other(format!("VK API error: {}", e)))?;

                let track = tracks
                    .first()
                    .ok_or_else(|| StreamError::NoStreamAvailable("Track not found".to_string()))?;

                Ok(track.url.clone())
            }
            _ => Err(StreamError::NotSupported),
        }
    }

    async fn fetch_metadata(&self, source: &TrackSource) -> StreamResult<StreamMetadata> {
        match source {
            TrackSource::External { id, system } if system == "vk" => {
                let client = self.get_client().await?;
                let tracks = client
                    .get_by_id(vec![id.clone()])
                    .await
                    .map_err(|e| StreamError::Other(format!("VK API error: {}", e)))?;

                let track = tracks
                    .first()
                    .ok_or_else(|| StreamError::MetadataError("Track not found".to_string()))?;

                Ok(StreamMetadata {
                    title: Some(track.title.clone()),
                    artist: Some(track.artist.clone()),
                    album: None,
                    duration: Some(std::time::Duration::from_secs(track.duration as u64)),
                    artwork_url: None, // VK API может не отдавать обложки в этом методе без доп. полей
                    sample_rate: None,
                    channels: None,
                })
            }
            _ => Err(StreamError::NotSupported),
        }
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
        offset: usize,
    ) -> StreamResult<Vec<SearchResult>> {
        let client = self.get_client().await?;
        let content_type = content_type.unwrap_or(ContentType::Track);

        match content_type {
            ContentType::Track => {
                let results = client
                    .search_audio(query, limit as u32, offset as u32)
                    .await
                    .map_err(|e| StreamError::Other(format!("VK search error: {}", e)))?;

                let search_results = results
                    .into_iter()
                    .map(|track| {
                        SearchResult::Track(TrackInfo {
                            source: TrackSource::External {
                                system: "vk".to_string(),
                                id: track.audio_id(),
                            },
                            title: track.title,
                            artist: track.artist,
                            duration: Some(std::time::Duration::from_secs(track.duration as u64)),
                            thumbnail: None,
                            description: None,
                        })
                    })
                    .collect();
                Ok(search_results)
            }
            _ => Ok(Vec::new()), // Пока только поиск треков
        }
    }

    async fn expand_collection(&self, collection: SearchResult) -> StreamResult<Vec<TrackInfo>> {
        match collection {
            SearchResult::Playlist(info) => {
                let client = self.get_client().await?;
                let parts: Vec<&str> = info.id.split('_').collect();
                if parts.len() < 2 {
                    return Err(StreamError::Other("Invalid VK playlist ID".into()));
                }

                let owner_id = parts[0]
                    .parse()
                    .map_err(|_| StreamError::Other("Invalid owner_id".into()))?;
                let id = parts[1]
                    .parse()
                    .map_err(|_| StreamError::Other("Invalid album_id".into()))?;
                let access_key = if parts.len() > 2 {
                    parts[2].to_string()
                } else {
                    String::new()
                };

                let playlist = Playlist {
                    id,
                    owner_id,
                    title: info.title,
                    description: info.description.unwrap_or_default(),
                    count: info.track_count as u32,
                    create_time: 0,
                    update_time: 0,
                    access_key,
                    photo: None,
                };

                let vk_tracks = client
                    .get_all_playlist_tracks(&playlist)
                    .await
                    .map_err(|e| StreamError::Other(format!("VK error: {}", e)))?;

                let tracks = vk_tracks
                    .into_iter()
                    .map(|track| TrackInfo {
                        source: TrackSource::External {
                            system: "vk".to_string(),
                            id: track.audio_id(),
                        },
                        title: track.title,
                        artist: track.artist,
                        duration: Some(std::time::Duration::from_secs(track.duration as u64)),
                        thumbnail: None,
                        description: None,
                    })
                    .collect();

                Ok(tracks)
            }
            _ => Err(StreamError::NotSupported),
        }
    }

    async fn get_profile_content(&self) -> StreamResult<(Vec<TrackInfo>, Vec<PlaylistInfo>)> {
        tracing::info!("Fetching VK profile content...");
        let client = self.get_client().await.map_err(|e| {
            tracing::error!("Failed to get VK client: {:?}", e);
            e
        })?;

        let me = client.get_me().await.map_err(|e| {
            tracing::error!("VK get_me() error: {}", e);
            StreamError::Other(format!("VK error: {}", e))
        })?;
        let vk_tracks = client.get_audio(Some(me.id), 100, 0).await.map_err(|e| {
            tracing::error!("VK get_audio error: {}", e);
            StreamError::Other(format!("VK error: {}", e))
        })?;

        tracing::info!("Retrieved {} tracks from VK", vk_tracks.len());

        let vk_playlists = client.get_all_playlists(Some(me.id)).await.map_err(|e| {
            tracing::error!("VK get_all_playlists error: {}", e);
            StreamError::Other(format!("VK error: {}", e))
        })?;
        tracing::info!("Retrieved {} playlists from VK", vk_playlists.len());

        let tracks = vk_tracks
            .into_iter()
            .map(|track| TrackInfo {
                source: TrackSource::External {
                    system: "vk".to_string(),
                    id: track.audio_id(),
                },
                title: track.title,
                artist: track.artist,
                duration: Some(std::time::Duration::from_secs(track.duration as u64)),
                thumbnail: None,
                description: None,
            })
            .collect();

        let playlists = vk_playlists
            .into_iter()
            .map(|p| PlaylistInfo {
                id: format!("{}_{}", p.owner_id, p.id),
                url: format!("https://vk.com/audio?z=audio_playlist{}", p.full_id()),
                service: StreamingService::Vk,
                title: p.title,
                creator: "".to_string(),
                tracks: vec![],
                track_count: p.count as usize,
                thumbnail: p.photo.and_then(|ph| ph.photo_600),
                description: Some(p.description),
            })
            .collect();

        Ok((tracks, playlists))
    }

    async fn is_available(&self) -> bool {
        let client = self.client.read().await;
        client.is_some()
    }

    async fn set_credentials(&self, credentials: Option<String>) {
        let mut client = self.client.write().await;
        *client = credentials.map(VkClient::new);
    }
}
