use anyhow::Result;
use async_trait::async_trait;

use crate::library::models::{
    CachedAlbumSummary, PlayHistoryEntry, Playlist, PlaylistItemKind, PlaylistWithItems, Track,
};
use crate::library::types::{PlaylistId, TrackId, TrackOrder, TrackSource};

#[async_trait]
pub trait TrackRepositoryV2: Send + Sync {
    async fn insert(&self, track: Track) -> Result<TrackId>;

    async fn get(&self, id: TrackId) -> Result<Option<Track>>;

    async fn update(&self, track: Track) -> Result<()>;

    async fn delete(&self, id: TrackId) -> Result<()>;

    async fn list_all(&self) -> Result<Vec<Track>>;

    /// Returns every track in the database, including those referenced by playlist items.
    async fn list_all_flat(&self) -> Result<Vec<Track>>;

    async fn search(&self, query: &str) -> Result<Vec<Track>>;

    async fn find_by_artist(&self, artist: &str) -> Result<Vec<Track>>;

    async fn find_by_album(&self, album: &str) -> Result<Vec<Track>>;

    async fn find_by_source(&self, source: &TrackSource) -> Result<Option<Track>>;

    async fn increment_play_count(&self, id: TrackId) -> Result<()>;

    async fn list_liked(&self, order: TrackOrder) -> Result<Vec<Track>>;

    /// Returns tracks whose cache_type is 'cached', in the given order.
    async fn list_cached(&self, order: TrackOrder) -> Result<Vec<Track>>;

    async fn set_liked(&self, id: TrackId, liked: bool) -> Result<()>;

    async fn set_unavailable(&self, id: TrackId, unavailable: bool) -> Result<()>;

    async fn reorder_liked(&self, track_ids: Vec<TrackId>) -> Result<()>;

    async fn find_liked_by_sources(&self, sources: &[TrackSource]) -> Result<std::collections::HashSet<TrackSource>>;

    /// Delete tracks that belong to no playlist and are not liked. `playlist_items_v2` has no
    /// foreign key to `tracks_v2`, so removing a playlist leaves its tracks behind forever —
    /// this is how those orphans get reclaimed. Returns the number of rows deleted.
    async fn delete_orphan_unliked(&self) -> Result<u64>;

    /// Delete artwork rows no track or playlist references any more. Returns rows deleted.
    async fn delete_orphan_artworks(&self) -> Result<u64>;
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

    async fn is_track_in_any_playlist(&self, track_id: TrackId) -> Result<bool>;

    /// Returns one summary per playlist that has at least one cached track among its items.
    async fn list_cached_albums(&self) -> Result<Vec<CachedAlbumSummary>>;
}

#[async_trait]
pub trait PlayHistoryRepository: Send + Sync {
    async fn add(&self, entry: PlayHistoryEntry) -> Result<()>;
    async fn list_recent(&self, limit: usize, offset: usize) -> Result<Vec<PlayHistoryEntry>>;
    async fn clear(&self) -> Result<()>;
}
