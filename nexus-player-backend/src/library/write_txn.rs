//! One SQLite transaction around many library writes.
//!
//! `LibraryService` reaches its repositories through `dyn` traits, which cannot
//! carry a caller-chosen connection. The handle here calls the generic
//! `*_on` functions on the concrete repositories instead, so every step runs on
//! the transaction and none touches a pool (the transaction holds SQLite's
//! write lock, so a pool step would wait for the busy timeout and fail).

use anyhow::{Context, Result};
use sea_orm::{ConnectionTrait, DatabaseTransaction, Statement};

use super::models::{Playlist, PlaylistItemKind, Track};
use super::service::ensure_artwork_on;
use super::types::{ArtworkId, ArtworkSource, PlaylistId, TrackId, TrackSource};
use super::{SeaOrmPlaylistRepository, SeaOrmTrackRepository};

/// A write transaction on the library. Dropping it without [`commit`](Self::commit)
/// rolls everything back.
pub struct LibraryWriteTxn {
    txn: DatabaseTransaction,
}

impl LibraryWriteTxn {
    pub(super) fn new(txn: DatabaseTransaction) -> Self {
        Self { txn }
    }

    pub async fn commit(self) -> Result<()> {
        self.txn
            .commit()
            .await
            .context("Failed to commit write transaction")
    }

    pub async fn get_all_playlists(&self) -> Result<Vec<Playlist>> {
        SeaOrmPlaylistRepository::list_all_on(&self.txn).await
    }

    pub async fn delete_playlist(&self, id: PlaylistId) -> Result<()> {
        SeaOrmPlaylistRepository::delete_on(&self.txn, id).await
    }

    /// Every track in the database, including those only referenced by playlists.
    pub async fn list_all_tracks(&self) -> Result<Vec<Track>> {
        SeaOrmTrackRepository::list_all_flat_on(&self.txn).await
    }

    pub async fn delete_track(&self, id: TrackId) -> Result<()> {
        SeaOrmTrackRepository::delete_on(&self.txn, id).await
    }

    pub async fn add_track(&self, mut track: Track) -> Result<TrackId> {
        if let Some(art) = track.metadata.artwork.as_mut() {
            if art.id.0 == 0 {
                if let ArtworkSource::Url { url } = &art.source {
                    let real_id = ensure_artwork_on(&self.txn, url).await?;
                    art.id = ArtworkId(real_id);
                }
            }
        }
        SeaOrmTrackRepository::insert_on(&self.txn, track).await
    }

    pub async fn find_by_source(&self, source: &TrackSource) -> Result<Option<Track>> {
        SeaOrmTrackRepository::find_by_source_on(&self.txn, source).await
    }

    /// Same semantics as `LibraryService::toggle_liked`: unliking a track that is
    /// neither indexed locally nor in a playlist removes it from the library.
    pub async fn toggle_liked(&self, id: TrackId) -> Result<()> {
        let track = SeaOrmTrackRepository::get_on(&self.txn, id)
            .await?
            .context("Track not found")?;
        let new_liked_state = !track.liked;
        SeaOrmTrackRepository::set_liked_on(&self.txn, id, new_liked_state).await?;

        if !new_liked_state {
            let row = self
                .txn
                .query_one(Statement::from_sql_and_values(
                    self.txn.get_database_backend(),
                    "SELECT COUNT(*) AS n FROM local_media_index WHERE track_id = ?",
                    [id.0.into()],
                ))
                .await?
                .context("Local index count returned no row")?;
            let indexed = row.try_get::<i64>("", "n")? > 0;
            if !indexed
                && !SeaOrmPlaylistRepository::is_track_in_any_playlist_on(&self.txn, id).await?
            {
                SeaOrmTrackRepository::delete_on(&self.txn, id).await?;
            }
        }
        Ok(())
    }

    pub async fn ensure_artwork_url(&self, url: &str) -> Result<i64> {
        ensure_artwork_on(&self.txn, url).await
    }

    pub async fn create_playlist(&self, playlist: Playlist) -> Result<PlaylistId> {
        SeaOrmPlaylistRepository::create_on(&self.txn, playlist).await
    }

    pub async fn add_item_to_playlist(
        &self,
        playlist_id: PlaylistId,
        item_kind: PlaylistItemKind,
        item_id: i64,
    ) -> Result<()> {
        SeaOrmPlaylistRepository::add_item_on(&self.txn, playlist_id, item_kind, item_id).await
    }
}
