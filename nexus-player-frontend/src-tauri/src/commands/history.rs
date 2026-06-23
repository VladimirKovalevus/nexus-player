use super::Result;
use crate::dto::{PlayHistoryEntryDto, TaskInfoDto, TrackDto};
use nexus_player_backend::library::types::{PlaylistId, TrackId};
use nexus_player_backend::state::state::AppState;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn get_play_history(
    state: State<'_, Arc<AppState>>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<PlayHistoryEntryDto>> {
    let entries = state
        .get_play_history(limit.unwrap_or(50), offset.unwrap_or(0))
        .await
        .map_err(|e| e.to_string())?;
    Ok(entries.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub async fn clear_play_history(state: State<'_, Arc<AppState>>) -> Result<()> {
    state
        .clear_play_history()
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn toggle_liked(state: State<'_, Arc<AppState>>, track: TrackDto) -> Result<()> {
    state
        .toggle_liked(track.try_to_audio_track()?)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reorder_liked_tracks(
    state: State<'_, Arc<AppState>>,
    track_ids: Vec<i64>,
) -> Result<()> {
    let ids: Vec<TrackId> = track_ids.into_iter().map(TrackId).collect();
    state
        .reorder_liked_tracks(ids)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reorder_playlist_items(
    state: State<'_, Arc<AppState>>,
    playlist_id: i64,
    item_ids: Vec<i64>,
) -> Result<()> {
    state
        .reorder_playlist_items(PlaylistId(playlist_id), item_ids)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_download_tasks(state: State<'_, Arc<AppState>>) -> Result<Vec<TaskInfoDto>> {
    let tasks = state.streaming().list_tasks().await;
    Ok(tasks.into_iter().map(|t| t.into()).collect())
}

#[tauri::command]
pub async fn remove_download_task(state: State<'_, Arc<AppState>>, track_id: u64) -> Result<()> {
    state.streaming().remove_task(track_id).await;
    Ok(())
}
