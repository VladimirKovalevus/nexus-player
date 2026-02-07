use nexus_player_backend::audio::types::TrackState;
use serde::{Deserialize, Serialize};

pub use nexus_player_backend::library::types::{ArtworkId, PlaylistId, TrackId};
use nexus_player_backend::streaming::types as streaming;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackDto {
    pub id: i64,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_secs: Option<f64>,
    pub artwork_url: Option<String>,
    pub play_count: u64,
    pub state: TrackStateDto,
    pub source: Option<TrackSourceDto>,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistWithTracksDto {
    pub playlist: PlaylistDto,
    pub tracks: Vec<TrackDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueStateDto {
    pub tracks: Vec<TrackDto>,
    pub current_index: Option<usize>,
    pub repeat_mode: String,
    pub shuffle_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioStateDto {
    pub state: String,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub volume: f32,
    pub current_track: Option<TrackDto>,
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
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePlaylistWithTracksRequest {
    pub title: String,
    pub description: Option<String>,
    pub tracks: Vec<TrackDto>,
}

impl TrackDto {
    pub fn to_audio_track(&self) -> nexus_player_backend::audio::types::Track {
        use nexus_player_backend::audio::types::{Track, TrackSource, TrackState};

        let source = match &self.source {
            Some(src) => TrackSource::External {
                id: src.url.clone(),
                system: src.system.clone(),
            },
            None => TrackSource::External {
                id: String::new(),
                system: "unknown".to_string(),
            },
        };

        let state = match &self.state {
            TrackStateDto::Saved => TrackState::Saved,
            TrackStateDto::Streaming => TrackState::Streaming,
            TrackStateDto::Cached => TrackState::Cached,
        };

        Track {
            id: self.id as usize,
            title: self.title.clone(),
            artist: self.artist.clone(),
            album: self.album.clone(),
            duration: self.duration_secs.map(std::time::Duration::from_secs_f64),
            artwork_url: self.artwork_url.clone(),
            format: None,
            source,
            state,
        }
    }
}

use nexus_player_backend::audio::types::{self as audio};
use nexus_player_backend::library::models;

impl From<models::Track> for TrackDto {
    fn from(track: models::Track) -> Self {
        let artwork_url = track.metadata.artwork.as_ref().and_then(|art| match &art.source {
            nexus_player_backend::library::types::ArtworkSource::Url { url } => Some(url.clone()),
            _ => None,
        });

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
            state: TrackStateDto::Saved,
            source: Some(track.source.into()),
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
            state: track.state.into(),
            source: Some(track.source.into()),
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
            state: track.state.clone().into(),
            source: Some(track.source.clone().into()),
        }
    }
}

impl From<models::Playlist> for PlaylistDto {
    fn from(playlist: models::Playlist) -> Self {
        let thumbnail = playlist.metadata.artwork.as_ref().and_then(|art| match &art.source {
            nexus_player_backend::library::types::ArtworkSource::Url { url } => Some(url.clone()),
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
        }
    }
}

impl From<models::PlaylistWithTracks> for PlaylistWithTracksDto {
    fn from(pwt: models::PlaylistWithTracks) -> Self {
        let mut playlist_dto: PlaylistDto = pwt.playlist.into();
        playlist_dto.track_count = pwt.tracks.len();

        Self {
            playlist: playlist_dto,
            tracks: vec![],
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
        };

        Self {
            state: state_str.to_string(),
            position_secs: state.position.as_secs_f64(),
            duration_secs: state.duration.as_secs_f64(),
            volume: state.volume,
            current_track: None,
            // current_track: state.current_track.map(|t| t.into()),
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
    pub title: String,
    pub artist: String,
    pub duration_secs: Option<f64>,
    pub thumbnail: Option<String>,
    pub source: TrackSourceDto,
    pub state: TrackStateDto,
}

impl From<nexus_player_backend::audio::types::TrackSource> for TrackSourceDto {
    fn from(track_source: nexus_player_backend::audio::types::TrackSource) -> Self {
        match track_source {
            nexus_player_backend::audio::types::TrackSource::External { id, system } => {
                Self { url: id, system }
            }
            _ => Self {
                url: track_source.as_str(),
                system: "unknown".to_string(),
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
                    title: track.title,
                    artist: track.artist,
                    duration_secs: track.duration.map(|d| d.as_secs_f64()),
                    thumbnail: track.thumbnail,
                    source: TrackSourceDto { url, system },
                    state: TrackStateDto::Streaming,
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
            title: track.title,
            artist: track.artist,
            duration_secs: track.duration.map(|d| d.as_secs_f64()),
            thumbnail: track.thumbnail,
            source: TrackSourceDto { url, system },
            state: TrackStateDto::Streaming,
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
                let service = std::str::FromStr::from_str(&playlist.system).unwrap_or(streaming::StreamingService::YouTube);
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
                let service = std::str::FromStr::from_str(&album.system).unwrap_or(streaming::StreamingService::YouTube);
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
                let service = std::str::FromStr::from_str(&channel.system).unwrap_or(streaming::StreamingService::YouTube);
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
