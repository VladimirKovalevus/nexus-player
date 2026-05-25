use std::collections::HashMap;

use anyhow::Result;
use chrono::Utc;

use crate::library::models::PlaylistItemKind;
use crate::library::service::LibraryService;
use crate::library::types::ArtworkSource;

use super::types::{
    SyncFile, SyncPlaylist, SyncPlaylistItem, SyncTrack, SyncTrackSource, SyncType,
};

pub struct SyncBuilder<'a> {
    service: &'a LibraryService,
}

impl<'a> SyncBuilder<'a> {
    pub fn new(service: &'a LibraryService) -> Self {
        Self { service }
    }

    pub async fn build_full(&self, device_name: Option<String>) -> Result<SyncFile> {
        self.build(SyncType::Full, device_name).await
    }

    pub async fn build_diff(&self, device_name: Option<String>) -> Result<SyncFile> {
        self.build(SyncType::Diff, device_name).await
    }

    async fn build(&self, sync_type: SyncType, device_name: Option<String>) -> Result<SyncFile> {
        let all_tracks = self.service.list_all_tracks_flat().await?;

        // Build sync_tracks and track_id -> index map in one pass (only non-local tracks)
        let mut track_id_to_index: HashMap<i64, usize> = HashMap::new();
        let mut sync_tracks: Vec<SyncTrack> = Vec::new();

        for t in &all_tracks {
            if let Some(source) = SyncTrackSource::from_track_source(&t.source) {
                let artwork_url = t.metadata.artwork.as_ref().and_then(|a| match &a.source {
                    ArtworkSource::Url { url } => Some(url.clone()),
                    _ => None,
                });
                let index = sync_tracks.len();
                track_id_to_index.insert(t.id.0, index);
                sync_tracks.push(SyncTrack {
                    title: t.metadata.title.clone(),
                    artist: t.artist.clone(),
                    album: t.album.clone(),
                    album_artist: t.album_artist.clone(),
                    year: t.year,
                    genre: t.genre.clone(),
                    track_number: t.track_number,
                    duration_secs: t.audio_info.as_ref().and_then(|i| i.duration_secs),
                    source,
                    liked: t.liked,
                    liked_position: t.liked_position,
                    artwork_url,
                    added_at: Some(t.added_at.timestamp()),
                });
            }
        }

        let all_playlists = self.service.get_all_playlists().await?;

        // Build sync_id map: playlist DB id -> index in all_playlists
        let playlist_id_to_sync_id: HashMap<i64, usize> = all_playlists
            .iter()
            .enumerate()
            .map(|(i, p)| (p.id.0, i))
            .collect();

        let mut sync_playlists = Vec::new();

        for (sync_id, playlist) in all_playlists.iter().enumerate() {
            let with_items = match self.service.get_playlist_with_items(playlist.id).await? {
                Some(pwi) => pwi,
                None => continue,
            };

            let items: Vec<SyncPlaylistItem> = with_items
                .items
                .iter()
                .filter_map(|item| match item.item_kind {
                    PlaylistItemKind::Track => track_id_to_index
                        .get(&item.item_id)
                        .map(|&track_index| SyncPlaylistItem::Track { track_index }),
                    PlaylistItemKind::Playlist => playlist_id_to_sync_id
                        .get(&item.item_id)
                        .map(|&sid| SyncPlaylistItem::Playlist { sync_id: sid }),
                })
                .collect();

            let artwork_url = playlist
                .metadata
                .artwork
                .as_ref()
                .and_then(|a| match &a.source {
                    ArtworkSource::Url { url } => Some(url.clone()),
                    _ => None,
                });

            sync_playlists.push(SyncPlaylist {
                sync_id,
                title: playlist.metadata.title.clone(),
                description: playlist.metadata.description.clone(),
                creator: playlist.creator.clone(),
                items,
                artwork_url,
                created_at: Some(playlist.created_at.timestamp()),
            });
        }

        Ok(SyncFile {
            version: 1,
            sync_type,
            created_at: Utc::now().timestamp(),
            device_name,
            tracks: sync_tracks,
            playlists: sync_playlists,
        })
    }
}
