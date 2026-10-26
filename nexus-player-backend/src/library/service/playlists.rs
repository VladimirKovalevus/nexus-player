use super::{ensure_artwork_on, LibraryService};
use crate::library::{models, PlaylistId, TrackId};
use anyhow::{Context, Result};

impl LibraryService {
    // ========================================================================
    // Playlist Operations
    // ========================================================================

    /// Create a new playlist
    pub async fn create_playlist(&self, playlist: models::Playlist) -> Result<PlaylistId> {
        self.playlist_repo.create(playlist).await
    }

    /// Get playlist by ID
    pub async fn get_playlist(&self, id: PlaylistId) -> Result<Option<models::Playlist>> {
        self.playlist_repo.get(id).await
    }

    /// Get playlist with items (tracks and sub-playlists)
    pub async fn get_playlist_with_items(
        &self,
        id: PlaylistId,
    ) -> Result<Option<models::PlaylistWithItems>> {
        self.playlist_repo.get_with_items(id).await
    }

    /// Get all playlists
    pub async fn get_all_playlists(&self) -> Result<Vec<models::Playlist>> {
        self.playlist_repo.list_all().await
    }

    /// Update playlist
    pub async fn update_playlist(&self, playlist: models::Playlist) -> Result<()> {
        self.playlist_repo.update(playlist).await
    }

    /// Delete playlist
    pub async fn delete_playlist(&self, id: PlaylistId) -> Result<()> {
        self.playlist_repo.delete(id).await
    }

    /// Reclaims tracks that belong to no playlist and are not liked, plus any artwork left
    /// unreferenced by them. Returns (tracks_deleted, artworks_deleted).
    pub async fn cleanup_library(&self) -> Result<(u64, u64)> {
        let tracks = self.track_repo.delete_orphan_unliked().await?;
        // Artworks second: the track rows above must be gone before they count as unreferenced.
        let artworks = self.track_repo.delete_orphan_artworks().await?;
        tracing::info!(
            "Library cleanup removed {} orphan tracks and {} orphan artworks",
            tracks,
            artworks
        );
        Ok((tracks, artworks))
    }

    /// Add track to playlist
    pub async fn add_track_to_playlist(
        &self,
        playlist_id: PlaylistId,
        track_id: TrackId,
    ) -> Result<()> {
        self.playlist_repo
            .get(playlist_id)
            .await?
            .context("Playlist not found")?;

        self.playlist_repo
            .add_item(playlist_id, models::PlaylistItemKind::Track, track_id.0)
            .await
    }

    /// Remove track from playlist
    pub async fn remove_track_from_playlist(
        &self,
        playlist_id: PlaylistId,
        track_id: TrackId,
    ) -> Result<()> {
        self.playlist_repo
            .remove_item(playlist_id, models::PlaylistItemKind::Track, track_id.0)
            .await
    }

    /// Add sub-playlist to playlist
    pub async fn add_playlist_to_playlist(
        &self,
        parent_id: PlaylistId,
        child_id: PlaylistId,
    ) -> Result<()> {
        self.playlist_repo
            .get(parent_id)
            .await?
            .context("Playlist not found")?;

        self.playlist_repo
            .add_item(parent_id, models::PlaylistItemKind::Playlist, child_id.0)
            .await
    }

    /// Remove sub-playlist from playlist
    pub async fn remove_playlist_from_playlist(
        &self,
        parent_id: PlaylistId,
        child_id: PlaylistId,
    ) -> Result<()> {
        self.playlist_repo
            .remove_item(parent_id, models::PlaylistItemKind::Playlist, child_id.0)
            .await
    }

    /// Reorder items in playlist
    pub async fn reorder_playlist_items(
        &self,
        playlist_id: PlaylistId,
        item_ids: Vec<i64>,
    ) -> Result<()> {
        self.playlist_repo
            .reorder_items(playlist_id, item_ids)
            .await
    }

