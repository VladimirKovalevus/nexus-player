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
use chrono::Utc;

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

        if playlist_repo.find_by_title("Liked Songs").await?.is_none() {
            let now = Utc::now();
            let liked_songs_playlist = models::Playlist {
                id: PlaylistId(0),
                metadata: models::Metadata {
                    title: "Liked Songs".to_string(),
                    description: Some("Your favorite tracks".to_string()),
                    artwork: None,
                },
                creator: None,
                created_at: now,
                updated_at: now,
            };
            playlist_repo.create(liked_songs_playlist).await?;
        }

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
    pub async fn add_track(&self, mut track: models::Track) -> Result<TrackId> {
        // Resolve artwork if present and ID is 0 (dummy from conversion)
        if let Some(art) = &track.metadata.artwork {
            if art.id.0 == 0 {
                if let crate::library::types::ArtworkSource::Url { url } = &art.source {
                    let real_id = self.ensure_artwork(url, &self.db).await?;
                    if let Some(mut a) = track.metadata.artwork.take() {
                        a.id = crate::library::types::ArtworkId(real_id);
                        track.metadata.artwork = Some(a);
                    }
                }
            }
        }

        let track_id = self.track_repo.insert(track).await?;
        if let Some(master_playlist) = self
            .playlist_repo
            .find_by_title("Liked Songs")
            .await
            .context("Failed to find master playlist")?
        {
            self.add_track_to_playlist(master_playlist.id, track_id)
                .await?;
        }
        Ok(track_id)
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

    /// Find existing track by source or insert a new one
    pub async fn ensure_track(&self, track: models::Track) -> Result<TrackId> {
        if let Some(existing) = self.track_repo.find_by_source(&track.source).await? {
            return Ok(existing.id);
        }
        self.add_track(track).await
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

    /// Get playlist with items (tracks and sub-playlists)
    pub async fn get_playlist_with_items(
        &self,
        id: PlaylistId,
    ) -> Result<Option<models::PlaylistWithItems>> {
        self.playlist_repo.get_with_items(id).await
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
        let playlist = self
            .playlist_repo
            .get_with_items(playlist_id)
            .await?
            .context("Playlist not found")?;

        let position = playlist.items.len() as u32;

        self.playlist_repo
            .add_item(playlist_id, models::PlaylistItemKind::Track, track_id.0, position)
            .await
    }

    /// Remove track from playlist
    pub async fn remove_track_from_playlist(
        &self,
        playlist_id: PlaylistId,
        track_id: TrackId,
    ) -> Result<()> {
        self.playlist_repo
            .remove_item(playlist_id, models::PlaylistItemKind::Track, track_id.0)
            .await
    }

    /// Add sub-playlist to playlist
    pub async fn add_playlist_to_playlist(
        &self,
        parent_id: PlaylistId,
        child_id: PlaylistId,
    ) -> Result<()> {
        let playlist = self
            .playlist_repo
            .get_with_items(parent_id)
            .await?
            .context("Playlist not found")?;

        let position = playlist.items.len() as u32;

        self.playlist_repo
            .add_item(parent_id, models::PlaylistItemKind::Playlist, child_id.0, position)
            .await
    }

    /// Remove sub-playlist from playlist
    pub async fn remove_playlist_from_playlist(
        &self,
        parent_id: PlaylistId,
        child_id: PlaylistId,
    ) -> Result<()> {
        self.playlist_repo
            .remove_item(parent_id, models::PlaylistItemKind::Playlist, child_id.0)
            .await
    }

    /// Reorder items in playlist
    pub async fn reorder_playlist_items(
        &self,
        playlist_id: PlaylistId,
        item_ids: Vec<i64>,
    ) -> Result<()> {
        self.playlist_repo
            .reorder_items(playlist_id, item_ids)
            .await
    }

    /// Create playlist with tracks in a single transaction
    ///
    /// Takes a runtime playlist (from streaming/playback), saves all tracks to library,
    /// creates the playlist, and links tracks together atomically.
    /// Create playlist with tracks in a single transaction
    pub async fn create_playlist_with_tracks(
        &self,
        runtime_playlist: crate::playlist::playlist::Playlist,
    ) -> Result<models::PlaylistWithItems> {
        use chrono::Utc;
        use sea_orm::{ActiveModelTrait, ActiveValue, TransactionTrait};

        use super::entities_v2::{playlist, playlist_item, track};

        let txn = self.db.begin().await?;

        let mut saved_track_ids = Vec::new();
        let mut first_artwork = None;
        let mut fallback_url = None;

        for audio_track in runtime_playlist.tracks() {
            let library_track: models::Track = audio_track.clone().into();
            let entity_track: track::Model = library_track.into();

            let artwork_id = if let Some(url) = &audio_track.artwork_url {
                let id = self.ensure_artwork(url, &txn).await?;
                if first_artwork.is_none() {
                    first_artwork = Some(id);
                    fallback_url = Some(url.clone());
                }
                Some(id)
            } else {
                if first_artwork.is_none() {
                    first_artwork = entity_track.artwork_id;
                }
                entity_track.artwork_id
            };

            let active_track = track::ActiveModel {
                id: ActiveValue::NotSet,
                title: ActiveValue::Set(entity_track.title),
                artist: ActiveValue::Set(entity_track.artist),
                description: ActiveValue::Set(entity_track.description),
                artwork_id: ActiveValue::Set(artwork_id),
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

        let playlist_artwork_id = if let Some(url) = &runtime_playlist.artwork {
            Some(self.ensure_artwork(url, &txn).await?)
        } else {
            first_artwork
        };

        let entity_playlist: playlist::Model = playlist_library.into();
        let active_playlist = playlist::ActiveModel {
            id: ActiveValue::NotSet,
            title: ActiveValue::Set(entity_playlist.title),
            description: ActiveValue::Set(entity_playlist.description),
            creator: ActiveValue::Set(entity_playlist.creator),
            artwork_id: ActiveValue::Set(playlist_artwork_id),
            created_at: ActiveValue::Set(entity_playlist.created_at),
            updated_at: ActiveValue::Set(entity_playlist.updated_at),
        };

        let playlist_result = active_playlist
            .insert(&txn)
            .await
            .context("Failed to create playlist")?;

        let playlist_id = PlaylistId(playlist_result.id);

        let mut playlist_items = Vec::new();

        for (position, track_id) in saved_track_ids.iter().enumerate() {
            let now_ts = Utc::now().timestamp();

            let active_item = playlist_item::ActiveModel {
                id: ActiveValue::NotSet,
                playlist_id: ActiveValue::Set(playlist_id.0),
                item_type: ActiveValue::Set("track".to_string()),
                item_id: ActiveValue::Set(track_id.0),
                position: ActiveValue::Set(position as i64),
                added_at: ActiveValue::Set(now_ts),
            };

            let inserted = active_item
                .insert(&txn)
                .await
                .context("Failed to add item to playlist")?;

            playlist_items.push(models::PlaylistItem {
                id: inserted.id,
                item_kind: models::PlaylistItemKind::Track,
                item_id: track_id.0,
                position: position as u32,
                added_at: Utc::now(),
            });
        }

        txn.commit().await.context("Failed to commit transaction")?;

        let artwork = if let Some(id) = playlist_artwork_id {
            let url = if let Some(url) = &runtime_playlist.artwork {
                Some(url.clone())
            } else {
                fallback_url
            };

            if let Some(url) = url {
                Some(super::types::Artwork {
                    id: super::types::ArtworkId(id),
                    source: super::types::ArtworkSource::Url { url },
                    width: None,
                    height: None,
                    created_at: now,
                })
            } else {
                None
            }
        } else {
            None
        };

        let playlist = models::Playlist {
            id: playlist_id,
            metadata: models::Metadata {
                title: runtime_playlist.title,
                description: runtime_playlist.description,
                artwork,
            },
            creator: None,
            created_at: now,
            updated_at: now,
        };

        Ok(models::PlaylistWithItems {
            playlist,
            items: playlist_items,
        })
    }

    async fn ensure_artwork<C>(&self, url: &str, db: &C) -> Result<i64>
    where
        C: sea_orm::ConnectionTrait,
    {
        use super::entities_v2::artwork;
        use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter};

        // Construct source_data JSON
        let source_data = format!(r#"{{"type":"url","url":"{}"}}"#, url);

        // Check if exists (simple check by source_data string match)
        let existing = artwork::Entity::find()
            .filter(artwork::Column::SourceType.eq("url"))
            .filter(artwork::Column::SourceData.eq(&source_data))
            .one(db)
            .await?;

        if let Some(art) = existing {
            return Ok(art.id);
        }

        // Create new
        let now = Utc::now().timestamp();
        let active_art = artwork::ActiveModel {
            id: ActiveValue::NotSet,
            source_type: ActiveValue::Set("url".to_string()),
            source_data: ActiveValue::Set(Some(source_data)),
            width: ActiveValue::Set(None),
            height: ActiveValue::Set(None),
            created_at: ActiveValue::Set(now),
        };

        let res = active_art.insert(db).await?;
        Ok(res.id)
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
