use async_trait::async_trait;

use crate::audio::types::MediaSource;
use crate::library::TrackSource;
use crate::streaming::error::{StreamError, StreamResult};
use crate::streaming::provider::StreamProvider;
use crate::streaming::types::{
    AlbumInfo, ContentType, ResolveOptions, ResolvedContent, SearchResult, StreamMetadata,
    StreamingService, TrackInfo,
};

pub struct BandcampProvider {}

impl BandcampProvider {
    pub fn new() -> Self {
        Self {}
    }

    fn is_bandcamp_url(url: &str) -> bool {
        url.contains("bandcamp.com")
    }

    fn convert_album_to_search_result(
        album: bandcamp::SearchResultItemAlbum,
    ) -> Option<SearchResult> {
        let id = album.url.item_url.clone();
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
        let duration = track
            .duration
            .map(|secs| std::time::Duration::from_secs_f32(secs));

        let urls = track
            .streaming_url
            .into_iter()
            .take(1)
            .collect::<Vec<(String, String)>>();

        if urls.is_empty() {
            return None;
        };
        let track_url = &urls[0].1;
        Some(TrackInfo {
            source: TrackSource::External {
                system: "bandcamp".to_string(),
                id: track_url.clone(),
            },
            title: track.title,
            artist: track.band_name,
            duration,
            thumbnail: track.image.get_url(),
            description: track.label,
        })
    }
}

#[async_trait]
impl StreamProvider for BandcampProvider {
    fn name(&self) -> &str {
        "Bandcamp"
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
        if url.contains("stream_redirect") {
            return Ok((
                url.clone(),
                StreamMetadata {
                    title: Some("Без названия".to_string()),
                    artist: Some("Неизвествен".to_string()),
                    album: None,
                    duration: None,
                    artwork_url: None,
                    sample_rate: None,
                    channels: None,
                },
            ));
        }

        let album = bandcamp::track_from_url(&url)
            .await
            .map_err(|e| StreamError::MetadataError(format!("Bandcamp API error: {:?}", e)))?;

        let track = album.tracks.first().ok_or_else(|| {
            StreamError::NoStreamAvailable("No tracks found in Bandcamp response".to_string())
        })?;

        let streaming_url =
            track.streaming_url.values().next().ok_or_else(|| {
                StreamError::NoStreamAvailable("No streaming URL found".to_string())
            })?;

        Ok((
            streaming_url.clone(),
            StreamMetadata {
                title: Some(track.title.clone()),
                artist: Some(album.band.name.clone()),
                album: Some(album.title.clone()),
                duration: None,
                artwork_url: album.image.get_url(),
                sample_rate: None,
                channels: None,
            },
        ))
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
        tracing::info!("expand collection {collection:?}");
        match collection {
            SearchResult::Track(t) => Ok(vec![t]),
            SearchResult::Album(a) => {
                let url = &a.url;
                let album = bandcamp::album_from_url(url).await.map_err(|e| {
                    StreamError::MetadataError(format!("Bandcamp API error: {:?}", e))
                })?;
                let img_url = album.image.get_url();
                let track_sources: Vec<TrackInfo> = album
                    .tracks
                    .into_iter()
                    .filter_map(|track| Self::convert_track_to_track_info(track, url))
                    .map(|mut track| {
                        if track.thumbnail.is_none() {
                            track.thumbnail = img_url.clone();
                        }
                        track
                    })
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
