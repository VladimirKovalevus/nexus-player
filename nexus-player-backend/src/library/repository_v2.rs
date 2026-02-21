use anyhow::Result;
use async_trait::async_trait;

use crate::library::models::{Playlist, PlaylistItemKind, PlaylistWithItems, Track};
use crate::library::types::{PlaylistId, TrackId, TrackSource};

#[async_trait]
pub trait TrackRepositoryV2: Send + Sync {
    async fn insert(&self, track: Track) -> Result<TrackId>;

    async fn get(&self, id: TrackId) -> Result<Option<Track>>;

    async fn update(&self, track: Track) -> Result<()>;

    async fn delete(&self, id: TrackId) -> Result<()>;

    async fn list_all(&self) -> Result<Vec<Track>>;

    async fn search(&self, query: &str) -> Result<Vec<Track>>;

    async fn find_by_artist(&self, artist: &str) -> Result<Vec<Track>>;

    async fn find_by_album(&self, album: &str) -> Result<Vec<Track>>;

    async fn find_by_source(&self, source: &TrackSource) -> Result<Option<Track>>;

    async fn increment_play_count(&self, id: TrackId) -> Result<()>;
}

#[async_trait]
pub trait PlaylistRepositoryV2: Send + Sync {
    async fn create(&self, playlist: Playlist) -> Result<PlaylistId>;

    async fn get(&self, id: PlaylistId) -> Result<Option<Playlist>>;

    async fn get_with_items(&self, id: PlaylistId) -> Result<Option<PlaylistWithItems>>;

    async fn find_by_title(&self, title: &str) -> Result<Option<Playlist>>;

    async fn update(&self, playlist: Playlist) -> Result<()>;

    async fn delete(&self, id: PlaylistId) -> Result<()>;

    async fn list_all(&self) -> Result<Vec<Playlist>>;

    async fn add_item(
        &self,
        playlist_id: PlaylistId,
        item_kind: PlaylistItemKind,
        item_id: i64,
        position: u32,
    ) -> Result<()>;

    async fn remove_item(
        &self,
        playlist_id: PlaylistId,
        item_kind: PlaylistItemKind,
        item_id: i64,
    ) -> Result<()>;

    async fn reorder_items(&self, playlist_id: PlaylistId, item_ids: Vec<i64>) -> Result<()>;
}
