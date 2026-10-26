use super::{ensure_artwork_on, LibraryService};
use crate::library::{models, types::TrackOrder, TrackId, TrackSource};
use anyhow::{Context, Result};

impl LibraryService {
    // ========================================================================
    // Track Operations
    // ========================================================================

    /// Add a new track to the library
    pub async fn add_track(&self, mut track: models::Track) -> Result<TrackId> {
        tracing::info!(
            "Adding track to library: {} by {:?}",
            track.metadata.title,
            track.artist
        );
        // Resolve artwork if present and ID is 0 (dummy from conversion)
        if let Some(art) = &track.metadata.artwork {
            if art.id.0 == 0 {
                if let crate::library::types::ArtworkSource::Url { url } = &art.source {
                    tracing::info!("Resolving artwork URL: {}", url);
                    let real_id = ensure_artwork_on(&self.db, url).await?;
                    if let Some(mut a) = track.metadata.artwork.take() {
                        a.id = crate::library::types::ArtworkId(real_id);
                        track.metadata.artwork = Some(a);
                    }
                }
            }
        }

        let track_id = self.track_repo.insert(track).await?;
        tracing::info!("Inserted track with ID: {:?}", track_id);
        Ok(track_id)
    }

    /// Get track by ID
    pub async fn get_track(&self, id: TrackId) -> Result<Option<models::Track>> {
        self.track_repo.get(id).await
    }

    /// Get all liked tracks with specified ordering
    pub async fn get_all_tracks(&self, order: TrackOrder) -> Result<Vec<models::Track>> {
        self.track_repo.list_liked(order).await
    }

    /// Get all cached tracks (cache_type = 'cached') with specified ordering
    pub async fn get_cached_tracks(&self, order: TrackOrder) -> Result<Vec<models::Track>> {
        self.track_repo.list_cached(order).await
    }

    /// Get one summary per playlist (album) that has at least one cached track
    pub async fn get_cached_albums(&self) -> Result<Vec<models::CachedAlbumSummary>> {
        self.playlist_repo.list_cached_albums().await
    }

    /// Search tracks by query
    pub async fn search_tracks(&self, query: &str) -> Result<Vec<models::Track>> {
        self.track_repo.search(query).await
    }

    /// Find tracks by artist
    pub async fn find_by_artist(&self, artist: &str) -> Result<Vec<models::Track>> {
        self.track_repo.find_by_artist(artist).await
    }

    /// Find tracks by album
    pub async fn find_by_album(&self, album: &str) -> Result<Vec<models::Track>> {
        self.track_repo.find_by_album(album).await
    }

    /// Get all tracks regardless of liked state (excludes playlist-only tracks)
    pub async fn list_all_tracks(&self) -> Result<Vec<models::Track>> {
        self.track_repo.list_all().await
    }

    /// Get every track in the database, including those only referenced by playlists
    pub async fn list_all_tracks_flat(&self) -> Result<Vec<models::Track>> {
        self.track_repo.list_all_flat().await
    }

    /// Update track
    pub async fn update_track(&self, track: models::Track) -> Result<()> {
        self.track_repo.update(track).await
    }

    /// Mark a track as unavailable (e.g. 404/403 from streaming provider)
    pub async fn set_track_unavailable(&self, id: TrackId, unavailable: bool) -> Result<()> {
        self.track_repo.set_unavailable(id, unavailable).await
    }

    /// Delete track
    pub async fn delete_track(&self, id: TrackId) -> Result<()> {
        self.track_repo.delete(id).await
    }

    /// Increment play count
    pub async fn increment_play_count(&self, id: TrackId) -> Result<()> {
        self.track_repo.increment_play_count(id).await
    }

    /// Toggle liked state for a track
    pub async fn toggle_liked(&self, id: TrackId) -> Result<()> {
        let track = self.track_repo.get(id).await?.context("Track not found")?;

        let new_liked_state = !track.liked;
        self.track_repo.set_liked(id, new_liked_state).await?;

        // If we unliked the track, check if it's in any playlist.
        // If not, delete it from library entirely.
        if !new_liked_state && !self.is_indexed_local_track(id).await? {
            let in_playlists = self.playlist_repo.is_track_in_any_playlist(id).await?;
            if !in_playlists {
                tracing::info!(
                    "Track {:?} is no longer liked and not in any playlist. Deleting from library.",
                    id
                );
                self.track_repo.delete(id).await?;
            }
        }

        Ok(())
    }

    /// Reorder liked tracks by providing ordered list of track IDs
    pub async fn reorder_liked_tracks(&self, track_ids: Vec<TrackId>) -> Result<()> {
        self.track_repo.reorder_liked(track_ids).await
    }

    pub async fn find_liked_by_sources(
        &self,
        sources: &[crate::library::types::TrackSource],
    ) -> Result<std::collections::HashSet<crate::library::types::TrackSource>> {
        self.track_repo.find_liked_by_sources(sources).await
    }

    /// Find existing track by source or insert a new one
    pub async fn ensure_track(&self, track: models::Track) -> Result<TrackId> {
        if let Some(existing) = self.track_repo.find_by_source(&track.source).await? {
            tracing::info!("Track already exists in library with ID: {:?}", existing.id);
            return Ok(existing.id);
        }
        self.add_track(track).await
    }
    pub async fn find_by_source(&self, source: &TrackSource) -> Result<Option<models::Track>> {
        self.track_repo.find_by_source(source).await
    }
}
