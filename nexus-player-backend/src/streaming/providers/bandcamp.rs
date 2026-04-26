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
        _album_url: &str,
    ) -> Option<TrackInfo> {
        if !track.is_streamable {
            return None;
        }

        let duration = track
            .duration
            .map(|secs| std::time::Duration::from_secs_f32(secs));

        // Store "band_id:track_id" so resolve() can re-fetch fresh streaming URLs
        // via the Bandcamp mobile API directly, bypassing Cloudflare-protected stream_redirect.
        let id = format!("{}:{}", track.band_id, track.id);

        Some(TrackInfo {
            source: TrackSource::External {
                system: "bandcamp".to_string(),
                id,
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
        // New format: "band_id:track_id" — fetch track page URL via mobile API, then pass it
        // to detect_stream_type which will extract a direct CDN URL from the page HTML.
        // This avoids bandcamp.com/stream_redirect which is blocked by Cloudflare JS challenge.
        if let Some((band_id_str, track_id_str)) = url.split_once(':') {
            if let (Ok(band_id), Ok(track_id)) =
                (band_id_str.parse::<u64>(), track_id_str.parse::<u64>())
            {
                tracing::debug!(
                    "[bandcamp resolve] fetching via API band_id={} track_id={}",
                    band_id, track_id
                );
                let album = bandcamp::fetch_track(band_id, track_id)
                    .await
                    .map_err(|e| {
                        let msg = format!("{:?}", e);
                        if msg.contains("404") || msg.to_lowercase().contains("not found") {
                            StreamError::Unavailable("Track not found (404)".to_string())
                        } else if msg.contains("403") || msg.to_lowercase().contains("forbidden") {
                            StreamError::Unavailable("Track forbidden (403)".to_string())
                        } else {
                            StreamError::MetadataError(format!("Bandcamp API error: {:?}", e))
                        }
                    })?;

                let track = album.tracks.first().ok_or_else(|| {
                    StreamError::NoStreamAvailable("No tracks found in Bandcamp response".to_string())
                })?;

                let streaming_url = track.streaming_url.values().next().ok_or_else(|| {
                    StreamError::NoStreamAvailable("No streaming URL in Bandcamp API response".to_string())
                })?;

                tracing::debug!("[bandcamp resolve] band_id:track_id → stream url={}", streaming_url);

                return Ok((
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
                ));
            }
        }

        // Legacy: track page URL (e.g. https://artist.bandcamp.com/track/title)
        if url.contains("bandcamp.com") && !url.contains("stream_redirect") && !url.contains("bcbits.com") {
            tracing::debug!("[bandcamp resolve] legacy page url={}", url);
            let album = bandcamp::track_from_url(&url)
                .await
                .map_err(|e| {
                    let msg = format!("{:?}", e);
                    if msg.contains("404") || msg.to_lowercase().contains("not found") {
                        StreamError::Unavailable("Track not found (404)".to_string())
                    } else if msg.contains("403") || msg.to_lowercase().contains("forbidden") {
                        StreamError::Unavailable("Track forbidden (403)".to_string())
                    } else {
                        StreamError::MetadataError(format!("Bandcamp API error: {:?}", e))
                    }
                })?;

            let track = album.tracks.first().ok_or_else(|| {
                StreamError::NoStreamAvailable("No tracks found in Bandcamp response".to_string())
            })?;

            let streaming_url = track.streaming_url.values().next().ok_or_else(|| {
                StreamError::NoStreamAvailable("No streaming URL found".to_string())
            })?;

            return Ok((
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
            ));
        }

        // Legacy: raw stream_redirect or bcbits.com URL stored directly — pass through as-is.
        // These may fail if the token expired, but there's no way to re-resolve without band_id.
        if url.contains("stream_redirect") || url.contains("bcbits.com") {
            tracing::warn!("[bandcamp resolve] legacy streaming url (may be expired): {}", url);
            return Ok((
                url.clone(),
                StreamMetadata {
                    title: None,
                    artist: None,
                    album: None,
                    duration: None,
                    artwork_url: None,
                    sample_rate: None,
                    channels: None,
                },
            ));
        }

        tracing::warn!("[bandcamp resolve] unrecognized source id={}, cannot resolve", url);
        Err(StreamError::NoStreamAvailable(format!(
            "Cannot resolve Bandcamp source: {}",
            url
        )))
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

    async fn direct_link_resolve(&self, url: &str) -> StreamResult<SearchResult> {
        if !Self::is_bandcamp_url(url) {
            return Err(StreamError::NotSupported);
        }

        if url.contains("/track/") {
            let album = bandcamp::track_from_url(url).await.map_err(|e| {
                StreamError::MetadataError(format!("Bandcamp API error: {:?}", e))
            })?;

            let img_url = album.image.get_url();
            let track = album.tracks.into_iter().next().ok_or_else(|| {
                StreamError::NoStreamAvailable("No tracks found".to_string())
            })?;

            let mut info = Self::convert_track_to_track_info(track, url).ok_or_else(|| {
                StreamError::Unavailable("Track is not streamable".to_string())
            })?;

            if info.thumbnail.is_none() {
                info.thumbnail = img_url;
            }

            Ok(SearchResult::Track(info))
        } else {
            let album = bandcamp::album_from_url(url).await.map_err(|e| {
                StreamError::MetadataError(format!("Bandcamp API error: {:?}", e))
            })?;

            Ok(SearchResult::Album(AlbumInfo {
                id: url.to_string(),
                url: url.to_string(),
                service: StreamingService::Bandcamp,
                title: album.title,
                artist: album.band.name,
                tracks: vec![],
                track_count: album.tracks.len(),
                year: None,
                thumbnail: album.image.get_url(),
                description: None,
            }))
        }
    }

    async fn is_available(&self) -> bool {
        true
    }
}
