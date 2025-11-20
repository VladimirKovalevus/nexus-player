//! Repository traits for data access
//!
//! This module defines trait-based interfaces for working with tracks and playlists.
//! Traits allow for flexible implementations (SQLite, PostgreSQL, in-memory for tests, etc.)

use async_trait::async_trait;
use anyhow::Result;

use crate::library::models::{Track, Playlist, PlaylistWithTracks};
use crate::library::types::{TrackId, PlaylistId};

// ============================================================================
// Track Repository
// ============================================================================

/// Repository for managing tracks in the library
#[async_trait]
pub trait TrackRepository: Send + Sync {
    /// Insert a new track into the library
    /// Returns the ID of the newly created track
    async fn insert(&self, track: Track) -> Result<TrackId>;

    /// Get a track by its ID
    async fn get(&self, id: TrackId) -> Result<Option<Track>>;

    /// Update an existing track
    async fn update(&self, track: Track) -> Result<()>;

    /// Delete a track by ID
    async fn delete(&self, id: TrackId) -> Result<()>;

    /// Get all tracks in the library
    async fn list_all(&self) -> Result<Vec<Track>>;

    /// Search tracks by title, artist, or album
    async fn search(&self, query: &str) -> Result<Vec<Track>>;

    /// Get tracks by artist
    async fn find_by_artist(&self, artist: &str) -> Result<Vec<Track>>;

    /// Get tracks by album
    async fn find_by_album(&self, album: &str) -> Result<Vec<Track>>;

    /// Increment play count for a track
    async fn increment_play_count(&self, id: TrackId) -> Result<()>;
}

// ============================================================================
// Playlist Repository
// ============================================================================

/// Repository for managing playlists
#[async_trait]
pub trait PlaylistRepository: Send + Sync {
    /// Create a new playlist
    async fn create(&self, playlist: Playlist) -> Result<PlaylistId>;

    /// Get a playlist by ID (without tracks)
    async fn get(&self, id: PlaylistId) -> Result<Option<Playlist>>;

    /// Get a playlist with all its tracks
    async fn get_with_tracks(&self, id: PlaylistId) -> Result<Option<PlaylistWithTracks>>;

    /// Update playlist metadata
    async fn update(&self, playlist: Playlist) -> Result<()>;

    /// Delete a playlist
    async fn delete(&self, id: PlaylistId) -> Result<()>;

    /// List all playlists
    async fn list_all(&self) -> Result<Vec<Playlist>>;

    /// Add a track to a playlist at a specific position
    async fn add_track(&self, playlist_id: PlaylistId, track_id: TrackId, position: u32) -> Result<()>;

    /// Remove a track from a playlist
    async fn remove_track(&self, playlist_id: PlaylistId, track_id: TrackId) -> Result<()>;

    /// Reorder tracks in a playlist
    async fn reorder_tracks(&self, playlist_id: PlaylistId, track_ids: Vec<TrackId>) -> Result<()>;
}
