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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistDto {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub creator: Option<String>,
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

use nexus_player_backend::audio::types as audio;
use nexus_player_backend::library::models;

impl From<models::Track> for TrackDto {
    fn from(track: models::Track) -> Self {
        Self {
            id: track.id.0,
            title: track.metadata.title,
            artist: track.artist,
            album: track.album,
            duration_secs: track
                .audio_info
                .as_ref()
                .and_then(|info| info.duration_secs),
            artwork_url: None,
            play_count: track.play_count,
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
        }
    }
}

impl From<models::Playlist> for PlaylistDto {
    fn from(playlist: models::Playlist) -> Self {
        Self {
            id: playlist.id.0,
            title: playlist.metadata.title,
            description: playlist.metadata.description,
            creator: playlist.creator,
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
            current_track: state.current_track.map(|t| t.into()),
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
pub struct StreamingTrackDto {
    pub title: String,
    pub artist: String,
    pub duration_secs: Option<f64>,
    pub thumbnail: Option<String>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamingPlaylistDto {
    pub id: String,
    pub title: String,
    pub creator: String,
    pub track_count: usize,
    pub thumbnail: Option<String>,
    pub url: String,
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
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamingChannelDto {
    pub id: String,
    pub name: String,
    pub subscriber_count: Option<u64>,
    pub thumbnail: Option<String>,
    pub url: String,
}

impl From<streaming::SearchResult> for SearchResultDto {
    fn from(result: streaming::SearchResult) -> Self {
        match result {
            streaming::SearchResult::Track(track) => {
                SearchResultDto::Track(StreamingTrackDto {
                    title: track.title,
                    artist: track.artist,
                    duration_secs: track.duration.map(|d| d.as_secs_f64()),
                    thumbnail: track.thumbnail,
                    url: track.source.as_str(),
                })
            }
            streaming::SearchResult::Playlist(playlist) => {
                SearchResultDto::Playlist(StreamingPlaylistDto {
                    id: playlist.id,
                    title: playlist.title,
                    creator: playlist.creator,
                    track_count: playlist.track_count,
                    thumbnail: playlist.thumbnail,
                    url: playlist.url,
                })
            }
            streaming::SearchResult::Album(album) => {
                SearchResultDto::Album(StreamingAlbumDto {
                    id: album.id,
                    title: album.title,
                    artist: album.artist,
                    track_count: album.track_count,
                    year: album.year,
                    thumbnail: album.thumbnail,
                    url: album.url,
                })
            }
            streaming::SearchResult::Channel(channel) => {
                SearchResultDto::Channel(StreamingChannelDto {
                    id: channel.id,
                    name: channel.name,
                    subscriber_count: channel.subscriber_count,
                    thumbnail: channel.thumbnail,
                    url: channel.url,
                })
            }
        }
    }
}

