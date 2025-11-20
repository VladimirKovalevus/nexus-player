use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use sqlx::SqlitePool;

use super::models::TrackRow;
use crate::library::models::Track;
use crate::library::repository::TrackRepository;
use crate::library::types::TrackId;

pub struct SqliteTrackRepository {
    pool: SqlitePool,
}

impl SqliteTrackRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TrackRepository for SqliteTrackRepository {
    async fn insert(&self, mut track: Track) -> Result<TrackId> {
        let now = Utc::now().timestamp();
        track.added_at = chrono::DateTime::from_timestamp(now, 0).context("Invalid timestamp")?;
        track.updated_at = chrono::DateTime::from_timestamp(now, 0).context("Invalid timestamp")?;

        let row: TrackRow = track.into();

        let result = sqlx::query!(
            r#"
            INSERT INTO tracks (
                title, artist, description, artwork_id,
                album, album_artist, year, genre, track_number,
                source_type, source_local_path, source_http_url,
                source_external_system, source_external_id,
                cached_local_path, cache_status, cache_downloaded_at,
                file_format, duration_secs, sample_rate, channels, bitrate,
                play_count, last_played, added_at, updated_at
            ) VALUES (
                ?1, ?2, ?3, ?4,
                ?5, ?6, ?7, ?8, ?9,
                ?10, ?11, ?12, ?13, ?14,
                ?15, ?16, ?17,
                ?18, ?19, ?20, ?21, ?22,
                ?23, ?24, ?25, ?26
            )
            "#,
            row.title,
            row.artist,
            row.description,
            row.artwork_id,
            row.album,
            row.album_artist,
            row.year,
            row.genre,
            row.track_number,
            row.source_type,
            row.source_local_path,
            row.source_http_url,
            row.source_external_system,
            row.source_external_id,
            row.cached_local_path,
            row.cache_status,
            row.cache_downloaded_at,
            row.file_format,
            row.duration_secs,
            row.sample_rate,
            row.channels,
            row.bitrate,
            row.play_count,
            row.last_played,
            row.added_at,
            row.updated_at
        )
        .execute(&self.pool)
        .await
        .context("Failed to insert track")?;

        Ok(TrackId(result.last_insert_rowid()))
    }

    async fn get(&self, id: TrackId) -> Result<Option<Track>> {
        let row = sqlx::query_as::<_, TrackRow>("SELECT * FROM tracks WHERE id = ?")
            .bind(id.0)
            .fetch_optional(&self.pool)
            .await
            .context("Failed to fetch track")?;

        match row {
            Some(row) => {
                let track = row.try_into()?;
                Ok(Some(track))
            }
            None => Ok(None),
        }
    }

    async fn update(&self, track: Track) -> Result<()> {
        let mut row: TrackRow = track.into();
        row.updated_at = Utc::now().timestamp();

        sqlx::query!(
            r#"
            UPDATE tracks SET
                title = ?1, artist = ?2, description = ?3, artwork_id = ?4,
                album = ?5, album_artist = ?6, year = ?7, genre = ?8, track_number = ?9,
                source_type = ?10, source_local_path = ?11, source_http_url = ?12,
                source_external_system = ?13, source_external_id = ?14,
                cached_local_path = ?15, cache_status = ?16, cache_downloaded_at = ?17,
                file_format = ?18, duration_secs = ?19, sample_rate = ?20, channels = ?21, bitrate = ?22,
                play_count = ?23, last_played = ?24, updated_at = ?25
            WHERE id = ?26
            "#,
            row.title, row.artist, row.description, row.artwork_id,
            row.album, row.album_artist, row.year, row.genre, row.track_number,
            row.source_type, row.source_local_path, row.source_http_url,
            row.source_external_system, row.source_external_id,
            row.cached_local_path, row.cache_status, row.cache_downloaded_at,
            row.file_format, row.duration_secs, row.sample_rate, row.channels, row.bitrate,
            row.play_count, row.last_played, row.updated_at,
            row.id
        )
        .execute(&self.pool)
        .await
        .context("Failed to update track")?;

        Ok(())
    }

    async fn delete(&self, id: TrackId) -> Result<()> {
        sqlx::query!("DELETE FROM tracks WHERE id = ?", id.0)
            .execute(&self.pool)
            .await
            .context("Failed to delete track")?;

        Ok(())
    }

    async fn list_all(&self) -> Result<Vec<Track>> {
        let rows = sqlx::query_as::<_, TrackRow>("SELECT * FROM tracks ORDER BY added_at DESC")
            .fetch_all(&self.pool)
            .await
            .context("Failed to list tracks")?;

        rows.into_iter()
            .map(|row| row.try_into())
            .collect::<Result<Vec<_>>>()
    }

    async fn search(&self, query: &str) -> Result<Vec<Track>> {
        let search_pattern = format!("%{}%", query);

        let rows = sqlx::query_as::<_, TrackRow>(
            r#"
            SELECT * FROM tracks
            WHERE title LIKE ?1 OR artist LIKE ?1 OR album LIKE ?1
            ORDER BY added_at DESC
            "#,
        )
        .bind(&search_pattern)
        .fetch_all(&self.pool)
        .await
        .context("Failed to search tracks")?;

        rows.into_iter()
            .map(|row| row.try_into())
            .collect::<Result<Vec<_>>>()
    }

    async fn find_by_artist(&self, artist: &str) -> Result<Vec<Track>> {
        let rows = sqlx::query_as::<_, TrackRow>(
            "SELECT * FROM tracks WHERE artist = ? ORDER BY year, track_number",
        )
        .bind(artist)
        .fetch_all(&self.pool)
        .await
        .context("Failed to find tracks by artist")?;

        rows.into_iter()
            .map(|row| row.try_into())
            .collect::<Result<Vec<_>>>()
    }

    async fn find_by_album(&self, album: &str) -> Result<Vec<Track>> {
        let rows = sqlx::query_as::<_, TrackRow>(
            "SELECT * FROM tracks WHERE album = ? ORDER BY track_number",
        )
        .bind(album)
        .fetch_all(&self.pool)
        .await
        .context("Failed to find tracks by album")?;

        rows.into_iter()
            .map(|row| row.try_into())
            .collect::<Result<Vec<_>>>()
    }

    async fn increment_play_count(&self, id: TrackId) -> Result<()> {
        let now = Utc::now().timestamp();

        sqlx::query!(
            r#"
            UPDATE tracks
            SET play_count = play_count + 1, last_played = ?1
            WHERE id = ?2
            "#,
            now,
            id.0
        )
        .execute(&self.pool)
        .await
        .context("Failed to increment play count")?;

        Ok(())
    }
}
