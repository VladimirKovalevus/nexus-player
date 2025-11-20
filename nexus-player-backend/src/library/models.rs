//! Domain models for tracks, playlists, and related entities
//!
//! These types represent the public API of the library module.
//! They use type-safe wrappers and enums from the `types` module.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::library::types::*;

// ============================================================================
// Metadata - shared metadata fields
// ============================================================================

/// Common metadata shared by tracks and playlists
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub title: String,
    pub description: Option<String>,
    pub artwork: Option<Artwork>,
}

// ============================================================================
// Track
// ============================================================================

/// A music track in the library
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub id: TrackId,

    // === Core metadata ===
    pub metadata: Metadata,

    // === Music-specific metadata ===
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub year: Option<u16>,
    pub genre: Option<String>,
    pub track_number: Option<u16>,

    // === Source and caching ===
    pub source: TrackSource,
    pub cache: CacheStatus,

    // === Audio technical info ===
    pub audio_info: Option<AudioInfo>,

    // === Statistics ===
    pub play_count: u64,
    pub last_played: Option<DateTime<Utc>>,

    // === Timestamps ===
    pub added_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ============================================================================
// Playlist
// ============================================================================

/// A playlist containing multiple tracks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub id: PlaylistId,

    // === Metadata ===
    pub metadata: Metadata,
    pub creator: Option<String>,

    // === Timestamps ===
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A track within a playlist (with position and metadata)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistTrack {
    pub track_id: TrackId,
    pub position: u32,
    pub added_at: DateTime<Utc>,
}

/// A complete playlist with its tracks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistWithTracks {
    pub playlist: Playlist,
    pub tracks: Vec<PlaylistTrack>,
}
