use async_trait::async_trait;
use bandcamp::Album;
use std::sync::Arc;

use crate::audio::types::MediaSource;
use crate::library::TrackSource;
use crate::streaming::cache::CacheManager;
use crate::streaming::error::{StreamError, StreamResult};
use crate::streaming::provider::StreamProvider;
use crate::streaming::types::{
    AlbumInfo, ContentType, ResolvedContent, SearchResult, StreamMetadata, StreamingService,
    TrackInfo,
};

pub struct BandcampProvider {
    ytdlp: Arc<dyn StreamProvider>,
}

impl BandcampProvider {
    pub fn new(ytdlp: Arc<dyn StreamProvider>) -> Self {
        Self { ytdlp }
    }

    fn is_bandcamp_url(url: &str) -> bool {
        url.contains("bandcamp.com")
    }

    fn convert_album_to_search_result(
        album: bandcamp::SearchResultItemAlbum,
    ) -> Option<SearchResult> {
        let id = album
            .url
            .item_url
            .split('/')
            .last()
            .unwrap_or("unknown")
            .to_string();
        let url = album.url.item_url.clone();
        let thumbnail = album.image.get_url();

        Some(SearchResult::Album(AlbumInfo {
            id,
            url,
            service: StreamingService::Bandcamp,
            title: album.name,
            artist: album.band_name,
            tracks: vec![],
            track_count: 0,
            year: None,
            thumbnail,
            description: None,
        }))
    }

    fn convert_track_to_track_info(
        track: bandcamp::AlbumTrack,
        album_url: &str,
    ) -> Option<TrackInfo> {
        let base_url = if let Some(pos) = album_url.rfind("/album/") {
            &album_url[..pos]
        } else if let Some(pos) = album_url.rfind("/track/") {
            &album_url[..pos]
        } else {
            return None;
        };

        let track_slug = track
            .title
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .map(|c| if c.is_whitespace() { '-' } else { c })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-");

        let track_url = format!("{}/track/{}", base_url, track_slug);

        let duration = track
            .duration
            .map(|secs| std::time::Duration::from_secs_f32(secs));

        Some(TrackInfo {
            source: TrackSource::External {
                system: "bandcamp".to_string(),
                id: track_url.clone(),
            },
            title: track.title,
            artist: "Unknown Artist".to_string(),
            duration,
            thumbnail: None,
            description: None,
        })
    }
}

#[async_trait]
impl StreamProvider for BandcampProvider {
    fn name(&self) -> &str {
        "Bandcamp"
    }

    async fn resolve(&self, source: &TrackSource) -> StreamResult<String> {
        match source {
            TrackSource::Http(_) => self.ytdlp.resolve(source).await,
            TrackSource::External { id: _, system: _ } => self.ytdlp.resolve(source).await,
            _ => Err(StreamError::NotSupported),
        }
    }

    async fn fetch_metadata(&self, source: &TrackSource) -> StreamResult<StreamMetadata> {
        let url = match source {
            TrackSource::Http(url) => url.as_str(),
            TrackSource::External { id, .. } => id.as_str(),
            _ => {
                return Err(StreamError::InvalidSource(
                    "Invalid source type for Bandcamp".to_string(),
                ))
            }
        };

        let album = bandcamp::album_from_url(url)
            .await
            .map_err(|e| StreamError::MetadataError(format!("Bandcamp API error: {:?}", e)))?;

        let duration = None;
        // album
        // .duration
        // .map(|secs| std::time::Duration::from_secs_f32(secs));

        Ok(StreamMetadata {
            title: Some(album.title),
            artist: Some(album.band.name),
            album: None,
            duration,
            artwork_url: (album.image.get_url()),
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
        let results = bandcamp::search(query)
            .await
            .map_err(|e| StreamError::MetadataError(format!("Bandcamp search error: {:?}", e)))?;

        let mut search_results = Vec::new();

        for item in results.into_iter().take(limit) {
            match item {
                bandcamp::SearchResultItem::Album(album) => {
                    if content_type.is_none() || content_type == Some(ContentType::Album) {
                        if let Some(result) = Self::convert_album_to_search_result(album) {
                            search_results.push(result);
                        }
                    }
                }
                bandcamp::SearchResultItem::Track(track) => {
                    if content_type.is_none() || content_type == Some(ContentType::Track) {
                        let track_url = track.url.item_url.clone();
                        let track_info = TrackInfo {
                            source: TrackSource::External {
                                system: "bandcamp".to_string(),
                                id: track_url,
                            },
                            title: track.name,
                            artist: track.band_name,
                            duration: None,
                            thumbnail: None,
                            description: None,
                        };

                        search_results.push(SearchResult::Track(track_info));
                    }
                }
                _ => {}
            }
        }

        Ok(search_results)
    }

    async fn expand_collection(&self, collection: SearchResult) -> StreamResult<Vec<TrackInfo>> {
        match collection {
            SearchResult::Track(t) => Ok(vec![t]),
            SearchResult::Album(a) => {
                let url = &a.url;
                let album = bandcamp::album_from_url(url).await.map_err(|e| {
                    StreamError::MetadataError(format!("Bandcamp API error: {:?}", e))
                })?;
                tracing::info!("IA EXPANDED{:?}", album);

                let track_sources: Vec<TrackInfo> = album
                    .tracks
                    .into_iter()
                    .filter_map(|track| Self::convert_track_to_track_info(track, url))
                    .collect();

                Ok(track_sources)
            }
            _ => todo!(),
        }
    }

    async fn is_available(&self) -> bool {
        true
    }
}
