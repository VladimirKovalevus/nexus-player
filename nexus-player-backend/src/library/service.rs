//! LibraryService - facade for all library operations
//!
//! This service encapsulates database connection and repositories,
//! providing a clean API for track and playlist management.

use anyhow::{Context, Result};
use sea_orm::DatabaseConnection;
use sqlx::SqlitePool;
use std::path::Path;
use std::sync::Arc;

use super::{
    db_v2, models, PlaylistId, PlaylistRepositoryV2, SeaOrmPlaylistRepository,
    SeaOrmTrackRepository, TrackId, TrackRepositoryV2,
};
use crate::db;

/// Service for managing music library (tracks and playlists)
pub struct LibraryService {
    pool: SqlitePool,
    db: DatabaseConnection,
    track_repo: Arc<dyn TrackRepositoryV2>,
    playlist_repo: Arc<dyn PlaylistRepositoryV2>,
}

impl LibraryService {
    /// Create new LibraryService with initialized database
    pub async fn new(db_path: &Path) -> Result<Self> {
        let pool = db::create_pool(db_path).await?;

        let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
        let db = db_v2::establish_connection(&db_url).await?;

        let track_repo: Arc<dyn TrackRepositoryV2> =
            Arc::new(SeaOrmTrackRepository::new(db.clone()));
        let playlist_repo: Arc<dyn PlaylistRepositoryV2> =
            Arc::new(SeaOrmPlaylistRepository::new(db.clone()));

        Ok(Self {
            pool,
            db,
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
    pub async fn get_playlist_with_tracks(
        &self,
        id: PlaylistId,
    ) -> Result<Option<models::PlaylistWithTracks>> {
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
    pub async fn add_track_to_playlist(
        &self,
        playlist_id: PlaylistId,
        track_id: TrackId,
    ) -> Result<()> {
        // Get current playlist to determine position
        let playlist = self
            .playlist_repo
            .get_with_tracks(playlist_id)
            .await?
            .context("Playlist not found")?;

        let position = playlist.tracks.len() as u32;

        self.playlist_repo
            .add_track(playlist_id, track_id, position)
            .await
    }

    /// Remove track from playlist
    pub async fn remove_track_from_playlist(
        &self,
        playlist_id: PlaylistId,
        track_id: TrackId,
    ) -> Result<()> {
        self.playlist_repo.remove_track(playlist_id, track_id).await
    }

    /// Reorder tracks in playlist
    pub async fn reorder_playlist_tracks(
        &self,
        playlist_id: PlaylistId,
        track_ids: Vec<TrackId>,
    ) -> Result<()> {
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
        use chrono::Utc;
        use sea_orm::{ActiveModelTrait, ActiveValue, TransactionTrait};

        use super::entities_v2::{playlist, playlist_track, track};

        let txn = self.db.begin().await?;

        let mut saved_track_ids = Vec::new();

        for audio_track in runtime_playlist.tracks() {
            let library_track: models::Track = audio_track.clone().into();
            let entity_track: track::Model = library_track.into();

            let active_track = track::ActiveModel {
                id: ActiveValue::NotSet,
                title: ActiveValue::Set(entity_track.title),
                artist: ActiveValue::Set(entity_track.artist),
                description: ActiveValue::Set(entity_track.description),
                artwork_id: ActiveValue::Set(entity_track.artwork_id),
                album: ActiveValue::Set(entity_track.album),
                album_artist: ActiveValue::Set(entity_track.album_artist),
                year: ActiveValue::Set(entity_track.year),
                genre: ActiveValue::Set(entity_track.genre),
                track_number: ActiveValue::Set(entity_track.track_number),
                source_type: ActiveValue::Set(entity_track.source_type),
                source_data: ActiveValue::Set(entity_track.source_data),
                cache_type: ActiveValue::Set(entity_track.cache_type),
                cache_data: ActiveValue::Set(entity_track.cache_data),
                file_format: ActiveValue::Set(entity_track.file_format),
                duration_secs: ActiveValue::Set(entity_track.duration_secs),
                sample_rate: ActiveValue::Set(entity_track.sample_rate),
                channels: ActiveValue::Set(entity_track.channels),
                bitrate: ActiveValue::Set(entity_track.bitrate),
                play_count: ActiveValue::Set(entity_track.play_count),
                last_played: ActiveValue::Set(entity_track.last_played),
                added_at: ActiveValue::Set(entity_track.added_at),
                updated_at: ActiveValue::Set(entity_track.updated_at),
            };

            let result = active_track
                .insert(&txn)
                .await
                .context("Failed to insert track")?;

            saved_track_ids.push(TrackId(result.id));
        }

        let now = Utc::now();
        let playlist_library = models::Playlist {
            id: PlaylistId(0),
            metadata: models::Metadata {
                title: runtime_playlist.title.clone(),
                description: runtime_playlist.description.clone(),
                artwork: None,
            },
            creator: None,
            created_at: now,
            updated_at: now,
        };

        let entity_playlist: playlist::Model = playlist_library.into();
        let active_playlist = playlist::ActiveModel {
            id: ActiveValue::NotSet,
            title: ActiveValue::Set(entity_playlist.title),
            description: ActiveValue::Set(entity_playlist.description),
            creator: ActiveValue::Set(entity_playlist.creator),
            artwork_id: ActiveValue::Set(entity_playlist.artwork_id),
            created_at: ActiveValue::Set(entity_playlist.created_at),
            updated_at: ActiveValue::Set(entity_playlist.updated_at),
        };

        let playlist_result = active_playlist
            .insert(&txn)
            .await
            .context("Failed to create playlist")?;

        let playlist_id = PlaylistId(playlist_result.id);

        let mut playlist_tracks = Vec::new();

        for (position, track_id) in saved_track_ids.iter().enumerate() {
            let now_ts = Utc::now().timestamp();

            let active_playlist_track = playlist_track::ActiveModel {
                playlist_id: ActiveValue::Set(playlist_id.0),
                track_id: ActiveValue::Set(track_id.0),
                position: ActiveValue::Set(position as i64),
                added_at: ActiveValue::Set(now_ts),
            };

            active_playlist_track
                .insert(&txn)
                .await
                .context("Failed to add track to playlist")?;

            playlist_tracks.push(models::PlaylistTrack {
                track_id: *track_id,
                position: position as u32,
                added_at: Utc::now(),
            });
        }

        txn.commit().await.context("Failed to commit transaction")?;

        let playlist = models::Playlist {
            id: playlist_id,
            metadata: models::Metadata {
                title: runtime_playlist.title,
                description: runtime_playlist.description,
                artwork: None,
            },
            creator: None,
            created_at: now,
            updated_at: now,
        };

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

    /// Get direct access to SeaORM database connection
    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }
}
