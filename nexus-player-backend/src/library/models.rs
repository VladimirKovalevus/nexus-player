use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::library::types::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub title: String,
    pub description: Option<String>,
    pub artwork: Option<Artwork>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub id: TrackId,

    pub metadata: Metadata,

    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub year: Option<u16>,
    pub genre: Option<String>,
    pub track_number: Option<u16>,

    pub source: TrackSource,
    pub cache: CacheStatus,

    pub audio_info: Option<AudioInfo>,

    pub play_count: u64,
    pub last_played: Option<DateTime<Utc>>,

    pub added_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub id: PlaylistId,

    pub metadata: Metadata,
    pub creator: Option<String>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistTrack {
    pub track_id: TrackId,
    pub position: u32,
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistWithTracks {
    pub playlist: Playlist,
    pub tracks: Vec<PlaylistTrack>,
}
