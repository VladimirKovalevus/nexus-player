//! Domain types for the library module
//!
//! This module contains type-safe wrappers and enums for working with library data.
//! These types provide better type safety and ergonomics compared to raw database types.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// Re-export TrackSource from audio module (single source of truth)
pub use crate::audio::types::TrackSource;

// ============================================================================
// ID Types - Newtype pattern for type safety
// ============================================================================

/// Unique identifier for a track
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TrackId(pub i64);

/// Unique identifier for a playlist
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlaylistId(pub i64);

/// Unique identifier for an artwork/image
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ArtworkId(pub i64);

// ============================================================================
// Artwork Types
// ============================================================================

/// Source of an artwork image
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArtworkSource {
    /// Image stored as binary data in the database
    Data { data: Vec<u8>, mime_type: String },
    /// Image referenced by URL
    Url { url: String },
}

/// Complete artwork information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artwork {
    pub id: ArtworkId,
    pub source: ArtworkSource,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub created_at: DateTime<Utc>,
}

// ============================================================================
// Cache Status
// ============================================================================

/// Status of track caching (for external/http tracks)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheStatus {
    /// Not cached
    None,
    /// Currently downloading
    Downloading,
    /// Successfully cached
    Cached {
        path: PathBuf,
        downloaded_at: DateTime<Utc>,
    },
    /// Download/cache failed
    Failed,
}

// ============================================================================
// Audio Metadata
// ============================================================================

/// Audio file codec/container format (different from audio::AudioFormat which is stream params)
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
    /// Parse format from file extension or format string
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

/// Technical audio parameters stored in database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioInfo {
    pub format: CodecFormat,
    pub duration_secs: Option<f64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
    pub bitrate: Option<u32>,
}
