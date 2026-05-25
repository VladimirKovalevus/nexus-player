use nexus_player_backend::audio::types::TrackState;
use serde::{Deserialize, Serialize};

pub use nexus_player_backend::library::types::{ArtworkId, CacheStatus, PlaylistId, TrackId};
use nexus_player_backend::streaming::types as streaming;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackDto {
    pub id: i64,
    pub title: String,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub duration_secs: Option<f64>,
    #[serde(default)]
    pub artwork_url: Option<String>,
    #[serde(default)]
    pub play_count: u64,
    pub liked: bool,
    pub state: TrackStateDto,
    #[serde(default)]
    pub source: Option<TrackSourceDto>,
    #[serde(default)]
    pub unavailable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedAlbumDto {
    pub id: i64,
    pub title: String,
    pub artist: Option<String>,
    pub artwork_url: Option<String>,
    pub cached_track_count: u64,
}

impl From<models::CachedAlbumSummary> for CachedAlbumDto {
    fn from(summary: models::CachedAlbumSummary) -> Self {
        let artwork_url = summary.artwork.as_ref().and_then(|art| match &art.source {
            nexus_player_backend::library::types::ArtworkSource::Url { url } => Some(url.clone()),
            _ => None,
        });

        Self {
            id: summary.id.0,
            title: summary.title,
            artist: summary.artist,
            artwork_url,
            cached_track_count: summary.cached_track_count,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistDto {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub creator: Option<String>,
    pub thumbnail: Option<String>,
    pub track_count: usize,
    pub created_at: i64,
    pub updated_at: i64,
    pub source: Option<TrackSourceDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistWithTracksDto {
    pub playlist: PlaylistDto,
    pub tracks: Vec<TrackDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "itemType")]
pub enum PlaylistItemDto {
    #[serde(rename = "track")]
    Track {
        id: i64,
        item_id: i64,
        track: TrackDto,
        position: u32,
    },
    #[serde(rename = "playlist")]
    Playlist {
        id: i64,
        item_id: i64,
        playlist: PlaylistDto,
        position: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistWithItemsDto {
    pub playlist: PlaylistDto,
    pub items: Vec<PlaylistItemDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerMetaDto {
    pub kind: String,
    pub id: String,
    pub system: String,
    pub title: String,
    pub artist: String,
    pub track_count: usize,
    pub thumbnail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "entryType")]
pub enum QueueEntryDto {
    #[serde(rename = "track")]
    Track { track: TrackDto },
    #[serde(rename = "container")]
    Container {
        meta: ContainerMetaDto,
        tracks: Option<Vec<TrackDto>>,
        play_index: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentPositionDto {
    pub item_index: usize,
    pub track_within_container: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchContextDto {
    pub system: String,
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPageDto {
    pub current_page: usize,
    pub total_pages: usize,
    pub page_size: usize,
    pub total_tracks: usize,
    pub track_index_in_page: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueStateDto {
    pub entries: Vec<QueueEntryDto>,
    pub current_position: Option<CurrentPositionDto>,
    pub repeat_mode: String,
    pub shuffle_enabled: bool,
    pub search_context: Option<SearchContextDto>,
    /// "tracks" | "playlists" when a radio session is loaded, otherwise None.
    pub radio_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupResultDto {
    pub deleted_tracks: u64,
    pub deleted_artworks: u64,
}

/// One entry of an artwork batch resolution.
///
/// `path` is the local file to render, or `None` while the cover is still being
/// fetched — the UI keeps showing the remote `url` until then.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedArtworkDto {
    pub url: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioStateDto {
    pub state: String,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub volume: f32,
    pub current_track: Option<TrackDto>,
    pub buffered_ranges: Vec<BufferedRangeDto>,
    pub waveforms: Vec<WaveformDto>,
    pub fft: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaveformDto {
    pub data: Vec<f32>,
    pub start_time: f64,
    pub end_time: f64,
    pub byte_start: Option<u64>,
    pub byte_end: Option<u64>,
}

impl From<nexus_player_backend::audio::types::Waveform> for WaveformDto {
    fn from(w: nexus_player_backend::audio::types::Waveform) -> Self {
        Self {
            data: w.data,
            start_time: w.start_time,
            end_time: w.end_time,
            byte_start: w.byte_start,
            byte_end: w.byte_end,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BufferedRangeDto {
    pub start: f32,
    pub end: f32,
}

impl From<nexus_player_backend::audio::types::BufferedRange> for BufferedRangeDto {
    fn from(range: nexus_player_backend::audio::types::BufferedRange) -> Self {
        Self {
            start: range.start,
            end: range.end,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTrackRequest {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub source_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePlaylistRequest {
    pub title: String,
    pub description: Option<String>,
    pub creator: Option<String>,
    pub artwork_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePlaylistWithTracksRequest {
    pub title: String,
    pub description: Option<String>,
    pub creator: Option<String>,
    pub artwork_url: Option<String>,
    pub tracks: Vec<TrackDto>,
}

impl TrackDto {
    pub fn try_to_audio_track(&self) -> Result<nexus_player_backend::audio::types::Track, String> {
        use nexus_player_backend::audio::types::{Track, TrackSource, TrackState};

        let src = self
            .source
            .as_ref()
            .ok_or_else(|| format!("Track '{}' has no source", self.title))?;
        let source = match src.system.as_str() {
            "local" => TrackSource::Local(src.url.clone().into()),
            "http" => TrackSource::Http(src.url.clone()),
            // A cached source is already a playable local file. TrackSourceDto does not carry
            // original_url, so preserve playback correctness by treating it as local here.
            "cache" => TrackSource::Local(src.url.clone().into()),
            _ => TrackSource::External {
                id: src.url.clone(),
                system: src.system.clone(),
            },
        };

        let state = match &self.state {
            TrackStateDto::Saved => TrackState::Saved,
            TrackStateDto::Streaming => TrackState::Streaming,
            TrackStateDto::Cached => TrackState::Cached,
        };

        Ok(Track {
            id: self.id as usize,
            title: self.title.clone(),
            artist: self.artist.clone(),
            album: self.album.clone(),
            duration: self.duration_secs.map(std::time::Duration::from_secs_f64),
            artwork_url: self.artwork_url.clone(),
            format: None,
            source,
            state,
            liked: self.liked,
        })
    }
}

use nexus_player_backend::audio::types::{self as audio};
use nexus_player_backend::library::models;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayHistoryEntryDto {
    pub id: i64,
    pub title: String,
    pub artist: Option<String>,
    pub source: TrackSourceDto,
    pub artwork_url: Option<String>,
    pub played_at: i64,
}

impl From<models::PlayHistoryEntry> for PlayHistoryEntryDto {
    fn from(entry: models::PlayHistoryEntry) -> Self {
        Self {
            id: entry.id,
            title: entry.title,
            artist: entry.artist,
            source: entry.source.into(),
            artwork_url: entry.artwork_url,
            played_at: entry.played_at,
        }
    }
}

impl From<models::Track> for TrackDto {
    fn from(track: models::Track) -> Self {
        let artwork_url = track
            .metadata
            .artwork
            .as_ref()
            .and_then(|art| match &art.source {
                nexus_player_backend::library::types::ArtworkSource::Url { url } => {
                    Some(url.clone())
                }
                _ => None,
            });

        let state = match track.cache {
            CacheStatus::Cached { .. } => TrackStateDto::Cached,
            _ => TrackStateDto::Saved,
        };

        Self {
            id: track.id.0,
            title: track.metadata.title,
            artist: track.artist,
            album: track.album,
            duration_secs: track
                .audio_info
                .as_ref()
                .and_then(|info| info.duration_secs),
            artwork_url,
            play_count: track.play_count,
            liked: track.liked,
            state,
            source: Some(track.source.into()),
            unavailable: track.unavailable,
        }
    }
}

impl From<audio::Track> for TrackDto {
    fn from(track: audio::Track) -> Self {
        Self {
            id: track.id as i64,
            title: track.title,
            artist: track.artist,
            album: track.album,
            duration_secs: track.duration.map(|d| d.as_secs_f64()),
            artwork_url: track.artwork_url,
            play_count: 0,
            liked: track.liked,
            state: track.state.into(),
            source: Some(track.source.into()),
            unavailable: false,
        }
    }
}

impl From<&audio::Track> for TrackDto {
    fn from(track: &audio::Track) -> Self {
        Self {
            id: track.id as i64,
            title: track.title.clone(),
            artist: track.artist.clone(),
            album: track.album.clone(),
            duration_secs: track.duration.map(|d| d.as_secs_f64()),
            artwork_url: track.artwork_url.clone(),
            play_count: 0,
            liked: track.liked,
            state: track.state.clone().into(),
            source: Some(track.source.clone().into()),
            unavailable: false,
        }
    }
}

impl From<models::Playlist> for PlaylistDto {
    fn from(playlist: models::Playlist) -> Self {
        let thumbnail = playlist
            .metadata
            .artwork
            .as_ref()
            .and_then(|art| match &art.source {
                nexus_player_backend::library::types::ArtworkSource::Url { url } => {
                    Some(url.clone())
                }
                _ => None,
            });

        Self {
            id: playlist.id.0,
            title: playlist.metadata.title,
            description: playlist.metadata.description,
            creator: playlist.creator,
            thumbnail,
            track_count: 0,
            created_at: playlist.created_at.timestamp(),
            updated_at: playlist.updated_at.timestamp(),
            source: None,
        }
    }
}

impl From<audio::AudioState> for AudioStateDto {
    fn from(state: audio::AudioState) -> Self {
        let state_str = match state.playback_state {
            audio::PlaybackState::Playing => "playing",
            audio::PlaybackState::Paused => "paused",
            audio::PlaybackState::Stopped => "stopped",
            audio::PlaybackState::Buffering => "buffering",
            audio::PlaybackState::Loading => "loading",
        };

        Self {
            state: state_str.to_string(),
            position_secs: state.position.as_secs_f64(),
            duration_secs: state.duration.as_secs_f64(),
            volume: state.volume,
            current_track: state.current_track.map(|t| t.into()),
            buffered_ranges: state
                .buffered_ranges
                .into_iter()
                .map(BufferedRangeDto::from)
                .collect(),
            waveforms: state.waveforms.into_iter().map(WaveformDto::from).collect(),
            fft: state.fft,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum SearchResultDto {
    Track(StreamingTrackDto),
    Playlist(StreamingPlaylistDto),
    Album(StreamingAlbumDto),
    Channel(StreamingChannelDto),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackSourceDto {
    pub url: String,
    pub system: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum TrackStateDto {
    Saved,
    Streaming,
    Cached,
}
impl From<TrackState> for TrackStateDto {
    fn from(value: TrackState) -> Self {
        match value {
            TrackState::Saved => TrackStateDto::Saved,
            TrackState::Streaming => TrackStateDto::Streaming,
            TrackState::Cached => TrackStateDto::Cached,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamingTrackDto {
    pub id: Option<i64>,
    pub title: String,
    pub artist: String,
    pub duration_secs: Option<f64>,
    pub thumbnail: Option<String>,
    pub source: TrackSourceDto,
    pub state: TrackStateDto,
    pub liked: bool,
}

impl StreamingTrackDto {
    pub fn to_audio_track(&self) -> nexus_player_backend::audio::types::Track {
        use nexus_player_backend::audio::types::{Track, TrackSource, TrackState};

        Track {
            id: hash_str_to_i64(&self.source.url) as usize,
            title: self.title.clone(),
            artist: Some(self.artist.clone()),
            album: None,
            duration: self.duration_secs.map(std::time::Duration::from_secs_f64),
            artwork_url: self.thumbnail.clone(),
            format: None,
            source: TrackSource::External {
                id: self.source.url.clone(),
                system: self.source.system.clone(),
            },
            state: TrackState::Streaming,
            liked: self.liked,
        }
    }
}

impl From<nexus_player_backend::audio::types::TrackSource> for TrackSourceDto {
    fn from(track_source: nexus_player_backend::audio::types::TrackSource) -> Self {
        match track_source {
            nexus_player_backend::audio::types::TrackSource::Local(path) => Self {
                url: path.display().to_string(),
                system: "local".to_string(),
            },
            nexus_player_backend::audio::types::TrackSource::Http(url) => Self {
                url,
                system: "http".to_string(),
            },
            nexus_player_backend::audio::types::TrackSource::External { id, system } => {
                Self { url: id, system }
            }
            nexus_player_backend::audio::types::TrackSource::Cached { cache_path, .. } => Self {
                url: cache_path.display().to_string(),
                system: "cache".to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamingPlaylistDto {
    pub id: String,
    pub title: String,
    pub creator: String,
    pub track_count: usize,
    pub thumbnail: Option<String>,
    pub tracks: Vec<StreamingTrackDto>,
    pub url: String,
    pub system: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamingAlbumDto {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub track_count: usize,
    pub year: Option<u32>,
    pub thumbnail: Option<String>,
    pub tracks: Vec<StreamingTrackDto>,
    pub url: String,
    pub system: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamingChannelDto {
    pub id: String,
    pub name: String,
    pub subscriber_count: Option<u64>,
    pub thumbnail: Option<String>,
    pub url: String,
    pub system: String,
}

impl From<streaming::SearchResult> for SearchResultDto {
    fn from(result: streaming::SearchResult) -> Self {
        match result {
            streaming::SearchResult::Track(track) => {
                let (url, system) = match &track.source {
                    nexus_player_backend::library::TrackSource::External { id, system } => {
                        (id.clone(), system.clone())
                    }
                    _ => (track.source.as_str(), "unknown".to_string()),
                };

                SearchResultDto::Track(StreamingTrackDto {
                    id: Some(hash_str_to_i64(&url)),
                    title: track.title,
                    artist: track.artist,
                    duration_secs: track.duration.map(|d| d.as_secs_f64()),
                    thumbnail: track.thumbnail,
                    source: TrackSourceDto { url, system },
                    state: TrackStateDto::Streaming,
                    liked: false,
                })
            }
            streaming::SearchResult::Playlist(playlist) => {
                SearchResultDto::Playlist(StreamingPlaylistDto {
                    id: playlist.id,
                    title: playlist.title,
                    creator: playlist.creator,
                    track_count: playlist.track_count,
                    thumbnail: playlist.thumbnail,
                    tracks: playlist.tracks.into_iter().map(|el| el.into()).collect(),
                    url: playlist.url,
                    system: playlist.service.to_string(),
                })
            }
            streaming::SearchResult::Album(album) => SearchResultDto::Album(StreamingAlbumDto {
                id: album.id,
                title: album.title,
                artist: album.artist,
                track_count: album.track_count,
                year: album.year,
                thumbnail: album.thumbnail,
                tracks: album.tracks.into_iter().map(|el| el.into()).collect(),
                url: album.url,
                system: album.service.to_string(),
            }),
            streaming::SearchResult::Channel(channel) => {
                SearchResultDto::Channel(StreamingChannelDto {
                    id: channel.id,
                    name: channel.name,
                    subscriber_count: channel.subscriber_count,
                    thumbnail: channel.thumbnail,
                    url: channel.url,
                    system: channel.service.to_string(),
                })
            }
        }
    }
}
impl From<streaming::TrackInfo> for StreamingTrackDto {
    fn from(track: streaming::TrackInfo) -> Self {
        let (url, system) = match &track.source {
            nexus_player_backend::library::TrackSource::External { id, system } => {
                (id.clone(), system.clone())
            }
            _ => (track.source.as_str(), "unknown".to_string()),
        };

        Self {
            id: Some(hash_str_to_i64(&url)),
            title: track.title,
            artist: track.artist,
            duration_secs: track.duration.map(|d| d.as_secs_f64()),
            thumbnail: track.thumbnail,
            source: TrackSourceDto { url, system },
            state: TrackStateDto::Streaming,
            liked: false,
        }
    }
}

impl From<SearchResultDto> for streaming::SearchResult {
    fn from(dto: SearchResultDto) -> Self {
        match dto {
            SearchResultDto::Track(track) => streaming::SearchResult::Track(streaming::TrackInfo {
                source: nexus_player_backend::library::TrackSource::External {
                    id: track.source.url,
                    system: track.source.system,
                },
                title: track.title,
                artist: track.artist,
                duration: track.duration_secs.map(std::time::Duration::from_secs_f64),
                thumbnail: track.thumbnail,
                description: None,
            }),
            SearchResultDto::Playlist(playlist) => {
                let service = std::str::FromStr::from_str(&playlist.system)
                    .unwrap_or(streaming::StreamingService::Bandcamp);
                streaming::SearchResult::Playlist(streaming::PlaylistInfo {
                    id: playlist.id,
                    url: playlist.url,
                    service,
                    title: playlist.title,
                    creator: playlist.creator,
                    tracks: playlist
                        .tracks
                        .into_iter()
                        .map(|t| streaming::TrackInfo {
                            source: nexus_player_backend::library::TrackSource::External {
                                id: t.source.url,
                                system: t.source.system,
                            },
                            title: t.title,
                            artist: t.artist,
                            duration: t.duration_secs.map(std::time::Duration::from_secs_f64),
                            thumbnail: t.thumbnail,
                            description: None,
                        })
                        .collect(),
                    track_count: playlist.track_count,
                    thumbnail: playlist.thumbnail,
                    description: None,
                })
            }
            SearchResultDto::Album(album) => {
                let service = std::str::FromStr::from_str(&album.system)
                    .unwrap_or(streaming::StreamingService::Bandcamp);
                streaming::SearchResult::Album(streaming::AlbumInfo {
                    id: album.id,
                    url: album.url,
                    service,
                    title: album.title,
                    artist: album.artist,
                    tracks: album
                        .tracks
                        .into_iter()
                        .map(|t| streaming::TrackInfo {
                            source: nexus_player_backend::library::TrackSource::External {
                                id: t.source.url,
                                system: t.source.system,
                            },
                            title: t.title,
                            artist: t.artist,
                            duration: t.duration_secs.map(std::time::Duration::from_secs_f64),
                            thumbnail: t.thumbnail,
                            description: None,
                        })
                        .collect(),
                    track_count: album.track_count,
                    year: album.year,
                    thumbnail: album.thumbnail,
                    description: None,
                })
            }
            SearchResultDto::Channel(channel) => {
                let service = std::str::FromStr::from_str(&channel.system)
                    .unwrap_or(streaming::StreamingService::Bandcamp);
                streaming::SearchResult::Channel(streaming::ChannelInfo {
                    id: channel.id,
                    url: channel.url,
                    service,
                    name: channel.name,
                    subscriber_count: channel.subscriber_count,
                    track_count: None,
                    thumbnail: channel.thumbnail,
                    description: None,
                })
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskInfoDto {
    pub track_id: u64,
    pub url: String,
    pub state: String,
    pub size_human: String,
    pub byte_size: u64,
    pub progress: f64,
    pub meta: Option<StreamingTrackDto>,
}

impl From<::streaming::state::types::TaskInfo> for TaskInfoDto {
    fn from(info: ::streaming::state::types::TaskInfo) -> Self {
        let meta = info.meta.map(|m| {
            let (url, system) = (info.url.clone(), "unknown".to_string()); // We don't have system here, but TaskInfo usually has URL
            StreamingTrackDto {
                id: Some(hash_str_to_i64(&url)),
                title: m.title.unwrap_or_default(),
                artist: m.artist.unwrap_or_default(),
                duration_secs: m.duration.map(|d| d.as_secs_f64()),
                thumbnail: m.artwork_url,
                source: TrackSourceDto { url, system },
                state: TrackStateDto::Streaming,
                liked: false,
            }
        });

        Self {
            track_id: info.track_id,
            url: info.url,
            state: info.state,
            size_human: info.size_human,
            byte_size: info.byte_size,
            progress: info.progress,
            meta,
        }
    }
}

pub(crate) fn hash_str_to_i64(s: &str) -> i64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut hasher);
    (hasher.finish() & i64::MAX as u64) as i64
}

impl From<nexus_player_backend::streaming::types::TrackInfo> for TrackDto {
    fn from(track: nexus_player_backend::streaming::types::TrackInfo) -> Self {
        let (url, system) = match &track.source {
            nexus_player_backend::library::TrackSource::External { id, system } => {
                (id.clone(), system.clone())
            }
            _ => (track.source.as_str(), "unknown".to_string()),
        };

        Self {
            id: hash_str_to_i64(&track.source.as_str()),
            unavailable: false,
            title: track.title,
            artist: Some(track.artist),
            album: None,
            duration_secs: track.duration.map(|d| d.as_secs_f64()),
            artwork_url: track.thumbnail,
            play_count: 0,
            liked: false,
            state: TrackStateDto::Streaming,
            source: Some(TrackSourceDto { url, system }),
        }
    }
}

#[cfg(test)]
mod local_source_tests {
    use super::*;
    use nexus_player_backend::audio::types::TrackSource;

    fn dto(source: Option<TrackSourceDto>) -> TrackDto {
        TrackDto {
            id: 42,
            title: "Local track".into(),
            artist: None,
            album: None,
            duration_secs: None,
            artwork_url: None,
            play_count: 0,
            liked: false,
            state: TrackStateDto::Saved,
            source,
            unavailable: false,
        }
    }

    #[test]
    fn local_source_round_trips_through_dto() {
        let expected = TrackSource::Local("/music/track.flac".into());
        let source_dto: TrackSourceDto = expected.clone().into();
        assert_eq!(source_dto.system, "local");
        assert_eq!(source_dto.url, "/music/track.flac");

        let runtime = dto(Some(source_dto)).try_to_audio_track().unwrap();
        assert_eq!(runtime.source, expected);
    }

    #[test]
    fn missing_source_is_rejected() {
        assert!(dto(None).try_to_audio_track().is_err());
    }
}

impl From<nexus_player_backend::streaming::types::PlaylistInfo> for PlaylistDto {
    fn from(playlist: nexus_player_backend::streaming::types::PlaylistInfo) -> Self {
        Self {
            id: hash_str_to_i64(&playlist.id),
            title: playlist.title,
            description: playlist.description,
            creator: Some(playlist.creator),
            thumbnail: playlist.thumbnail,
            track_count: playlist.track_count,
            created_at: 0,
            updated_at: 0,
            source: Some(TrackSourceDto {
                url: playlist.id,
                system: playlist.service.to_string(),
            }),
        }
    }
}

// ─── EQ DTOs ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EqSettingsDto {
    pub enabled: bool,
    pub gains: Vec<f32>,
    pub preset_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EqPresetDto {
    pub name: String,
    pub gains: Vec<f32>,
}

impl From<nexus_player_backend::audio::eq::EqSettings> for EqSettingsDto {
    fn from(s: nexus_player_backend::audio::eq::EqSettings) -> Self {
        Self {
            enabled: s.enabled,
            gains: s.gains.to_vec(),
            preset_name: s.preset_name,
        }
    }
}

impl From<EqSettingsDto> for nexus_player_backend::audio::eq::EqSettings {
    fn from(dto: EqSettingsDto) -> Self {
        let mut gains = [0.0f32; 10];
        for (i, &g) in dto.gains.iter().take(10).enumerate() {
            gains[i] = g.clamp(-12.0, 12.0);
        }
        Self {
            enabled: dto.enabled,
            gains,
            preset_name: dto.preset_name,
        }
    }
}
