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

    /// Create playlist with tracks in a single transaction
    ///
    /// Takes a runtime playlist (from streaming/playback), saves all tracks to library,
    /// creates the playlist, and links tracks together atomically.
    pub async fn create_playlist_with_tracks(
        &self,
        runtime_playlist: crate::playlist::playlist::Playlist,
    ) -> Result<models::PlaylistWithTracks> {
        let mut tx = self.pool.begin().await?;

        let mut saved_track_ids = Vec::new();

        for audio_track in runtime_playlist.tracks() {
            let library_track: models::Track = audio_track.clone().into();

            let track_id = sqlx::query_scalar::<_, i64>(
                r#"
                INSERT INTO tracks (
                    title, description, artist, album, album_artist, year, genre, track_number,
                    source_type, source_path, cache_status, duration_secs, sample_rate,
                    channels, bitrate, format, play_count, added_at, updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
                "#,
            )
            .bind(&library_track.metadata.title)
            .bind(&library_track.metadata.description)
            .bind(&library_track.artist)
            .bind(&library_track.album)
            .bind(&library_track.album_artist)
            .bind(library_track.year.map(|y| y as i64))
            .bind(&library_track.genre)
            .bind(library_track.track_number.map(|t| t as i64))
            .bind("local")
            .bind("")
            .bind("none")
            .bind(library_track.audio_info.as_ref().and_then(|i| i.duration_secs))
            .bind(library_track.audio_info.as_ref().and_then(|i| i.sample_rate).map(|s| s as i64))
            .bind(library_track.audio_info.as_ref().and_then(|i| i.channels).map(|c| c as i64))
            .bind(library_track.audio_info.as_ref().and_then(|i| i.bitrate).map(|b| b as i64))
            .bind(library_track.audio_info.as_ref().map(|i| i.format.as_str()))
            .bind(library_track.play_count as i64)
            .bind(library_track.added_at.timestamp())
            .bind(library_track.updated_at.timestamp())
            .fetch_one(&mut *tx)
            .await
            .context("Failed to insert track")?;

            saved_track_ids.push(TrackId(track_id));
        }

        let playlist_library = models::Playlist {
            id: PlaylistId(0),
            metadata: models::Metadata {
                title: runtime_playlist.title.clone(),
                description: runtime_playlist.description.clone(),
                artwork: None,
            },
            creator: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let playlist_id = sqlx::query_scalar::<_, i64>(
            r#"
            INSERT INTO playlists (title, description, creator, artwork_id, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(&playlist_library.metadata.title)
        .bind(&playlist_library.metadata.description)
        .bind(&playlist_library.creator)
        .bind(None::<i64>)
        .bind(playlist_library.created_at.timestamp())
        .bind(playlist_library.updated_at.timestamp())
        .fetch_one(&mut *tx)
        .await
        .context("Failed to create playlist")?;

        let playlist_id = PlaylistId(playlist_id);

        let mut playlist_tracks = Vec::new();

        for (position, track_id) in saved_track_ids.iter().enumerate() {
            let now = chrono::Utc::now();

            sqlx::query(
                r#"
                INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at)
                VALUES (?1, ?2, ?3, ?4)
                "#,
            )
            .bind(playlist_id.0)
            .bind(track_id.0)
            .bind(position as i64)
            .bind(now.timestamp())
            .execute(&mut *tx)
            .await
            .context("Failed to add track to playlist")?;

            playlist_tracks.push(models::PlaylistTrack {
                track_id: *track_id,
                position: position as u32,
                added_at: now,
            });
        }

        tx.commit().await.context("Failed to commit transaction")?;

        let mut playlist = playlist_library;
        playlist.id = playlist_id;

        Ok(models::PlaylistWithTracks {
            playlist,
            tracks: playlist_tracks,
        })
    }

    // ========================================================================
    // Utility
    // ========================================================================

    /// Get direct access to database pool (for advanced use)
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}
