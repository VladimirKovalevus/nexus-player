use serde::{Deserialize, Serialize};

use crate::audio::types::TrackSource;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncFile {
    pub version: u32,
    pub sync_type: SyncType,
    pub created_at: i64,
    pub device_name: Option<String>,
    pub tracks: Vec<SyncTrack>,
    pub playlists: Vec<SyncPlaylist>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncType {
    Full,
    Diff,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncTrack {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub year: Option<u32>,
    pub genre: Option<String>,
    pub track_number: Option<u32>,
    pub duration_secs: Option<f64>,
    pub source: SyncTrackSource,
    pub liked: bool,
    pub liked_position: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artwork_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub added_at: Option<i64>,
}

/// Subset of TrackSource that can be synced across devices.
/// Local and Cached sources are excluded; Cached is normalized to Http.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SyncTrackSource {
    Http { url: String },
    External { system: String, id: String },
}

impl SyncTrackSource {
    pub fn into_track_source(self) -> TrackSource {
        match self {
            SyncTrackSource::Http { url } => TrackSource::Http(url),
            SyncTrackSource::External { system, id } => TrackSource::External { system, id },
        }
    }

    pub fn from_track_source(source: &TrackSource) -> Option<Self> {
        match source {
            TrackSource::Local(_) => None,
            TrackSource::Http(url) => Some(SyncTrackSource::Http { url: url.clone() }),
            TrackSource::External { system, id } => Some(SyncTrackSource::External {
                system: system.clone(),
                id: id.clone(),
            }),
            TrackSource::Cached { original_url, .. } => Some(SyncTrackSource::Http {
                url: original_url.clone(),
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncPlaylist {
    pub sync_id: usize,
    pub title: String,
    pub description: Option<String>,
    pub creator: Option<String>,
    pub items: Vec<SyncPlaylistItem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artwork_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SyncPlaylistItem {
    Track { track_index: usize },
    Playlist { sync_id: usize },
}
