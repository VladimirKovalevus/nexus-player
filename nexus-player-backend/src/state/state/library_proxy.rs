use super::AppState;
use crate::audio::types::Track as AudioTrack;
use crate::library::models::{
    Metadata, PlayHistoryEntry, Playlist as LibraryPlaylist, Track as LibraryTrack,
};
use crate::library::types::TrackOrder;
use crate::library::{PlaylistId, TrackId};
use crate::playlist::playlist::Playlist;
use crate::streaming::types::ContentType;
use anyhow::{Context, Result};

impl AppState {
    // --- Library Proxies (unchanged mostly) ---

    pub async fn add_track_to_library(&self, track: LibraryTrack) -> Result<TrackId> {
        self.library.add_track(track).await
    }

    pub async fn get_all_library_tracks(&self, order: TrackOrder) -> Result<Vec<LibraryTrack>> {
        self.library.get_all_tracks(order).await
    }

    pub async fn get_cached_library_tracks(&self, order: TrackOrder) -> Result<Vec<LibraryTrack>> {
        self.library.get_cached_tracks(order).await
    }

    pub async fn get_cached_library_albums(
        &self,
    ) -> Result<Vec<crate::library::models::CachedAlbumSummary>> {
        self.library.get_cached_albums().await
    }

    pub async fn search_library_tracks(&self, query: &str) -> Result<Vec<LibraryTrack>> {
        self.library.search_tracks(query).await
    }

    pub async fn delete_track_from_library(&self, track_id: TrackId) -> Result<()> {
        self.library.delete_track(track_id).await
    }

    pub async fn get_library_track(&self, track_id: TrackId) -> Result<Option<LibraryTrack>> {
        self.library.get_track(track_id).await
    }

    pub async fn update_track(&self, track: LibraryTrack) -> Result<()> {
        self.library.update_track(track).await
    }

    pub async fn create_playlist(
        &self,
        title: String,
        description: Option<String>,
        creator: Option<String>,
        artwork_url: Option<String>,
    ) -> Result<PlaylistId> {
        let artwork = artwork_url.map(|url| crate::library::types::Artwork {
            id: crate::library::types::ArtworkId(0),
            source: crate::library::types::ArtworkSource::Url { url },
            width: None,
            height: None,
            created_at: chrono::Utc::now(),
        });

        let playlist = LibraryPlaylist {
            id: PlaylistId(0),
            metadata: Metadata {
                title,
                description,
                artwork,
            },
            creator,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        self.library.create_playlist(playlist).await
    }

    pub async fn get_all_playlists(&self) -> Result<Vec<LibraryPlaylist>> {
        self.library.get_all_playlists().await
    }

    pub async fn get_playlist(
        &self,
        playlist_id: PlaylistId,
    ) -> Result<Option<crate::library::models::Playlist>> {
        self.library.get_playlist(playlist_id).await
    }

    pub async fn get_playlist_with_items(
        &self,
        playlist_id: PlaylistId,
    ) -> Result<Option<crate::library::models::PlaylistWithItems>> {
        self.library.get_playlist_with_items(playlist_id).await
    }

    pub async fn add_playlist_to_playlist(
        &self,
        parent_id: PlaylistId,
        child_id: PlaylistId,
    ) -> Result<()> {
        self.library
            .add_playlist_to_playlist(parent_id, child_id)
            .await
    }

    pub async fn remove_playlist_from_playlist(
        &self,
        parent_id: PlaylistId,
        child_id: PlaylistId,
    ) -> Result<()> {
        self.library
            .remove_playlist_from_playlist(parent_id, child_id)
            .await
    }

    pub async fn add_track_to_playlist(
        &self,
        playlist_id: PlaylistId,
        track: AudioTrack,
    ) -> Result<()> {
        let library_track: LibraryTrack = track.into();
        let track_id = self.library.ensure_track(library_track).await?;
        tracing::info!("ENSURE TRACK {track_id:?}");

        self.library
            .add_track_to_playlist(playlist_id, track_id)
            .await
    }

    pub async fn remove_track_from_playlist(
        &self,
        playlist_id: PlaylistId,
        track_id: TrackId,
    ) -> Result<()> {
        self.library
            .remove_track_from_playlist(playlist_id, track_id)
            .await
    }

    pub async fn delete_playlist(&self, playlist_id: PlaylistId) -> Result<()> {
        self.library.delete_playlist(playlist_id).await?;
        // The playlist's tracks are now orphaned — reclaim the ones that aren't liked.
        // Tracks still referenced by another playlist are kept, otherwise deleting one
        // playlist would silently gut the others.
        self.library.cleanup_library().await?;
        Ok(())
    }

    /// Manual library cleanup. Returns (tracks_deleted, artworks_deleted).
    pub async fn cleanup_library(&self) -> Result<(u64, u64)> {
        self.library.cleanup_library().await
    }

    pub async fn create_playlist_with_tracks(
        &self,
        title: String,
        description: Option<String>,
        artwork_url: Option<String>,
        tracks: Vec<AudioTrack>,
    ) -> Result<PlaylistId> {
        let mut playlist = Playlist::new(title);
        playlist.description = description;
        playlist.artwork = artwork_url;
        playlist.add_tracks(tracks);
        let result = self.library.create_playlist_with_tracks(playlist).await?;
        Ok(result.playlist.id)
    }

    pub async fn create_playlist_from_container(
        &self,
        provider: String,
        content_type: ContentType,
        id: String,
        title: String,
        description: Option<String>,
        artwork_url: Option<String>,
    ) -> Result<PlaylistId> {
        let track_infos = self
            .streaming
            .expand_collection(&provider, content_type, &id)
            .await
            .context("Failed to expand container")?;

        let audio_tracks: Vec<AudioTrack> = track_infos.into_iter().map(|t| t.into()).collect();

        self.create_playlist_with_tracks(title, description, artwork_url, audio_tracks)
            .await
    }

    // --- Liked Tracks ---

    pub async fn toggle_liked(&self, track: AudioTrack) -> Result<()> {
        let library_track: LibraryTrack = track.into();
        let track_id = self.library.ensure_track(library_track).await?;
        self.library.toggle_liked(track_id).await
    }

    pub async fn reorder_liked_tracks(&self, track_ids: Vec<TrackId>) -> Result<()> {
        self.library.reorder_liked_tracks(track_ids).await
    }

    pub async fn reorder_playlist_items(
        &self,
        playlist_id: PlaylistId,
        item_ids: Vec<i64>,
    ) -> Result<()> {
        self.library
            .reorder_playlist_items(playlist_id, item_ids)
            .await
    }

    // --- Play History ---

    pub async fn get_play_history(
        &self,
        limit: usize,
        offset: usize,
    ) -> anyhow::Result<Vec<PlayHistoryEntry>> {
        self.library.get_play_history(limit, offset).await
    }

    pub async fn clear_play_history(&self) -> anyhow::Result<()> {
        self.library.clear_play_history().await
    }
}
