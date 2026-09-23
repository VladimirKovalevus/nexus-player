use std::collections::HashMap;

use anyhow::Result;
use chrono::Utc;

use crate::library::models::{Metadata, Playlist, PlaylistItemKind, Track};
use crate::library::service::LibraryService;
use crate::library::types::{
    Artwork, ArtworkId, ArtworkSource, AudioInfo, CacheStatus, CodecFormat, PlaylistId, TrackId,
};
use crate::library::LibraryWriteTxn;

use super::types::{SyncFile, SyncPlaylistItem, SyncTrack, SyncType};

pub struct SyncApplier<'a> {
    service: &'a LibraryService,
}

impl<'a> SyncApplier<'a> {
    pub fn new(service: &'a LibraryService) -> Self {
        Self { service }
    }

    pub async fn apply(&self, file: SyncFile) -> Result<()> {
        let txn = self.service.begin_write().await?;
        match file.sync_type {
            SyncType::Full => Self::apply_full(&txn, file).await?,
            SyncType::Diff => Self::apply_diff(&txn, file).await?,
        }
        txn.commit().await
    }

    async fn apply_full(txn: &LibraryWriteTxn, file: SyncFile) -> Result<()> {
        // Step 1: delete all playlists (CASCADE removes their items)
        let playlists = txn.get_all_playlists().await?;
        for playlist in playlists {
            txn.delete_playlist(playlist.id).await?;
        }

        // Step 2: delete all non-local tracks
        let all_tracks = txn.list_all_tracks().await?;
        for track in all_tracks {
            match &track.source {
                crate::audio::types::TrackSource::Local(_) => continue,
                _ => txn.delete_track(track.id).await?,
            }
        }

        // Step 3: insert all tracks from sync file, indexed same as file.tracks
        let track_ids = Self::insert_tracks(txn, &file.tracks).await?;

        // Step 4: create playlists and add items
        Self::create_playlists(txn, &file.playlists, &track_ids).await?;

        Ok(())
    }

    async fn apply_diff(txn: &LibraryWriteTxn, file: SyncFile) -> Result<()> {
        // Step 1: ensure all tracks exist (insert if missing by source), update liked if needed
        let track_ids = Self::ensure_tracks(txn, &file.tracks).await?;

        // Step 2: create playlists (no dedup — duplicates are intentional in diff mode)
        Self::create_playlists(txn, &file.playlists, &track_ids).await?;

        Ok(())
    }

    /// Insert tracks from sync file with their full metadata. Returns TrackIds indexed same as input.
    async fn insert_tracks(
        txn: &LibraryWriteTxn,
        sync_tracks: &[SyncTrack],
    ) -> Result<Vec<TrackId>> {
        let mut ids = Vec::with_capacity(sync_tracks.len());
        for st in sync_tracks {
            let model = sync_track_to_model(st);
            let id = txn.add_track(model).await?;
            ids.push(id);
        }
        Ok(ids)
    }

    /// Ensure tracks exist (find by source or insert). Updates liked state if track already exists.
    /// Returns TrackIds indexed same as input.
    async fn ensure_tracks(
        txn: &LibraryWriteTxn,
        sync_tracks: &[SyncTrack],
    ) -> Result<Vec<TrackId>> {
        let mut ids = Vec::with_capacity(sync_tracks.len());
        for st in sync_tracks {
            let track_source = st.source.clone().into_track_source();
            let id = if let Some(existing) = txn.find_by_source(&track_source).await? {
                if st.liked && !existing.liked {
                    txn.toggle_liked(existing.id).await?;
                }
                existing.id
            } else {
                let model = sync_track_to_model(st);
                txn.add_track(model).await?
            };
            ids.push(id);
        }
        Ok(ids)
    }

    /// Create playlists from sync file and populate their items.
    /// Two-pass: first create all playlists (to get IDs), then add items.
    async fn create_playlists(
        txn: &LibraryWriteTxn,
        sync_playlists: &[super::types::SyncPlaylist],
        track_ids: &[TrackId],
    ) -> Result<()> {
        let now = Utc::now();

        // Pass 1: create all playlists, build sync_id -> PlaylistId map
        let mut sync_id_to_playlist_id: HashMap<usize, PlaylistId> = HashMap::new();
        for sp in sync_playlists {
            let playlist_created_at = sp
                .created_at
                .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
                .unwrap_or(now);
            let artwork = if let Some(url) = &sp.artwork_url {
                let art_id = txn.ensure_artwork_url(url).await?;
                Some(Artwork {
                    id: ArtworkId(art_id),
                    source: ArtworkSource::Url { url: url.clone() },
                    width: None,
                    height: None,
                    created_at: now,
                })
            } else {
                None
            };
            let playlist = Playlist {
                id: PlaylistId(0),
                metadata: Metadata {
                    title: sp.title.clone(),
                    description: sp.description.clone(),
                    artwork,
                },
                creator: sp.creator.clone(),
                created_at: playlist_created_at,
                updated_at: now,
            };
            let id = txn.create_playlist(playlist).await?;
            sync_id_to_playlist_id.insert(sp.sync_id, id);
        }

        // Pass 2: add items to each playlist
        for sp in sync_playlists {
            let playlist_id = match sync_id_to_playlist_id.get(&sp.sync_id) {
                Some(&id) => id,
                None => continue,
            };

            for item in &sp.items {
                match item {
                    SyncPlaylistItem::Track { track_index } => {
                        if let Some(&track_id) = track_ids.get(*track_index) {
                            txn.add_item_to_playlist(
                                playlist_id,
                                PlaylistItemKind::Track,
                                track_id.0,
                            )
                            .await?;
                        } else {
                            tracing::warn!(
                                "Skipping track item in playlist '{}': track_index {} out of bounds",
                                sp.title,
                                track_index
                            );
                        }
                    }
                    SyncPlaylistItem::Playlist { sync_id } => {
                        if let Some(&child_id) = sync_id_to_playlist_id.get(sync_id) {
                            txn.add_item_to_playlist(
                                playlist_id,
                                PlaylistItemKind::Playlist,
                                child_id.0,
                            )
                            .await?;
                        } else {
                            tracing::warn!(
                                "Skipping sub-playlist item in '{}': sync_id {} not found",
                                sp.title,
                                sync_id
                            );
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

fn sync_track_to_model(st: &SyncTrack) -> Track {
    let now = Utc::now();
    let added_at = st
        .added_at
        .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
        .unwrap_or(now);
    let artwork = st.artwork_url.as_ref().map(|url| Artwork {
        id: ArtworkId(0),
        source: ArtworkSource::Url { url: url.clone() },
        width: None,
        height: None,
        created_at: now,
    });
    Track {
        id: TrackId(0),
        metadata: Metadata {
            title: st.title.clone(),
            description: None,
            artwork,
        },
        artist: st.artist.clone(),
        album: st.album.clone(),
        album_artist: st.album_artist.clone(),
        year: st.year,
        genre: st.genre.clone(),
        track_number: st.track_number,
        source: st.source.clone().into_track_source(),
        cache: CacheStatus::None,
        audio_info: st.duration_secs.map(|d| AudioInfo {
            format: CodecFormat::Other,
            duration_secs: Some(d),
            sample_rate: None,
            channels: None,
            bitrate: None,
        }),
        play_count: 0,
        last_played: None,
        added_at,
        updated_at: now,
        liked: st.liked,
        liked_position: st.liked_position,
        unavailable: false,
    }
}
