//! Library module - manages tracks, playlists, and metadata
//!
//! This module provides a clean domain API for working with the music library.
//! It uses a three-layer architecture:
//! - Public domain types (models, types) - used by application code
//! - Repository traits (repository) - define data access interfaces
//! - Private database layer (db) - handles persistence

mod db;

pub mod models;
pub mod types;
pub mod repository;

// Re-export commonly used types
pub use types::{TrackId, PlaylistId, ArtworkId, TrackSource, CacheStatus, CodecFormat, AudioInfo};

// Re-export repository traits
pub use repository::{TrackRepository, PlaylistRepository};

// Re-export repository implementations
pub use db::{SqliteTrackRepository, SqlitePlaylistRepository};
