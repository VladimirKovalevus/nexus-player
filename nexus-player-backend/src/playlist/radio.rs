//! Builds the fixed play order for a radio session.
//!
//! The order is computed once, when radio is switched on, and then stored inside
//! `ContainerContext::Radio` so playback follows exactly that order — unlike
//! `CursorQueue::shuffle_enabled`, which re-randomises inside the prefetch window on
//! every `next()` and therefore never remembers anything.

use anyhow::Result;
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::collections::HashSet;

use crate::library::models::PlaylistItemKind;
use crate::library::types::PlaylistId;
use crate::library::LibraryService;
use crate::playlist::cursor::{RadioBlock, RadioMode};
use anyhow::Context;
use std::future::Future;
use std::pin::Pin;

/// Builds the frozen radio order for `mode` as blocks: each root playlist whole (shown as
/// one container row) or a single loose track. Already shuffled and with unavailable
/// tracks removed.
pub async fn build_radio_blocks(
    library: &LibraryService,
    mode: RadioMode,
) -> Result<Vec<RadioBlock>> {
    match mode {
        RadioMode::Tracks => Ok(build_tracks_order(library)
            .await?
            .into_iter()
            .map(|id| RadioBlock {
                playlist_id: None,
                track_ids: vec![id],
            })
            .collect()),
        RadioMode::Playlists => build_playlists_order(library).await,
    }
}

/// The radio order for `mode` as raw track ids in play order.
pub async fn build_radio_order(library: &LibraryService, mode: RadioMode) -> Result<Vec<i64>> {
    Ok(build_radio_blocks(library, mode)
        .await?
        .into_iter()
        .flat_map(|b| b.track_ids)
        .collect())
}

/// Every track in the library, in random order.
async fn build_tracks_order(library: &LibraryService) -> Result<Vec<i64>> {
    // `list_all_tracks_flat` is the whole `track` table. Note `get_all_tracks` would be
    // wrong here — it only returns liked tracks.
    let tracks = library.list_all_tracks_flat().await?;

    let mut ids: Vec<i64> = tracks
        .into_iter()
        .filter(|t| !t.unavailable)
        .map(|t| t.id.0)
        .collect();

    // thread_rng() is !Send, so it must not be held across an await.
    let mut rng = thread_rng();
    ids.shuffle(&mut rng);

    Ok(ids)
}

/// Root playlists (each kept whole and in its own order) interleaved at random with the
/// tracks that belong to no playlist. Shuffling a list of *blocks* is what keeps playlist
/// tracks contiguous while still letting loose tracks land between playlists.
async fn build_playlists_order(library: &LibraryService) -> Result<Vec<RadioBlock>> {
    let unavailable = unavailable_track_ids(library).await?;

    let mut blocks: Vec<RadioBlock> = Vec::new();

    for playlist_id in root_playlist_ids(library).await? {
        let mut path = Vec::new();
        let track_ids = flatten_playlist_tracks(library, playlist_id, &mut path).await?;

        let block: Vec<i64> = track_ids
            .into_iter()
            .filter(|id| !unavailable.contains(id))
            .collect();

        if !block.is_empty() {
            blocks.push(RadioBlock {
                playlist_id: Some(playlist_id.0),
                track_ids: block,
            });
        }
    }

    let playlist_blocks = blocks.len();
    let playlist_tracks: usize = blocks.iter().map(|b| b.track_ids.len()).sum();

    // `list_all_tracks` is already "tracks not referenced by any playlist_item".
    for track in library.list_all_tracks().await? {
        if !track.unavailable {
            blocks.push(RadioBlock {
                playlist_id: None,
                track_ids: vec![track.id.0],
            });
        }
    }

    tracing::info!(
        "Radio (playlists): {} playlist blocks ({} tracks) + {} loose tracks",
        playlist_blocks,
        playlist_tracks,
        blocks.len() - playlist_blocks
    );

    let mut rng = thread_rng();
    blocks.shuffle(&mut rng);

    Ok(blocks)
}

/// Playlists that are not an item of any other playlist. Nested playlists are expanded
/// inside their parent, so making them blocks of their own would duplicate their tracks.
async fn root_playlist_ids(library: &LibraryService) -> Result<Vec<PlaylistId>> {
    let playlists = library.get_all_playlists().await?;

    let mut nested: HashSet<i64> = HashSet::new();
    for playlist in &playlists {
        if let Some(pwi) = library.get_playlist_with_items(playlist.id).await? {
            for item in pwi.items {
                if item.item_kind == PlaylistItemKind::Playlist {
                    nested.insert(item.item_id);
                }
            }
        }
    }

    Ok(playlists
        .into_iter()
        .map(|p| p.id)
        .filter(|id| !nested.contains(&id.0))
        .collect())
}

/// Ids flagged unavailable. Needed because flattening a playlist yields ids only, and
/// resolving each one individually would be an N+1 query.
async fn unavailable_track_ids(library: &LibraryService) -> Result<HashSet<i64>> {
    Ok(library
        .list_all_tracks_flat()
        .await?
        .into_iter()
        .filter(|t| t.unavailable)
        .map(|t| t.id.0)
        .collect())
}

/// Flattens a playlist into track ids, expanding nested playlists in place. `path` holds
/// the playlists on the way down: one already on it is skipped (cycle guard), while a
/// playlist reached again through another branch is expanded again.
fn flatten_playlist_tracks<'a>(
    library: &'a LibraryService,
    playlist_id: PlaylistId,
    path: &'a mut Vec<i64>,
) -> Pin<Box<dyn Future<Output = Result<Vec<i64>>> + Send + 'a>> {
    Box::pin(async move {
        if path.contains(&playlist_id.0) {
            return Ok(vec![]);
        }
        let pwi = library
            .get_playlist_with_items(playlist_id)
            .await?
            .context("Playlist not found")?;

        path.push(playlist_id.0);
        let mut track_ids = Vec::new();
        for item in pwi.items {
            match item.item_kind {
                PlaylistItemKind::Track => track_ids.push(item.item_id),
                PlaylistItemKind::Playlist => {
                    let nested =
                        flatten_playlist_tracks(library, PlaylistId(item.item_id), path).await?;
                    track_ids.extend(nested);
                }
            }
        }
        path.pop();
        Ok(track_ids)
    })
}
