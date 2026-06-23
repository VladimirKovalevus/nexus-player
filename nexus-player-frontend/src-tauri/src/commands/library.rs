use super::Result;
use crate::dto::{CleanupResultDto, ResolvedArtworkDto};
use nexus_player_backend::state::state::AppState;
use std::sync::Arc;
use streaming::images::ImageVariant;
use tauri::State;

#[tauri::command]
pub async fn cleanup_library(state: State<'_, Arc<AppState>>) -> Result<CleanupResultDto> {
    let (deleted_tracks, deleted_artworks) =
        state.cleanup_library().await.map_err(|e| e.to_string())?;
    Ok(CleanupResultDto {
        deleted_tracks,
        deleted_artworks,
    })
}

/// Resolve cover URLs to local cached files.
///
/// Takes the whole batch in one call: a virtualized list re-resolves every
/// visible row on each scroll pass, so per-row invocations would replace a
/// rendering stall with an IPC one. Uncached covers come back with `path: None`.
/// A fetch is queued only for covers belonging to something saved in the library —
/// search results and other transient listings keep using the remote URL and pull
/// no traffic. The `artwork-cached` event announces each fetched cover as it lands.
#[tauri::command]
pub async fn resolve_artwork(
    state: State<'_, Arc<AppState>>,
    urls: Vec<String>,
    variant: Option<String>,
) -> Result<Vec<ResolvedArtworkDto>> {
    let variant = variant
        .as_deref()
        .and_then(ImageVariant::parse)
        .unwrap_or(ImageVariant::Thumb);

    let paths = state.resolve_artwork(&urls, variant).await;

    Ok(urls
        .into_iter()
        .zip(paths)
        .map(|(url, path)| ResolvedArtworkDto { url, path })
        .collect())
}

#[tauri::command]
pub async fn clear_artwork_cache(state: State<'_, Arc<AppState>>) -> Result<()> {
    state.clear_artwork_cache().await.map_err(|e| e.to_string())
}
