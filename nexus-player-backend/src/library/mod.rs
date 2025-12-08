mod db;
mod db_v2;

pub mod convert;
pub mod convert_v2;
pub mod entities_v2;
pub mod metadata;
pub mod models;
pub mod repository;
pub mod repository_v2;
pub mod types;
pub mod service;
pub mod service_v2;

pub use types::{ArtworkId, AudioInfo, CacheStatus, CodecFormat, PlaylistId, TrackId, TrackSource};

pub use repository::{PlaylistRepository, TrackRepository};
pub use repository_v2::{PlaylistRepositoryV2, TrackRepositoryV2};

pub use db::{SqlitePlaylistRepository, SqliteTrackRepository};
pub use db_v2::{SeaOrmPlaylistRepository, SeaOrmTrackRepository};

pub use service::LibraryService;
pub use service_v2::LibraryServiceV2;
