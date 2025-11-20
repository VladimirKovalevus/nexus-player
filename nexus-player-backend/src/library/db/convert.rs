use anyhow::Ok;
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use std::path::PathBuf;

use super::models::*;
use crate::library::models;
use crate::library::types::*;

fn timestamp_to_datetime(timestamp: i64) -> Result<DateTime<Utc>> {
    DateTime::from_timestamp(timestamp, 0).context("Invalid timestamp")
}

fn datetime_to_timestamp(dt: DateTime<Utc>) -> i64 {
    dt.timestamp()
}

fn opt_timestamp_to_datetime(timestamp: Option<i64>) -> Result<Option<DateTime<Utc>>> {
    match timestamp {
        Some(ts) => Ok(Some(timestamp_to_datetime(ts)?)),
        None => Ok(None),
    }
}

impl TryFrom<ArtworkRow> for Artwork {
    type Error = anyhow::Error;

    fn try_from(row: ArtworkRow) -> Result<Self> {
        let source = match (row.data, row.url) {
            (Some(data), _) => ArtworkSource::Data {
                data,
                mime_type: row.mime_type.unwrap_or_else(|| "image/jpeg".to_string()),
            },
            (None, Some(url)) => ArtworkSource::Url { url },
            (None, None) => bail!("Artwork must have either data or url"),
        };

        Ok(Artwork {
            id: ArtworkId(row.id),
            source,
            width: row.width.map(|w| w as u32),
            height: row.height.map(|h| h as u32),
            created_at: timestamp_to_datetime(row.created_at)?,
        })
    }
}

impl From<Artwork> for ArtworkRow {
    fn from(artwork: Artwork) -> Self {
        let (data, url, mime_type) = match artwork.source {
            ArtworkSource::Data { data, mime_type } => (Some(data), None, Some(mime_type)),
            ArtworkSource::Url { url } => (None, Some(url), None),
        };

        ArtworkRow {
            id: artwork.id.0,
            data,
            url,
            mime_type,
            width: artwork.width.map(|w| w as i32),
            height: artwork.height.map(|h| h as i32),
            created_at: datetime_to_timestamp(artwork.created_at),
        }
    }
}

fn parse_track_source(
    source_type: &str,
    local_path: Option<String>,
    http_url: Option<String>,
    external_system: Option<String>,
    external_id: Option<String>,
) -> Result<TrackSource> {
    match source_type {
        "local" => {
            let path = local_path
                .context("local source requires source_local_path field")?
                .into();
            Ok(TrackSource::Local(path))
        }
        "http" => {
            let url = http_url.context("http source requires source_http_url field")?;
            Ok(TrackSource::Http(url))
        }
        "external" => {
            let system =
                external_system.context("external source requires source_external_system field")?;
            let id = external_id.context("external source requires source_external_id field")?;
            Ok(TrackSource::External { system, id })
        }
        _ => bail!("Unknown source type: {}", source_type),
    }
}

fn serialize_track_source(
    source: &TrackSource,
) -> (
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    match source {
        TrackSource::Local(path) => (
            "local".to_string(),
            Some(path.display().to_string()),
            None,
            None,
            None,
        ),
        TrackSource::Http(url) => ("http".to_string(), None, Some(url.clone()), None, None),
        TrackSource::External { system, id } => (
            "external".to_string(),
            None,
            None,
            Some(system.clone()),
            Some(id.clone()),
        ),
    }
}

fn parse_cache_status(
    status: &str,
    cached_path: Option<String>,
    downloaded_at: Option<i64>,
) -> Result<CacheStatus> {
    match status {
        "none" => Ok(CacheStatus::None),
        "downloading" => Ok(CacheStatus::Downloading),
        "cached" => {
            let path = cached_path
                .context("cached status requires cached_local_path")?
                .into();
            let downloaded_at = timestamp_to_datetime(
                downloaded_at.context("cached status requires cache_downloaded_at")?,
            )?;
            Ok(CacheStatus::Cached {
                path,
                downloaded_at,
            })
        }
        "failed" => Ok(CacheStatus::Failed),
        _ => bail!("Unknown cache status: {}", status),
    }
}

fn serialize_cache_status(cache: &CacheStatus) -> (String, Option<String>, Option<i64>) {
    match cache {
        CacheStatus::None => ("none".to_string(), None, None),
        CacheStatus::Downloading => ("downloading".to_string(), None, None),
        CacheStatus::Cached {
            path,
            downloaded_at,
        } => (
            "cached".to_string(),
            Some(path.display().to_string()),
            Some(datetime_to_timestamp(*downloaded_at)),
        ),
        CacheStatus::Failed => ("failed".to_string(), None, None),
    }
}

impl TryFrom<TrackRow> for models::Track {
    type Error = anyhow::Error;

