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
    db_v2, models, types::TrackOrder, PlaylistId, PlaylistRepositoryV2, SeaOrmPlaylistRepository,
    SeaOrmTrackRepository, TrackId, TrackRepositoryV2,
};
use crate::db;
use crate::library::models::Track;
use crate::library::repository_v2::PlayHistoryRepository;
use crate::library::TrackSource;
use chrono::Utc;

/// The `artworks_v2.source_data` encoding for a URL artwork.
///
/// Shared by the writer and every lookup, so comparisons stay exact.
fn artwork_source_data(url: &str) -> String {
    format!(r#"{{"type":"url","url":"{}"}}"#, url)
}

/// Service for managing music library (tracks and playlists)
pub struct LibraryService {
    pub(crate) pool: SqlitePool,
    db: DatabaseConnection,
    track_repo: Arc<dyn TrackRepositoryV2>,
    playlist_repo: Arc<dyn PlaylistRepositoryV2>,
    history_repo: Arc<dyn PlayHistoryRepository>,
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
        let history_repo: Arc<dyn PlayHistoryRepository> =
            Arc::new(db_v2::SeaOrmHistoryRepository::new(db.clone()));

        let service = Self {
            pool,
            db,
            track_repo,
            playlist_repo,
            history_repo,
        };
        service.repair_misclassified_local_sources().await?;
        Ok(service)
    }

    /// Older frontend DTO conversion represented local files as External("unknown", path).
    /// Repair those rows and merge them into their canonical local track when one exists.
    async fn repair_misclassified_local_sources(&self) -> Result<()> {
        use sqlx::Row;

        let mut txn = self.pool.begin().await?;
        let rows = sqlx::query(
            "SELECT id, source_data, liked, liked_position, play_count FROM tracks_v2 WHERE source_type = 'external'",
        )
        .fetch_all(&mut *txn)
        .await?;

        for row in rows {
            let bad_id: i64 = row.get("id");
            let Some(source_data) = row.get::<Option<String>, _>("source_data") else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&source_data) else {
                continue;
            };
            if value.get("system").and_then(|v| v.as_str()) != Some("unknown") {
                continue;
            }
            let Some(path) = value.get("id").and_then(|v| v.as_str()) else {
                continue;
            };
            if !std::path::Path::new(path).is_absolute() {
                continue;
            }

            let local_data = serde_json::to_string(&serde_json::json!({
                "type": "local",
                "path": path,
            }))?;
            let canonical_id = sqlx::query_scalar::<_, i64>(
                "SELECT id FROM tracks_v2 WHERE source_type = 'local' AND source_data = ? LIMIT 1",
            )
            .bind(&local_data)
            .fetch_optional(&mut *txn)
            .await?;

            if let Some(canonical_id) = canonical_id {
                let bad_liked: i64 = row.get("liked");
                let bad_liked_position: Option<i64> = row.get("liked_position");
                let bad_play_count: i64 = row.get("play_count");
                sqlx::query(
                    "UPDATE tracks_v2 SET liked = MAX(liked, ?), liked_position = CASE WHEN liked_position IS NULL THEN ? ELSE liked_position END, play_count = MAX(play_count, ?) WHERE id = ?",
                )
                .bind(bad_liked)
                .bind(bad_liked_position)
                .bind(bad_play_count)
                .bind(canonical_id)
                .execute(&mut *txn)
                .await?;

                // Avoid violating the unique (playlist, type, item) index when both copies exist.
                sqlx::query(
                    "DELETE FROM playlist_items_v2 AS bad WHERE item_type = 'track' AND item_id = ? AND EXISTS (SELECT 1 FROM playlist_items_v2 AS good WHERE good.playlist_id = bad.playlist_id AND good.item_type = 'track' AND good.item_id = ?)",
                )
                .bind(bad_id)
                .bind(canonical_id)
                .execute(&mut *txn)
                .await?;
                sqlx::query(
                    "UPDATE playlist_items_v2 SET item_id = ? WHERE item_type = 'track' AND item_id = ?",
                )
                .bind(canonical_id)
                .bind(bad_id)
                .execute(&mut *txn)
                .await?;

                let canonical_indexed = sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM local_media_index WHERE track_id = ?",
                )
                .bind(canonical_id)
                .fetch_one(&mut *txn)
                .await?
                    > 0;
                if canonical_indexed {
                    sqlx::query("DELETE FROM local_media_index WHERE track_id = ?")
                        .bind(bad_id)
                        .execute(&mut *txn)
                        .await?;
                } else {
                    sqlx::query("UPDATE local_media_index SET track_id = ? WHERE track_id = ?")
                        .bind(canonical_id)
                        .bind(bad_id)
                        .execute(&mut *txn)
                        .await?;
                }
                sqlx::query("DELETE FROM tracks_v2 WHERE id = ?")
                    .bind(bad_id)
                    .execute(&mut *txn)
                    .await?;
            } else {
                sqlx::query(
                    "UPDATE tracks_v2 SET source_type = 'local', source_data = ? WHERE id = ?",
                )
                .bind(local_data)
                .bind(bad_id)
                .execute(&mut *txn)
                .await?;
            }
        }

        txn.commit().await?;
        Ok(())
    }

    // ========================================================================
    // Track Operations
    // ========================================================================

    /// Add a new track to the library
    pub async fn add_track(&self, mut track: models::Track) -> Result<TrackId> {
        tracing::info!(
            "Adding track to library: {} by {:?}",
            track.metadata.title,
            track.artist
        );
        // Resolve artwork if present and ID is 0 (dummy from conversion)
        if let Some(art) = &track.metadata.artwork {
            if art.id.0 == 0 {
                if let crate::library::types::ArtworkSource::Url { url } = &art.source {
                    tracing::info!("Resolving artwork URL: {}", url);
                    let real_id = self.ensure_artwork(url, &self.db).await?;
                    if let Some(mut a) = track.metadata.artwork.take() {
                        a.id = crate::library::types::ArtworkId(real_id);
                        track.metadata.artwork = Some(a);
                    }
                }
            }
        }

        let track_id = self.track_repo.insert(track).await?;
        tracing::info!("Inserted track with ID: {:?}", track_id);
        Ok(track_id)
    }

    /// Get track by ID
    pub async fn get_track(&self, id: TrackId) -> Result<Option<models::Track>> {
        self.track_repo.get(id).await
    }

    /// Get all liked tracks with specified ordering
    pub async fn get_all_tracks(&self, order: TrackOrder) -> Result<Vec<models::Track>> {
        self.track_repo.list_liked(order).await
    }

    /// Get all cached tracks (cache_type = 'cached') with specified ordering
    pub async fn get_cached_tracks(&self, order: TrackOrder) -> Result<Vec<models::Track>> {
        self.track_repo.list_cached(order).await
    }

    /// Get one summary per playlist (album) that has at least one cached track
    pub async fn get_cached_albums(&self) -> Result<Vec<models::CachedAlbumSummary>> {
        self.playlist_repo.list_cached_albums().await
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

    /// Get all tracks regardless of liked state (excludes playlist-only tracks)
    pub async fn list_all_tracks(&self) -> Result<Vec<models::Track>> {
        self.track_repo.list_all().await
    }

    /// Get every track in the database, including those only referenced by playlists
    pub async fn list_all_tracks_flat(&self) -> Result<Vec<models::Track>> {
        self.track_repo.list_all_flat().await
    }

    /// Update track
    pub async fn update_track(&self, track: models::Track) -> Result<()> {
        self.track_repo.update(track).await
    }

    /// Mark a track as unavailable (e.g. 404/403 from streaming provider)
    pub async fn set_track_unavailable(&self, id: TrackId, unavailable: bool) -> Result<()> {
        self.track_repo.set_unavailable(id, unavailable).await
    }

    /// Delete track
    pub async fn delete_track(&self, id: TrackId) -> Result<()> {
        self.track_repo.delete(id).await
    }

    /// Increment play count
    pub async fn increment_play_count(&self, id: TrackId) -> Result<()> {
        self.track_repo.increment_play_count(id).await
    }

    /// Toggle liked state for a track
    pub async fn toggle_liked(&self, id: TrackId) -> Result<()> {
        let track = self.track_repo.get(id).await?.context("Track not found")?;

        let new_liked_state = !track.liked;
        self.track_repo.set_liked(id, new_liked_state).await?;

        // If we unliked the track, check if it's in any playlist.
        // If not, delete it from library entirely.
        if !new_liked_state && !self.is_indexed_local_track(id).await? {
            let in_playlists = self.playlist_repo.is_track_in_any_playlist(id).await?;
            if !in_playlists {
                tracing::info!(
                    "Track {:?} is no longer liked and not in any playlist. Deleting from library.",
                    id
                );
                self.track_repo.delete(id).await?;
            }
        }

        Ok(())
    }

    /// Reorder liked tracks by providing ordered list of track IDs
    pub async fn reorder_liked_tracks(&self, track_ids: Vec<TrackId>) -> Result<()> {
        self.track_repo.reorder_liked(track_ids).await
    }

    pub async fn find_liked_by_sources(
        &self,
        sources: &[crate::library::types::TrackSource],
    ) -> Result<std::collections::HashSet<crate::library::types::TrackSource>> {
        self.track_repo.find_liked_by_sources(sources).await
    }

    /// Find existing track by source or insert a new one
    pub async fn ensure_track(&self, track: models::Track) -> Result<TrackId> {
        if let Some(existing) = self.track_repo.find_by_source(&track.source).await? {
            tracing::info!("Track already exists in library with ID: {:?}", existing.id);
            return Ok(existing.id);
        }
        self.add_track(track).await
    }
    pub async fn find_by_source(&self, source: &TrackSource) -> Result<Option<models::Track>> {
        self.track_repo.find_by_source(&source).await
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

    /// Reclaims tracks that belong to no playlist and are not liked, plus any artwork left
    /// unreferenced by them. Returns (tracks_deleted, artworks_deleted).
    pub async fn cleanup_library(&self) -> Result<(u64, u64)> {
        let tracks = self.track_repo.delete_orphan_unliked().await?;
        // Artworks second: the track rows above must be gone before they count as unreferenced.
        let artworks = self.track_repo.delete_orphan_artworks().await?;
        tracing::info!(
            "Library cleanup removed {} orphan tracks and {} orphan artworks",
            tracks,
            artworks
        );
        Ok((tracks, artworks))
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
            .add_item(
                playlist_id,
                models::PlaylistItemKind::Track,
                track_id.0,
                position,
            )
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
            .add_item(
                parent_id,
                models::PlaylistItemKind::Playlist,
                child_id.0,
                position,
            )
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
        use sea_orm::{
            ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter, TransactionTrait,
        };

        use super::entities_v2::{playlist, playlist_item, track};

        let txn = self.db.begin().await?;

        let mut saved_track_ids = Vec::new();
        let mut seen_track_ids = std::collections::HashSet::new();
        let mut first_artwork = None;
        let mut fallback_url = None;

        for audio_track in runtime_playlist.tracks() {
            let library_track: models::Track = audio_track.clone().into();
            let entity_track: track::Model = library_track.into();

            if let Some(existing) = track::Entity::find()
                .filter(track::Column::SourceType.eq(entity_track.source_type.clone()))
                .filter(track::Column::SourceData.eq(entity_track.source_data.clone()))
                .one(&txn)
                .await
                .context("Failed to find existing track by source")?
            {
                if first_artwork.is_none() {
                    first_artwork = existing.artwork_id;
                }
                if seen_track_ids.insert(existing.id) {
                    saved_track_ids.push(TrackId(existing.id));
                }
                continue;
            }

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
                liked: ActiveValue::Set(entity_track.liked),
                liked_position: ActiveValue::Set(entity_track.liked_position),
                unavailable: ActiveValue::Set(entity_track.unavailable),
            };

            let result = active_track
                .insert(&txn)
                .await
                .context("Failed to insert track")?;

            if seen_track_ids.insert(result.id) {
                saved_track_ids.push(TrackId(result.id));
            }
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

    // ========================================================================
    // Play History Operations
    // ========================================================================

    pub async fn add_play_history(&self, entry: models::PlayHistoryEntry) -> Result<()> {
        self.history_repo.add(entry).await
    }

    pub async fn get_play_history(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<models::PlayHistoryEntry>> {
        self.history_repo.list_recent(limit, offset).await
    }

    pub async fn clear_play_history(&self) -> Result<()> {
        self.history_repo.clear().await
    }

    pub async fn ensure_artwork_url(&self, url: &str) -> Result<i64> {
        self.ensure_artwork(url, &self.db).await
    }

    /// Which of `urls` already belong to something saved in the library.
    ///
    /// An `artworks_v2` row exists only for a saved track or playlist (see the
    /// `ensure_artwork` call sites), so this is the test for "is in the library" —
    /// used to keep transient listings like search results from pulling traffic.
    pub async fn known_artwork_urls(
        &self,
        urls: &[String],
    ) -> Result<std::collections::HashSet<String>> {
        use super::entities_v2::artwork;
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};

        let mut known = std::collections::HashSet::new();
        if urls.is_empty() {
            return Ok(known);
        }

        // Chunked to stay clear of SQLite's bound-parameter ceiling.
        for chunk in urls.chunks(400) {
            let encoded: Vec<String> = chunk.iter().map(|u| artwork_source_data(u)).collect();

            let found: Vec<String> = artwork::Entity::find()
                .filter(artwork::Column::SourceType.eq("url"))
                .filter(artwork::Column::SourceData.is_in(encoded))
                .select_only()
                .column(artwork::Column::SourceData)
                .into_tuple()
                .all(&self.db)
                .await?;

            let found: std::collections::HashSet<String> = found.into_iter().collect();
            known.extend(
                chunk
                    .iter()
                    .filter(|u| found.contains(&artwork_source_data(u)))
                    .cloned(),
            );
        }

        Ok(known)
    }

    async fn ensure_artwork<C>(&self, url: &str, db: &C) -> Result<i64>
    where
        C: sea_orm::ConnectionTrait,
    {
        use super::entities_v2::artwork;
        use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter};

        let source_data = artwork_source_data(url);

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

#[cfg(test)]
mod tests {
    use super::*;

    async fn service() -> (LibraryService, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let service = LibraryService::new(&dir.path().join("test.db")).await.unwrap();
        (service, dir)
    }

    #[tokio::test]
    async fn known_artwork_urls_finds_only_saved_covers() {
        let (service, _guard) = service().await;

        let saved = "https://i1.sndcdn.com/artworks-abc-large.jpg";
        let unsaved = "https://i1.sndcdn.com/artworks-xyz-large.jpg";
        service.ensure_artwork_url(saved).await.unwrap();

        let known = service
            .known_artwork_urls(&[saved.to_string(), unsaved.to_string()])
            .await
            .unwrap();

        assert!(known.contains(saved), "a saved cover must be recognised");
        assert!(
            !known.contains(unsaved),
            "a search-result cover must not be recognised"
        );
        assert_eq!(known.len(), 1);
    }

    /// The lookup rebuilds the same `source_data` string the writer stored, so the
    /// two must agree on URLs that are awkward to embed in JSON.
    #[tokio::test]
    async fn known_artwork_urls_matches_the_writers_encoding() {
        let (service, _guard) = service().await;

        let awkward = [
            "https://f4.bcbits.com/img/a123_0.jpg?a=1&b=2",
            "https://example.com/cover with spaces.jpg",
            "https://example.com/путь/обложка.jpg",
        ];
        for url in awkward {
            service.ensure_artwork_url(url).await.unwrap();
        }

        let urls: Vec<String> = awkward.iter().map(|u| u.to_string()).collect();
        let known = service.known_artwork_urls(&urls).await.unwrap();

        assert_eq!(known.len(), awkward.len(), "round-trip lost a URL: {known:?}");
    }

    #[tokio::test]
    async fn known_artwork_urls_handles_an_empty_batch() {
        let (service, _guard) = service().await;
        assert!(service.known_artwork_urls(&[]).await.unwrap().is_empty());
    }
}
