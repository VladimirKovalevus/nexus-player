use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub use crate::audio::types::TrackSource;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TrackId(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlaylistId(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ArtworkId(pub i64);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArtworkSource {
    Data { data: Vec<u8>, mime_type: String },

    Url { url: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artwork {
    pub id: ArtworkId,
    pub source: ArtworkSource,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheStatus {
    None,

    Downloading,

    Cached {
        path: PathBuf,
        downloaded_at: DateTime<Utc>,
    },

    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CodecFormat {
    Mp3,
    Flac,
    Ogg,
    Opus,
    Aac,
    Wav,
    Other,
}

impl CodecFormat {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "mp3" => Self::Mp3,
            "flac" => Self::Flac,
            "ogg" => Self::Ogg,
            "opus" => Self::Opus,
            "aac" | "m4a" => Self::Aac,
            "wav" => Self::Wav,
            _ => Self::Other,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Mp3 => "mp3",
            Self::Flac => "flac",
            Self::Ogg => "ogg",
            Self::Opus => "opus",
            Self::Aac => "aac",
            Self::Wav => "wav",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioInfo {
    pub format: CodecFormat,
    pub duration_secs: Option<f64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
    pub bitrate: Option<u32>,
}
