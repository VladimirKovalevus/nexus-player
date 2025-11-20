//! SQLite implementation of PlaylistRepository

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use sqlx::SqlitePool;

use super::models::{PlaylistRow, PlaylistTrackRow};
use crate::library::models::{Playlist, PlaylistTrack, PlaylistWithTracks};
use crate::library::repository::PlaylistRepository;
use crate::library::types::{PlaylistId, TrackId};

// ============================================================================
// SQLite Playlist Repository
// ============================================================================

pub struct SqlitePlaylistRepository {
    pool: SqlitePool,
}

impl SqlitePlaylistRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PlaylistRepository for SqlitePlaylistRepository {
    async fn create(&self, mut playlist: Playlist) -> Result<PlaylistId> {
        let now = Utc::now().timestamp();
        playlist.created_at =
            chrono::DateTime::from_timestamp(now, 0).context("Invalid timestamp")?;
        playlist.updated_at =
            chrono::DateTime::from_timestamp(now, 0).context("Invalid timestamp")?;

        let row: PlaylistRow = playlist.into();

        let result = sqlx::query!(
            r#"
            INSERT INTO playlists (title, description, creator, artwork_id, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            row.title,
            row.description,
            row.creator,
            row.artwork_id,
            row.created_at,
            row.updated_at
        )
        .execute(&self.pool)
        .await
        .context("Failed to create playlist")?;

        Ok(PlaylistId(result.last_insert_rowid()))
    }

    async fn get(&self, id: PlaylistId) -> Result<Option<Playlist>> {
        let row = sqlx::query_as::<_, PlaylistRow>("SELECT * FROM playlists WHERE id = ?")
            .bind(id.0)
            .fetch_optional(&self.pool)
            .await
            .context("Failed to fetch playlist")?;

        match row {
            Some(row) => {
                let playlist = row.try_into()?;
                Ok(Some(playlist))
            }
            None => Ok(None),
        }
    }

    async fn get_with_tracks(&self, id: PlaylistId) -> Result<Option<PlaylistWithTracks>> {
        // TODO(human): Implement get_with_tracks
        // This method needs to:
        // 1. Fetch the playlist using sqlx::query_as::<_, PlaylistRow>
        // 2. Fetch all playlist_tracks for this playlist using sqlx::query_as::<_, PlaylistTrackRow>
        // 3. Convert rows to domain types using .try_into()
        // 4. Construct PlaylistWithTracks { playlist, tracks }
        // Hint: Use JOIN or two separate queries - your choice!
        let playlist_row = sqlx::query_as::<_, PlaylistRow>(
            r#"
            SELECT * FROM playlists WHERE id = ? 
            "#,
        )
        .bind(id.0)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch playlist")?;
        let Some(playlist_row) = playlist_row else {
            return Ok(None);
        };
        let playlist = playlist_row.try_into()?;

        let tracks_rows = sqlx::query_as::<_, PlaylistTrackRow>(
            r#"
            SELECT * FROM playlist_tracks where playlist_id = ?
            "#,
        )
        .bind(id.0)
        .fetch_all(&self.pool)
        .await
        .context("Failed to fetch tracks")?;
        let tracks: Vec<PlaylistTrack> = tracks_rows
            .into_iter()
            .map(|t| t.try_into())
            .collect::<Result<Vec<_>>>()?;

        Ok(Some(PlaylistWithTracks {
            playlist: playlist,
            tracks: tracks,
        }))
    }

    async fn update(&self, playlist: Playlist) -> Result<()> {
        let mut row: PlaylistRow = playlist.into();
        row.updated_at = Utc::now().timestamp();

        sqlx::query!(
            r#"
            UPDATE playlists SET
                title = ?1, description = ?2, creator = ?3, artwork_id = ?4, updated_at = ?5
            WHERE id = ?6
            "#,
            row.title,
            row.description,
            row.creator,
            row.artwork_id,
            row.updated_at,
            row.id
        )
        .execute(&self.pool)
        .await
        .context("Failed to update playlist")?;

        Ok(())
    }

    async fn delete(&self, id: PlaylistId) -> Result<()> {
        // CASCADE delete will automatically remove playlist_tracks entries
        sqlx::query!("DELETE FROM playlists WHERE id = ?", id.0)
            .execute(&self.pool)
            .await
            .context("Failed to delete playlist")?;

        Ok(())
    }

    async fn list_all(&self) -> Result<Vec<Playlist>> {
        let rows =
            sqlx::query_as::<_, PlaylistRow>("SELECT * FROM playlists ORDER BY created_at DESC")
                .fetch_all(&self.pool)
                .await
                .context("Failed to list playlists")?;

        rows.into_iter()
            .map(|row| row.try_into())
            .collect::<Result<Vec<_>>>()
    }

    async fn add_track(
        &self,
        playlist_id: PlaylistId,
        track_id: TrackId,
        position: u32,
    ) -> Result<()> {
        let now = Utc::now().timestamp();
        let pos = position as i64;  // Store temporary value to extend lifetime

        sqlx::query!(
            r#"
            INSERT INTO playlist_tracks (playlist_id, track_id, position, added_at)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            playlist_id.0,
            track_id.0,
            pos,
            now
        )
        .execute(&self.pool)
        .await
        .context("Failed to add track to playlist")?;

        // Update playlist's updated_at timestamp
        sqlx::query!(
            "UPDATE playlists SET updated_at = ? WHERE id = ?",
            now,
            playlist_id.0
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn remove_track(&self, playlist_id: PlaylistId, track_id: TrackId) -> Result<()> {
        sqlx::query!(
            "DELETE FROM playlist_tracks WHERE playlist_id = ? AND track_id = ?",
            playlist_id.0,
            track_id.0
        )
        .execute(&self.pool)
        .await
        .context("Failed to remove track from playlist")?;

        // Update playlist's updated_at timestamp
        let now = Utc::now().timestamp();
        sqlx::query!(
            "UPDATE playlists SET updated_at = ? WHERE id = ?",
            now,
            playlist_id.0
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn reorder_tracks(&self, playlist_id: PlaylistId, track_ids: Vec<TrackId>) -> Result<()> {
        // Start transaction
        let mut tx = self.pool.begin().await?;

        // Update position for each track
        for (idx, track_id) in track_ids.iter().enumerate() {
            let pos = idx as i64;  // Store temporary value to extend lifetime
            sqlx::query!(
                r#"
                UPDATE playlist_tracks
                SET position = ?1
                WHERE playlist_id = ?2 AND track_id = ?3
                "#,
                pos,
                playlist_id.0,
                track_id.0
            )
            .execute(&mut *tx)
            .await?;
        }

        // Update playlist timestamp
        let now = Utc::now().timestamp();
        sqlx::query!(
            "UPDATE playlists SET updated_at = ? WHERE id = ?",
            now,
            playlist_id.0
        )
        .execute(&mut *tx)
        .await?;

        // Commit transaction
        tx.commit().await?;

        Ok(())
    }
}