    /// Create playlist with tracks in a single transaction
    ///
    /// Takes a runtime playlist (from streaming/playback), saves all tracks to library,
    /// creates the playlist, and links tracks together atomically.
    /// Create playlist with tracks in a single transaction
    pub async fn create_playlist_with_tracks(
        &self,
        runtime_playlist: crate::playlist::playlist::Playlist,
    ) -> Result<models::PlaylistWithItems> {
        use chrono::Utc;
        use sea_orm::{
            ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter, TransactionTrait,
        };

        use crate::library::entities_v2::{playlist, playlist_item, track};

        let txn = self.db.begin().await?;

        let mut saved_track_ids = Vec::new();
        let mut seen_track_ids = std::collections::HashSet::new();
        let mut first_artwork = None;
        let mut fallback_url = None;

        for audio_track in runtime_playlist.tracks() {
            let library_track: models::Track = audio_track.clone().into();
            let entity_track: track::Model = library_track.into();

            if let Some(existing) = track::Entity::find()
                .filter(track::Column::SourceType.eq(entity_track.source_type.clone()))
                .filter(track::Column::SourceData.eq(entity_track.source_data.clone()))
                .one(&txn)
                .await
                .context("Failed to find existing track by source")?
            {
                if first_artwork.is_none() {
                    first_artwork = existing.artwork_id;
                }
                if seen_track_ids.insert(existing.id) {
                    saved_track_ids.push(TrackId(existing.id));
                }
                continue;
            }

            let artwork_id = if let Some(url) = &audio_track.artwork_url {
                let id = ensure_artwork_on(&txn, url).await?;
                if first_artwork.is_none() {
                    first_artwork = Some(id);
                    fallback_url = Some(url.clone());
                }
                Some(id)
            } else {
                if first_artwork.is_none() {
                    first_artwork = entity_track.artwork_id;
                }
                entity_track.artwork_id
            };

            let active_track = track::ActiveModel {
                id: ActiveValue::NotSet,
                title: ActiveValue::Set(entity_track.title),
                artist: ActiveValue::Set(entity_track.artist),
                description: ActiveValue::Set(entity_track.description),
                artwork_id: ActiveValue::Set(artwork_id),
                album: ActiveValue::Set(entity_track.album),
                album_artist: ActiveValue::Set(entity_track.album_artist),
                year: ActiveValue::Set(entity_track.year),
                genre: ActiveValue::Set(entity_track.genre),
                track_number: ActiveValue::Set(entity_track.track_number),
                source_type: ActiveValue::Set(entity_track.source_type),
                source_data: ActiveValue::Set(entity_track.source_data),
                cache_type: ActiveValue::Set(entity_track.cache_type),
                cache_data: ActiveValue::Set(entity_track.cache_data),
                file_format: ActiveValue::Set(entity_track.file_format),
                duration_secs: ActiveValue::Set(entity_track.duration_secs),
                sample_rate: ActiveValue::Set(entity_track.sample_rate),
                channels: ActiveValue::Set(entity_track.channels),
                bitrate: ActiveValue::Set(entity_track.bitrate),
                play_count: ActiveValue::Set(entity_track.play_count),
                last_played: ActiveValue::Set(entity_track.last_played),
                added_at: ActiveValue::Set(entity_track.added_at),
                updated_at: ActiveValue::Set(entity_track.updated_at),
                liked: ActiveValue::Set(entity_track.liked),
                liked_position: ActiveValue::Set(entity_track.liked_position),
                unavailable: ActiveValue::Set(entity_track.unavailable),
            };

            let result = active_track
                .insert(&txn)
                .await
                .context("Failed to insert track")?;

            if seen_track_ids.insert(result.id) {
                saved_track_ids.push(TrackId(result.id));
            }
        }

        let now = Utc::now();
        let playlist_library = models::Playlist {
            id: PlaylistId(0),
            metadata: models::Metadata {
                title: runtime_playlist.title.clone(),
                description: runtime_playlist.description.clone(),
                artwork: None,
            },
            creator: None,
            created_at: now,
            updated_at: now,
        };

        let playlist_artwork_id = if let Some(url) = &runtime_playlist.artwork {
            Some(ensure_artwork_on(&txn, url).await?)
        } else {
            first_artwork
        };

        let entity_playlist: playlist::Model = playlist_library.into();
        let active_playlist = playlist::ActiveModel {
            id: ActiveValue::NotSet,
            title: ActiveValue::Set(entity_playlist.title),
            description: ActiveValue::Set(entity_playlist.description),
            creator: ActiveValue::Set(entity_playlist.creator),
            artwork_id: ActiveValue::Set(playlist_artwork_id),
            created_at: ActiveValue::Set(entity_playlist.created_at),
            updated_at: ActiveValue::Set(entity_playlist.updated_at),
        };

        let playlist_result = active_playlist
            .insert(&txn)
            .await
            .context("Failed to create playlist")?;

        let playlist_id = PlaylistId(playlist_result.id);

        let mut playlist_items = Vec::new();

        for (position, track_id) in saved_track_ids.iter().enumerate() {
            let now_ts = Utc::now().timestamp();

            let active_item = playlist_item::ActiveModel {
                id: ActiveValue::NotSet,
                playlist_id: ActiveValue::Set(playlist_id.0),
                item_type: ActiveValue::Set("track".to_string()),
                item_id: ActiveValue::Set(track_id.0),
                position: ActiveValue::Set(position as i64),
                added_at: ActiveValue::Set(now_ts),
            };

            let inserted = active_item
                .insert(&txn)
                .await
                .context("Failed to add item to playlist")?;

            playlist_items.push(models::PlaylistItem {
                id: inserted.id,
                item_kind: models::PlaylistItemKind::Track,
                item_id: track_id.0,
                position: position as u32,
                added_at: Utc::now(),
            });
        }

        txn.commit().await.context("Failed to commit transaction")?;

        let artwork = if let Some(id) = playlist_artwork_id {
            let url = if let Some(url) = &runtime_playlist.artwork {
                Some(url.clone())
            } else {
                fallback_url
            };

            url.map(|url| crate::library::types::Artwork {
                id: crate::library::types::ArtworkId(id),
                source: crate::library::types::ArtworkSource::Url { url },
                width: None,
                height: None,
                created_at: now,
            })
        } else {
            None
        };

        let playlist = models::Playlist {
            id: playlist_id,
            metadata: models::Metadata {
                title: runtime_playlist.title,
                description: runtime_playlist.description,
                artwork,
            },
            creator: None,
            created_at: now,
            updated_at: now,
        };

        Ok(models::PlaylistWithItems {
            playlist,
            items: playlist_items,
        })
    }
}
