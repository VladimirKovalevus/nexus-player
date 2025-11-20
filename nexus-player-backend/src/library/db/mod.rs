//! Database layer for library module
//!
//! This module contains database-specific types and conversions.
//! It is private to the library module and should not be exposed to the rest of the application.

pub(super) mod models;
pub(super) mod convert;
pub(super) mod track_repo;
pub(super) mod playlist_repo;

// Re-export repository implementations for convenience
pub use track_repo::SqliteTrackRepository;
pub use playlist_repo::SqlitePlaylistRepository;
