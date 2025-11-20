//! Library module - manages tracks, playlists, and metadata
//!
//! This module provides a clean domain API for working with the music library.
//! It uses a two-layer architecture:
//! - Public domain types (models, types) - used by application code
//! - Private database layer (db) - handles persistence

mod db;

pub mod models;
pub mod types;

// Re-export commonly used types
pub use types::{TrackId, PlaylistId, ArtworkId, TrackSource, CacheStatus, CodecFormat, AudioInfo};
