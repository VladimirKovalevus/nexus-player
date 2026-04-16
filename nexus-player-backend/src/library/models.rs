use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::library::types::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayHistoryEntry {
    pub id: i64,
    pub title: String,
    pub artist: Option<String>,
    pub source: TrackSource,
    pub artwork_url: Option<String>,
    pub played_at: i64,
}

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
    pub year: Option<u32>,
    pub genre: Option<String>,
    pub track_number: Option<u32>,

    pub source: TrackSource,
    pub cache: CacheStatus,

    pub audio_info: Option<AudioInfo>,

    pub play_count: u64,
    pub last_played: Option<DateTime<Utc>>,

    pub added_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    pub liked: bool,
    pub liked_position: Option<i64>,
    pub unavailable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub id: PlaylistId,

    pub metadata: Metadata,
    pub creator: Option<String>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaylistItemKind {
    Track,
    Playlist,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistItem {
    pub id: i64,
    pub item_kind: PlaylistItemKind,
    pub item_id: i64,
    pub position: u32,
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistWithItems {
    pub playlist: Playlist,
    pub items: Vec<PlaylistItem>,
}
