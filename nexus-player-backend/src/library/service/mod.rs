//! LibraryService - facade for all library operations
//!
//! This service encapsulates database connection and repositories,
//! providing a clean API for track and playlist management.

use anyhow::{Context, Result};
use sea_orm::DatabaseConnection;
use sqlx::SqlitePool;
use std::path::Path;
use std::sync::Arc;

mod artwork;
mod history;
mod playlists;
mod tracks;

use super::{
    db_v2, PlaylistRepositoryV2, SeaOrmPlaylistRepository, SeaOrmTrackRepository, TrackRepositoryV2,
};
use crate::db;
use crate::library::repository_v2::PlayHistoryRepository;
use chrono::Utc;

/// The `artworks_v2.source_data` encoding for a URL artwork.
///
/// Shared by the writer and every lookup, so comparisons stay exact.
fn artwork_source_data(url: &str) -> String {
    format!(r#"{{"type":"url","url":"{}"}}"#, url)
}

pub(in crate::library) async fn ensure_artwork_on<C>(db: &C, url: &str) -> Result<i64>
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
    ///
    /// Raw sqlx rather than SeaORM: this is a one-off data migration that also rewrites
    /// `local_media_index`, which has no SeaORM entity (`local_library.rs` owns it through
    /// the same pool), and its merge statements (`MAX(...)`, correlated `DELETE ... EXISTS`)
    /// read more plainly as SQL. It runs before any repository is used, so it cannot race
    /// with SeaORM writes.
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

    /// Begins one transaction for a group of library writes; see [`LibraryWriteTxn`].
    pub async fn begin_write(&self) -> Result<super::write_txn::LibraryWriteTxn> {
        let txn = sea_orm::TransactionTrait::begin(&self.db)
            .await
            .context("Failed to begin write transaction")?;
        Ok(super::write_txn::LibraryWriteTxn::new(txn))
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
mod tests;
