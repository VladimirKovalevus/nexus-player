mod db_v2;

pub mod convert;
pub mod convert_v2;
pub mod entities_v2;
pub mod local_library;
pub mod metadata;
pub mod models;
pub mod repository;
pub mod repository_v2;
pub mod service;
pub mod types;

pub use types::{ArtworkId, AudioInfo, CacheStatus, CodecFormat, PlaylistId, TrackId, TrackSource};

pub use repository::{PlaylistRepository, TrackRepository};
pub use repository_v2::{PlaylistRepositoryV2, TrackRepositoryV2};

pub use db_v2::{SeaOrmPlaylistRepository, SeaOrmTrackRepository};

pub use service::LibraryService;
