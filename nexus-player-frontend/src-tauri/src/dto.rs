use serde::{Deserialize, Serialize};

pub use nexus_player_backend::library::types::{ArtworkId, PlaylistId, TrackId};

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
pub struct QueueTrackDto {
    pub index: usize,
    pub track_id: usize,
    pub title: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioStateDto {
    pub state: String,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub volume: f32,
    pub current_track: Option<QueueTrackDto>,
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

        let current_track = state.current_track.map(|track| QueueTrackDto {
            index: 0,
            track_id: track.id,
            title: "Unknown".to_string(),
            source: track.source.as_str(),
        });

        Self {
            state: state_str.to_string(),
            position_secs: state.position.as_secs_f64(),
            duration_secs: state.duration.as_secs_f64(),
            volume: state.volume,
            current_track,
        }
    }
}
