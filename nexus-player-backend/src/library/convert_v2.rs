use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::library::entities_v2;
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ArtworkSourceData {
    Data { data: Vec<u8>, mime_type: String },
    Url { url: String },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum TrackSourceData {
    Local { path: String },
    Http { url: String },
    External { system: String, id: String },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum CacheData {
    Cached { path: String, downloaded_at: i64 },
}

impl TryFrom<entities_v2::artwork::Model> for Artwork {
    type Error = anyhow::Error;

    fn try_from(entity: entities_v2::artwork::Model) -> Result<Self> {
        let source = match entity.source_type.as_str() {
            "data" | "url" => {
                let data: ArtworkSourceData = serde_json::from_str(
                    entity
                        .source_data
                        .as_deref()
                        .context("source_data is required")?,
                )?;
                match data {
                    ArtworkSourceData::Data { data, mime_type } => {
                        ArtworkSource::Data { data, mime_type }
                    }
                    ArtworkSourceData::Url { url } => ArtworkSource::Url { url },
                }
            }
            _ => bail!("Unknown artwork source type: {}", entity.source_type),
        };

        Ok(Artwork {
            id: ArtworkId(entity.id),
            source,
            width: entity.width.map(|w| w as u32),
            height: entity.height.map(|h| h as u32),
            created_at: timestamp_to_datetime(entity.created_at)?,
        })
    }
}

impl From<Artwork> for entities_v2::artwork::Model {
    fn from(artwork: Artwork) -> Self {
        let (source_type, source_data) = match artwork.source {
            ArtworkSource::Data { data, mime_type } => {
                let data = ArtworkSourceData::Data { data, mime_type };
                ("data".to_string(), serde_json::to_string(&data).ok())
            }
            ArtworkSource::Url { url } => {
                let data = ArtworkSourceData::Url { url };
                ("url".to_string(), serde_json::to_string(&data).ok())
            }
        };

        entities_v2::artwork::Model {
            id: artwork.id.0,
            source_type,
            source_data,
            width: artwork.width.map(|w| w as i32),
            height: artwork.height.map(|h| h as i32),
            created_at: datetime_to_timestamp(artwork.created_at),
        }
    }
}

impl TryFrom<entities_v2::track::Model> for models::Track {
    type Error = anyhow::Error;

    fn try_from(entity: entities_v2::track::Model) -> Result<Self> {
        let source = if let Some(source_data_str) = &entity.source_data {
            let source_data: TrackSourceData = serde_json::from_str(source_data_str)?;
            match source_data {
                TrackSourceData::Local { path } => TrackSource::Local(path.into()),
                TrackSourceData::Http { url } => TrackSource::Http(url),
                TrackSourceData::External { system, id } => TrackSource::External { system, id },
            }
        } else {
            bail!("Track source_data is required");
        };

        let cache = match entity.cache_type.as_str() {
            "none" => CacheStatus::None,
            "downloading" => CacheStatus::Downloading,
            "cached" => {
                if let Some(cache_data_str) = &entity.cache_data {
                    let cache_data: CacheData = serde_json::from_str(cache_data_str)?;
                    match cache_data {
                        CacheData::Cached {
                            path,
                            downloaded_at,
                        } => CacheStatus::Cached {
                            path: path.into(),
                            downloaded_at: timestamp_to_datetime(downloaded_at)?,
                        },
                    }
                } else {
                    bail!("Cached status requires cache_data");
                }
            }
            "failed" => CacheStatus::Failed,
            _ => bail!("Unknown cache type: {}", entity.cache_type),
        };

        let audio_info = if entity.file_format.is_some() || entity.duration_secs.is_some() {
            Some(AudioInfo {
                format: entity
                    .file_format
                    .as_deref()
                    .map(CodecFormat::parse)
                    .unwrap_or(CodecFormat::Other),
                duration_secs: entity.duration_secs,
                sample_rate: entity.sample_rate.map(|r| r as u32),
                channels: entity.channels.map(|c| c as u8),
                bitrate: entity.bitrate.map(|b| b as u32),
            })
        } else {
            None
        };

        Ok(models::Track {
            id: TrackId(entity.id),
            metadata: models::Metadata {
                title: entity.title,
                description: entity.description,
                artwork: None,
            },
            artist: entity.artist,
            album: entity.album,
            album_artist: entity.album_artist,
            year: entity.year.map(|y| y as u32),
            genre: entity.genre,
            track_number: entity.track_number.map(|n| n as u32),
            source,
            cache,
            audio_info,
            play_count: entity.play_count as u64,
            last_played: opt_timestamp_to_datetime(entity.last_played)?,
            added_at: timestamp_to_datetime(entity.added_at)?,
            updated_at: timestamp_to_datetime(entity.updated_at)?,
            liked: entity.liked != 0,
            liked_position: entity.liked_position,
            unavailable: entity.unavailable != 0,
        })
    }
}

impl From<models::Track> for entities_v2::track::Model {
    fn from(track: models::Track) -> Self {
        let (source_type, source_data) = match &track.source {
            TrackSource::Local(path) => {
                let data = TrackSourceData::Local {
                    path: path.display().to_string(),
                };
                ("local".to_string(), serde_json::to_string(&data).ok())
            }
            TrackSource::Http(url) => {
                let data = TrackSourceData::Http { url: url.clone() };
                ("http".to_string(), serde_json::to_string(&data).ok())
            }
            TrackSource::External { system, id } => {
                let data = TrackSourceData::External {
                    system: system.clone(),
                    id: id.clone(),
                };
                ("external".to_string(), serde_json::to_string(&data).ok())
            }
            TrackSource::Cached { .. } => ("cached".to_string(), None),
        };

        let (cache_type, cache_data) = match &track.cache {
            CacheStatus::None => ("none".to_string(), None),
            CacheStatus::Downloading => ("downloading".to_string(), None),
            CacheStatus::Cached {
                path,
                downloaded_at,
            } => {
                let data = CacheData::Cached {
                    path: path.display().to_string(),
                    downloaded_at: datetime_to_timestamp(*downloaded_at),
                };
                ("cached".to_string(), serde_json::to_string(&data).ok())
            }
            CacheStatus::Failed => ("failed".to_string(), None),
        };

        entities_v2::track::Model {
            id: track.id.0,
            title: track.metadata.title,
            artist: track.artist,
            description: track.metadata.description,
            artwork_id: track.metadata.artwork.map(|a| a.id.0),
            album: track.album,
            album_artist: track.album_artist,
            year: track.year.map(|y| y as i32),
            genre: track.genre,
            track_number: track.track_number.map(|n| n as i32),
            source_type,
            source_data,
            cache_type,
            cache_data,
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
            liked: track.liked as i32,
            liked_position: track.liked_position,
            unavailable: track.unavailable as i32,
        }
    }
}

impl TryFrom<entities_v2::playlist::Model> for models::Playlist {
    type Error = anyhow::Error;

    fn try_from(entity: entities_v2::playlist::Model) -> Result<Self> {
        Ok(models::Playlist {
            id: PlaylistId(entity.id),
            metadata: models::Metadata {
                title: entity.title,
                description: entity.description,
                artwork: None,
            },
            creator: entity.creator,
            created_at: timestamp_to_datetime(entity.created_at)?,
            updated_at: timestamp_to_datetime(entity.updated_at)?,
        })
    }
}

impl From<models::Playlist> for entities_v2::playlist::Model {
    fn from(playlist: models::Playlist) -> Self {
        entities_v2::playlist::Model {
            id: playlist.id.0,
            title: playlist.metadata.title,
            description: playlist.metadata.description,
            creator: playlist.creator,
            artwork_id: playlist.metadata.artwork.map(|a| a.id.0),
            created_at: datetime_to_timestamp(playlist.created_at),
            updated_at: datetime_to_timestamp(playlist.updated_at),
        }
    }
}

impl TryFrom<entities_v2::playlist_item::Model> for models::PlaylistItem {
    type Error = anyhow::Error;

    fn try_from(entity: entities_v2::playlist_item::Model) -> Result<Self> {
        let item_kind = match entity.item_type.as_str() {
            "track" => models::PlaylistItemKind::Track,
            "playlist" => models::PlaylistItemKind::Playlist,
            _ => bail!("Unknown playlist item type: {}", entity.item_type),
        };

        Ok(models::PlaylistItem {
            id: entity.id,
            item_kind,
            item_id: entity.item_id,
            position: entity.position as u32,
            added_at: timestamp_to_datetime(entity.added_at)?,
        })
    }
}

impl TryFrom<entities_v2::play_history::Model> for models::PlayHistoryEntry {
    type Error = anyhow::Error;

    fn try_from(entity: entities_v2::play_history::Model) -> Result<Self> {
        let source_data: TrackSourceData = serde_json::from_str(&entity.source_data)?;
        let source = match source_data {
            TrackSourceData::Local { path } => TrackSource::Local(path.into()),
            TrackSourceData::Http { url } => TrackSource::Http(url),
            TrackSourceData::External { system, id } => TrackSource::External { system, id },
        };

        Ok(models::PlayHistoryEntry {
            id: entity.id,
            title: entity.title,
            artist: entity.artist,
            source,
            artwork_url: entity.artwork_url,
            played_at: entity.played_at,
        })
    }
}
