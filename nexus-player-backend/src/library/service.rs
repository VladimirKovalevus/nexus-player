//! LibraryService - facade for all library operations
//!
//! This service encapsulates database connection and repositories,
//! providing a clean API for track and playlist management.

use std::path::Path;
use std::sync::Arc;
use anyhow::{Result, Context};
use sqlx::SqlitePool;

use crate::db;
use super::{
    TrackRepository, PlaylistRepository,
    SqliteTrackRepository, SqlitePlaylistRepository,
    models, TrackId, PlaylistId,
};

/// Service for managing music library (tracks and playlists)
pub struct LibraryService {
    pool: SqlitePool,
    track_repo: Arc<dyn TrackRepository>,
    playlist_repo: Arc<dyn PlaylistRepository>,
}

impl LibraryService {
    /// Create new LibraryService with initialized database
    pub async fn new(db_path: &Path) -> Result<Self> {
        // Initialize database and run migrations
        let pool = db::create_pool(db_path).await?;

        // Create repositories
        let track_repo: Arc<dyn TrackRepository> = Arc::new(
            SqliteTrackRepository::new(pool.clone())
        );
        let playlist_repo: Arc<dyn PlaylistRepository> = Arc::new(
            SqlitePlaylistRepository::new(pool.clone())
        );

        Ok(Self {
            pool,
            track_repo,
            playlist_repo,
        })
    }

    // ========================================================================
    // Track Operations
    // ========================================================================

    /// Add a new track to the library
    pub async fn add_track(&self, track: models::Track) -> Result<TrackId> {
        self.track_repo.insert(track).await
    }

    /// Get track by ID
    pub async fn get_track(&self, id: TrackId) -> Result<Option<models::Track>> {
        self.track_repo.get(id).await
    }

    /// Get all tracks
    pub async fn get_all_tracks(&self) -> Result<Vec<models::Track>> {
        self.track_repo.list_all().await
    }

    /// Search tracks by query
    pub async fn search_tracks(&self, query: &str) -> Result<Vec<models::Track>> {
        self.track_repo.search(query).await
    }

    /// Find tracks by artist
    pub async fn find_by_artist(&self, artist: &str) -> Result<Vec<models::Track>> {
        self.track_repo.find_by_artist(artist).await
    }

    /// Find tracks by album
    pub async fn find_by_album(&self, album: &str) -> Result<Vec<models::Track>> {
        self.track_repo.find_by_album(album).await
    }

    /// Update track
    pub async fn update_track(&self, track: models::Track) -> Result<()> {
        self.track_repo.update(track).await
    }

    /// Delete track
    pub async fn delete_track(&self, id: TrackId) -> Result<()> {
        self.track_repo.delete(id).await
    }

    /// Increment play count
    pub async fn increment_play_count(&self, id: TrackId) -> Result<()> {
        self.track_repo.increment_play_count(id).await
    }

    // ========================================================================
    // Playlist Operations
    // ========================================================================

    /// Create a new playlist
    pub async fn create_playlist(&self, playlist: models::Playlist) -> Result<PlaylistId> {
        self.playlist_repo.create(playlist).await
    }

    /// Get playlist by ID
    pub async fn get_playlist(&self, id: PlaylistId) -> Result<Option<models::Playlist>> {
        self.playlist_repo.get(id).await
    }

    /// Get playlist with tracks
    pub async fn get_playlist_with_tracks(&self, id: PlaylistId) -> Result<Option<models::PlaylistWithTracks>> {
        self.playlist_repo.get_with_tracks(id).await
    }

    /// Get all playlists
    pub async fn get_all_playlists(&self) -> Result<Vec<models::Playlist>> {
        self.playlist_repo.list_all().await
    }

    /// Update playlist
    pub async fn update_playlist(&self, playlist: models::Playlist) -> Result<()> {
        self.playlist_repo.update(playlist).await
    }

    /// Delete playlist
    pub async fn delete_playlist(&self, id: PlaylistId) -> Result<()> {
        self.playlist_repo.delete(id).await
    }

    /// Add track to playlist
    pub async fn add_track_to_playlist(&self, playlist_id: PlaylistId, track_id: TrackId) -> Result<()> {
        // Get current playlist to determine position
        let playlist = self.playlist_repo
            .get_with_tracks(playlist_id)
            .await?
            .context("Playlist not found")?;

        let position = playlist.tracks.len() as u32;

        self.playlist_repo
            .add_track(playlist_id, track_id, position)
            .await
    }

    /// Remove track from playlist
    pub async fn remove_track_from_playlist(&self, playlist_id: PlaylistId, track_id: TrackId) -> Result<()> {
        self.playlist_repo
            .remove_track(playlist_id, track_id)
            .await
    }

    /// Reorder tracks in playlist
    pub async fn reorder_playlist_tracks(&self, playlist_id: PlaylistId, track_ids: Vec<TrackId>) -> Result<()> {
        self.playlist_repo
            .reorder_tracks(playlist_id, track_ids)
            .await
    }

    // ========================================================================
    // Utility
    // ========================================================================

    /// Get direct access to database pool (for advanced use)
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}