    fn try_from(row: TrackRow) -> Result<Self> {
        let source = parse_track_source(
            &row.source_type,
            row.source_local_path,
            row.source_http_url,
            row.source_external_system,
            row.source_external_id,
        )?;

        let cache = parse_cache_status(
            &row.cache_status,
            row.cached_local_path,
            row.cache_downloaded_at,
        )?;

        let audio_info = if row.file_format.is_some() || row.duration_secs.is_some() {
            Some(AudioInfo {
                format: row
                    .file_format
                    .as_deref()
                    .map(CodecFormat::from_str)
                    .unwrap_or(CodecFormat::Other),
                duration_secs: row.duration_secs,
                sample_rate: row.sample_rate.map(|r| r as u32),
                channels: row.channels.map(|c| c as u8),
                bitrate: row.bitrate.map(|b| b as u32),
            })
        } else {
            None
        };

        Ok(models::Track {
            id: TrackId(row.id),
            metadata: models::Metadata {
                title: row.title,
                description: row.description,
                artwork: None,
            },
            artist: row.artist,
            album: row.album,
            album_artist: row.album_artist,
            year: row.year.map(|y| y as u16),
            genre: row.genre,
            track_number: row.track_number.map(|n| n as u16),
            source,
            cache,
            audio_info,
            play_count: row.play_count as u64,
            last_played: opt_timestamp_to_datetime(row.last_played)?,
            added_at: timestamp_to_datetime(row.added_at)?,
            updated_at: timestamp_to_datetime(row.updated_at)?,
        })
    }
}

impl From<models::Track> for TrackRow {
    fn from(track: models::Track) -> Self {
        let (
            source_type,
            source_local_path,
            source_http_url,
            source_external_system,
            source_external_id,
        ) = serialize_track_source(&track.source);

        let (cache_status, cached_local_path, cache_downloaded_at) =
            serialize_cache_status(&track.cache);

        TrackRow {
            id: track.id.0,
            title: track.metadata.title,
            artist: track.artist,
            description: track.metadata.description,
            artwork_id: None,
            album: track.album,
            album_artist: track.album_artist,
            year: track.year.map(|y| y as i32),
            genre: track.genre,
            track_number: track.track_number.map(|n| n as i32),
            source_type,
            source_local_path,
            source_http_url,
            source_external_system,
            source_external_id,
            cached_local_path,
            cache_status,
            cache_downloaded_at,
            file_format: track
                .audio_info
                .as_ref()
                .map(|i| i.format.as_str().to_string()),
            duration_secs: track.audio_info.as_ref().and_then(|i| i.duration_secs),
            sample_rate: track
                .audio_info
                .as_ref()
                .and_then(|i| i.sample_rate)
                .map(|r| r as i32),
            channels: track
                .audio_info
                .as_ref()
                .and_then(|i| i.channels)
                .map(|c| c as i32),
            bitrate: track
                .audio_info
                .as_ref()
                .and_then(|i| i.bitrate)
                .map(|b| b as i32),
            play_count: track.play_count as i64,
            last_played: track.last_played.map(datetime_to_timestamp),
            added_at: datetime_to_timestamp(track.added_at),
            updated_at: datetime_to_timestamp(track.updated_at),
        }
    }
}

impl TryFrom<PlaylistRow> for models::Playlist {
    type Error = anyhow::Error;

    fn try_from(row: PlaylistRow) -> Result<Self> {
        Ok(models::Playlist {
            id: PlaylistId(row.id),
            metadata: models::Metadata {
                title: row.title,
                description: row.description,
                artwork: None,
            },
            creator: row.creator,
            created_at: timestamp_to_datetime(row.created_at)?,
            updated_at: timestamp_to_datetime(row.updated_at)?,
        })
    }
}

impl From<models::Playlist> for PlaylistRow {
    fn from(playlist: models::Playlist) -> Self {
        PlaylistRow {
            id: playlist.id.0,
            title: playlist.metadata.title,
            description: playlist.metadata.description,
            creator: playlist.creator,
            artwork_id: None,
            created_at: datetime_to_timestamp(playlist.created_at),
            updated_at: datetime_to_timestamp(playlist.updated_at),
        }
    }
}

impl TryFrom<PlaylistTrackRow> for models::PlaylistTrack {
    type Error = anyhow::Error;

    fn try_from(row: PlaylistTrackRow) -> Result<Self> {
        Ok(models::PlaylistTrack {
            track_id: TrackId(row.track_id),
            position: row.position as u32,
            added_at: timestamp_to_datetime(row.added_at)?,
        })
    }
}

impl From<models::PlaylistTrack> for PlaylistTrackRow {
    fn from(track: models::PlaylistTrack) -> Self {
        PlaylistTrackRow {
            playlist_id: 0,
            track_id: track.track_id.0,
            position: track.position as i64,
            added_at: datetime_to_timestamp(track.added_at),
        }
    }
}
