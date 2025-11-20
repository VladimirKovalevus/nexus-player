//! Database row types
//!
//! These types directly map to database tables and are used only for SQLx queries.
//! They use primitive types (i64, String, etc.) that match SQL columns.
//! Conversions to/from domain types are in the `convert` module.

use sqlx::FromRow;

// ============================================================================
// Artwork Row
// ============================================================================

/// Database row for the `artworks` table
#[derive(Debug, Clone, FromRow)]
pub(super) struct ArtworkRow {
    pub id: i64,

    // Image source (one of these must be present)
    pub data: Option<Vec<u8>>,
    pub url: Option<String>,

    // Metadata
    pub mime_type: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,

    pub created_at: i64,
}

// ============================================================================
// Track Row
// ============================================================================

/// Database row for the `tracks` table
#[derive(Debug, Clone, FromRow)]
pub(super) struct TrackRow {
    pub id: i64,

    // === Metadata ===
    pub title: String,
    pub artist: Option<String>,
    pub description: Option<String>,
    pub artwork_id: Option<i64>,

    // === Additional metadata ===
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub track_number: Option<i32>,

    // === Source (flat representation) ===
    pub source_type: String,  // 'local' | 'http' | 'external'
    pub source_local_path: Option<String>,
    pub source_http_url: Option<String>,
    pub source_external_system: Option<String>,
    pub source_external_id: Option<String>,

    // === Cache ===
    pub cached_local_path: Option<String>,
    pub cache_status: String,  // 'none' | 'downloading' | 'cached' | 'failed'
    pub cache_downloaded_at: Option<i64>,

    // === Audio info ===
    pub file_format: Option<String>,
    pub duration_secs: Option<f64>,
    pub sample_rate: Option<i32>,
    pub channels: Option<i32>,
    pub bitrate: Option<i32>,

    // === Statistics ===
    pub play_count: i64,
    pub last_played: Option<i64>,

    // === Timestamps ===
    pub added_at: i64,
    pub updated_at: i64,
}

// ============================================================================
// Playlist Rows
// ============================================================================

/// Database row for the `playlists` table
#[derive(Debug, Clone, FromRow)]
pub(super) struct PlaylistRow {
    pub id: i64,

    // === Metadata ===
    pub title: String,
    pub description: Option<String>,
    pub creator: Option<String>,
    pub artwork_id: Option<i64>,

    // === Timestamps ===
    pub created_at: i64,
    pub updated_at: i64,
}

/// Database row for the `playlist_tracks` junction table
#[derive(Debug, Clone, FromRow)]
pub(super) struct PlaylistTrackRow {
    pub playlist_id: i64,
    pub track_id: i64,
    pub position: i64,
    pub added_at: i64,
}
