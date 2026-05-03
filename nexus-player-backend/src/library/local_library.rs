use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use walkdir::WalkDir;

use super::metadata::{create_track_from_parsed, parse_from_path};
use super::models::Track;
use super::service::LibraryService;
use super::types::TrackId;

const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "ogg", "opus", "aac", "m4a", "wav"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicSource {
    pub id: i64,
    pub path: String,
    pub is_default: bool,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub found: u64,
    pub added: u64,
    pub updated: u64,
    pub skipped: u64,
    pub unavailable: u64,
    pub errors: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidMediaEntry {
    pub source_key: String,
    pub path: String,
    pub size: Option<i64>,
    pub modified_at: Option<i64>,
}

fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn modified_millis(value: SystemTime) -> Option<i64> {
    value
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|v| v.as_millis() as i64)
}

fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|v| v.to_str())
        .map(|v| AUDIO_EXTENSIONS.contains(&v.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

impl LibraryService {
    pub async fn ensure_default_music_source(&self) -> Result<()> {
        if let Some(path) = dirs::audio_dir() {
            let path = path.to_string_lossy().into_owned();
            sqlx::query("INSERT OR IGNORE INTO local_music_sources(path, is_default, created_at) VALUES (?, 1, ?)")
                .bind(path).bind(now()).execute(&self.pool).await?;
        }
        Ok(())
    }

    pub async fn music_sources(&self) -> Result<Vec<MusicSource>> {
        self.ensure_default_music_source().await?;
        let rows = sqlx::query(
            "SELECT id, path, is_default FROM local_music_sources ORDER BY is_default DESC, path",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| MusicSource {
                id: r.get("id"),
                path: r.get("path"),
                is_default: r.get::<i64, _>("is_default") != 0,
            })
            .collect())
    }

    pub async fn add_music_source(&self, path: &Path) -> Result<()> {
        let canonical = path.canonicalize().context("Music folder does not exist")?;
        anyhow::ensure!(canonical.is_dir(), "Music source is not a directory");
        sqlx::query("INSERT OR IGNORE INTO local_music_sources(path, is_default, created_at) VALUES (?, 0, ?)")
            .bind(canonical.to_string_lossy().as_ref()).bind(now()).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn remove_music_source(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM local_music_sources WHERE id = ? AND is_default = 0")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn local_tracks(&self) -> Result<Vec<Track>> {
        let ids: Vec<i64> = sqlx::query_scalar(
            "SELECT track_id FROM local_media_index ORDER BY unavailable, track_id DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut tracks = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(track) = self.get_track(TrackId(id)).await? {
                tracks.push(track);
            }
        }
        Ok(tracks)
    }

    async fn index_path(
        &self,
        source_key: &str,
        path: PathBuf,
        source_id: Option<i64>,
        size: Option<i64>,
        modified: Option<i64>,
        scan: i64,
        result: &mut ScanResult,
    ) {
        result.found += 1;
        let existing = sqlx::query(
            "SELECT track_id, file_size, modified_at FROM local_media_index WHERE source_key = ?",
        )
        .bind(source_key)
        .fetch_optional(&self.pool)
        .await;
        if let Ok(Some(row)) = existing {
            let track_id: i64 = row.get("track_id");
            if row.get::<Option<i64>, _>("file_size") == size
                && row.get::<Option<i64>, _>("modified_at") == modified
            {
                if let Ok(Some(mut track)) = self.get_track(TrackId(track_id)).await {
                    let unknown_title = track.metadata.title.trim().is_empty()
                        || track.metadata.title.trim().eq_ignore_ascii_case("unknown");
                    let unknown_artist = track
                        .artist
                        .as_deref()
                        .map(str::trim)
                        .map(|artist| {
                            artist.is_empty() || artist.eq_ignore_ascii_case("unknown")
                        })
                        .unwrap_or(true);
                    if unknown_title && unknown_artist {
                        if let Some(file_name) = path.file_stem().and_then(|name| name.to_str()) {
                            track.metadata.title = file_name.to_owned();
                            track.updated_at = chrono::Utc::now();
                            let _ = self.update_track(track).await;
                        }
                    }
                }
                let _ = sqlx::query("UPDATE local_media_index SET last_seen_scan = ?, unavailable = 0 WHERE track_id = ?")
                    .bind(scan).bind(track_id).execute(&self.pool).await;
                let _ = self.set_track_unavailable(TrackId(track_id), false).await;
                result.skipped += 1;
                return;
            }
        }

        let mut parsed = match parse_from_path(&path) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(?path, %e, "local metadata read failed");
                result.errors += 1;
                return;
            }
        };
        let unknown_title = parsed.title.trim().is_empty()
            || parsed.title.trim().eq_ignore_ascii_case("unknown");
        let unknown_artist = parsed
            .artist
            .as_deref()
            .map(str::trim)
            .map(|artist| artist.is_empty() || artist.eq_ignore_ascii_case("unknown"))
            .unwrap_or(true);
        if unknown_title && unknown_artist {
            if let Some(file_name) = path.file_stem().and_then(|name| name.to_str()) {
                parsed.title = file_name.to_owned();
            }
        }
        let mut track = create_track_from_parsed(path, parsed);
        let track_id = match self.find_by_source(&track.source).await {
            Ok(Some(existing)) => {
                track.id = existing.id;
                track.liked = existing.liked;
                track.liked_position = existing.liked_position;
                track.play_count = existing.play_count;
                track.last_played = existing.last_played;
                track.added_at = existing.added_at;
                if let Err(e) = self.update_track(track).await {
                    tracing::warn!(%e, "local track update failed");
                    result.errors += 1;
                    return;
                }
                existing.id
            }
            Ok(None) => match self.add_track(track).await {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(%e, "local track insert failed");
                    result.errors += 1;
                    return;
                }
            },
            Err(e) => {
                tracing::warn!(%e, "local track lookup failed");
                result.errors += 1;
                return;
            }
        };
        let existed = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM local_media_index WHERE source_key = ?",
        )
        .bind(source_key)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0)
            > 0;
        if sqlx::query("INSERT INTO local_media_index(track_id, source_key, source_id, file_size, modified_at, last_seen_scan, unavailable) VALUES (?, ?, ?, ?, ?, ?, 0) ON CONFLICT(source_key) DO UPDATE SET track_id=excluded.track_id, source_id=excluded.source_id, file_size=excluded.file_size, modified_at=excluded.modified_at, last_seen_scan=excluded.last_seen_scan, unavailable=0")
            .bind(track_id.0).bind(source_key).bind(source_id).bind(size).bind(modified).bind(scan).execute(&self.pool).await.is_err() { result.errors += 1; return; }
        if existed {
            result.updated += 1
        } else {
            result.added += 1
        }
    }

    pub async fn scan_desktop_music(&self) -> Result<ScanResult> {
        let scan = now();
        let sources = self.music_sources().await?;
        let mut result = ScanResult::default();
        let mut seen = HashSet::new();
        for source in sources {
            for entry in WalkDir::new(&source.path)
                .follow_links(false)
                .into_iter()
                .filter_map(|v| v.ok())
            {
                if !entry.file_type().is_file() || !is_audio(entry.path()) {
                    continue;
                }
                let canonical = match entry.path().canonicalize() {
                    Ok(v) => v,
                    Err(_) => {
                        result.errors += 1;
                        continue;
                    }
                };
                let key = canonical.to_string_lossy().into_owned();
                if !seen.insert(key.clone()) {
                    continue;
                }
                let metadata = entry.metadata().ok();
                let size = metadata.as_ref().map(|v| v.len() as i64);
                let modified = metadata
                    .and_then(|v| v.modified().ok())
                    .and_then(modified_millis);
                self.index_path(
                    &key,
                    canonical,
                    Some(source.id),
                    size,
                    modified,
                    scan,
                    &mut result,
                )
                .await;
            }
        }
        self.mark_missing(scan, &mut result).await?;
        Ok(result)
    }

    pub async fn import_android_media(
        &self,
        entries: Vec<AndroidMediaEntry>,
    ) -> Result<ScanResult> {
        let scan = now();
        let mut result = ScanResult::default();
        for entry in entries {
            self.index_path(
                &entry.source_key,
                PathBuf::from(entry.path),
                None,
                entry.size,
                entry.modified_at,
                scan,
                &mut result,
            )
            .await;
        }
        self.mark_missing(scan, &mut result).await?;
        Ok(result)
    }

    async fn mark_missing(&self, scan: i64, result: &mut ScanResult) -> Result<()> {
        let ids: Vec<i64> = sqlx::query_scalar(
            "SELECT track_id FROM local_media_index WHERE last_seen_scan <> ? AND unavailable = 0",
        )
        .bind(scan)
        .fetch_all(&self.pool)
        .await?;
        for id in &ids {
            self.set_track_unavailable(TrackId(*id), true).await?;
        }
        sqlx::query("UPDATE local_media_index SET unavailable = 1 WHERE last_seen_scan <> ?")
            .bind(scan)
            .execute(&self.pool)
            .await?;
        result.unavailable = ids.len() as u64;
        Ok(())
    }

    pub async fn is_indexed_local_track(&self, id: TrackId) -> Result<bool> {
        Ok(sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM local_media_index WHERE track_id = ?",
        )
        .bind(id.0)
        .fetch_one(&self.pool)
        .await?
            > 0)
    }
}
