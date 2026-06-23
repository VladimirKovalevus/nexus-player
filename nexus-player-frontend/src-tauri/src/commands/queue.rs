use super::Result;
use crate::dto::{AudioStateDto, LibraryPageDto, TrackDto};
use nexus_player_backend::library::types::{PlaylistId, TrackId, TrackOrder};
use nexus_player_backend::state::state::AppState;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn add_to_queue(state: State<'_, Arc<AppState>>, track_id: i64) -> Result<usize> {
    state
        .add_to_queue(TrackId(track_id))
        .await
        .map_err(|e| e.to_string())?;

    // Return dummy index or calculate real one?
    // In async world, index might change before we return.
    Ok(0)
}

#[tauri::command]
pub async fn remove_from_queue(state: State<'_, Arc<AppState>>, index: usize) -> Result<bool> {
    let removed = state.remove_from_queue(index).await;
    Ok(removed.is_some())
}

#[tauri::command]
pub async fn clear_queue(state: State<'_, Arc<AppState>>) -> Result<()> {
    state.clear_queue().await;
    Ok(())
}

#[tauri::command]
pub async fn get_queue(state: State<'_, Arc<AppState>>) -> Result<Vec<TrackDto>> {
    use nexus_player_backend::playlist::playback_queue::ActiveQueue;
    let queue = state.lock_queue().await;
    match &*queue {
        ActiveQueue::Cursor(cq) => {
            let (tracks, _) = cq.snapshot();
            Ok(tracks.into_iter().map(TrackDto::from).collect())
        }
        ActiveQueue::Search(sq) => {
            // Return flat list of tracks for compatibility
            let (entries, _) = sq.snapshot();
            let mut tracks = Vec::new();
            for entry in entries {
                match entry {
                    nexus_player_backend::playlist::playback_queue::QueueEntry::Track(t) => {
                        tracks.push(TrackDto::from(t));
                    }
                    nexus_player_backend::playlist::playback_queue::QueueEntry::Container {
                        tracks: Some(ts),
                        ..
                    } => {
                        for t in ts {
                            tracks.push(TrackDto::from(t));
                        }
                    }
                    _ => {}
                }
            }
            Ok(tracks)
        }
    }
}

#[tauri::command]
pub async fn load_playlist_to_queue(
    state: State<'_, Arc<AppState>>,
    playlist_id: i64,
) -> Result<()> {
    state
        .load_playlist_to_queue(PlaylistId(playlist_id))
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn load_library_page_to_queue(
    state: State<'_, Arc<AppState>>,
    track_id: i64,
    page_size: Option<usize>,
    order: Option<TrackOrder>,
) -> Result<LibraryPageDto> {
    let page_size = page_size.unwrap_or(50);
    let order = order.unwrap_or_default();

    let (current_page, total_pages, track_index_in_page) = state
        .load_library_page_to_queue(TrackId(track_id), page_size, order)
        .await
        .map_err(|e| e.to_string())?;

    let total_tracks = total_pages * page_size; // approximate; not critical
    Ok(LibraryPageDto {
        current_page,
        total_pages,
        page_size,
        total_tracks,
        track_index_in_page,
    })
}

#[tauri::command]
pub async fn load_library_page(
    state: State<'_, Arc<AppState>>,
    page: usize,
    page_size: usize,
    order: Option<TrackOrder>,
) -> Result<LibraryPageDto> {
    let order = order.unwrap_or_default();

    let total_pages = state
        .load_library_page(page, page_size, order)
        .await
        .map_err(|e| e.to_string())?;

    Ok(LibraryPageDto {
        current_page: page,
        total_pages,
        page_size,
        total_tracks: 0,
        track_index_in_page: 0,
    })
}

#[tauri::command]
pub async fn load_and_play_library_page_at_track(
    state: State<'_, Arc<AppState>>,
    track_id: i64,
    page_size: Option<usize>,
    order: Option<TrackOrder>,
) -> Result<LibraryPageDto> {
    let page_size = page_size.unwrap_or(50);
    let order = order.unwrap_or_default();
    let (current_page, total_pages, track_index_in_page) = state
        .load_and_play_library_page_at_track(TrackId(track_id), page_size, order)
        .await
        .map_err(|e| e.to_string())?;
    Ok(LibraryPageDto {
        current_page,
        total_pages,
        page_size,
        total_tracks: total_pages * page_size,
        track_index_in_page,
    })
}

#[tauri::command]
pub async fn play_library_track(state: State<'_, Arc<AppState>>, track_id: i64) -> Result<()> {
    state
        .play_library_track(TrackId(track_id))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn load_and_play_library_tracks(
    state: State<'_, Arc<AppState>>,
    track_ids: Vec<i64>,
    selected_id: i64,
) -> Result<()> {
    state
        .load_and_play_library_tracks(
            track_ids.into_iter().map(TrackId).collect(),
            TrackId(selected_id),
        )
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn load_and_play_local_tracks(
    state: State<'_, Arc<AppState>>,
    track_ids: Vec<i64>,
    selected_id: i64,
) -> Result<()> {
    state
        .load_and_play_local_tracks(
            track_ids.into_iter().map(TrackId).collect(),
            TrackId(selected_id),
        )
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn load_playlist_and_play(
    state: State<'_, Arc<AppState>>,
    playlist_id: i64,
    start_index: Option<usize>,
) -> Result<()> {
    state
        .load_playlist_and_play(PlaylistId(playlist_id), start_index.unwrap_or(0))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn load_and_play_library_page(
    state: State<'_, Arc<AppState>>,
    page: usize,
    page_size: usize,
    order: Option<TrackOrder>,
) -> Result<LibraryPageDto> {
    let order = order.unwrap_or_default();
    let total_pages = state
        .load_and_play_library_page(page, page_size, order)
        .await
        .map_err(|e| e.to_string())?;
    Ok(LibraryPageDto {
        current_page: page,
        total_pages,
        page_size,
        total_tracks: 0,
        track_index_in_page: 0,
    })
}

#[tauri::command]
pub async fn play_at_index(state: State<'_, Arc<AppState>>, index: usize) -> Result<()> {
    state.play_at_index(index).await;
    Ok(())
}

#[tauri::command]
pub async fn play_next(state: State<'_, Arc<AppState>>) -> Result<()> {
    state.play_next().await;
    Ok(())
}

#[tauri::command]
pub async fn play_previous(state: State<'_, Arc<AppState>>) -> Result<()> {
    state.play_previous().await;
    Ok(())
}

#[tauri::command]
pub fn pause(state: State<'_, Arc<AppState>>) -> Result<()> {
    state.pause();
    Ok(())
}

#[tauri::command]
pub fn resume(state: State<'_, Arc<AppState>>) -> Result<()> {
    state.resume();
    Ok(())
}

#[tauri::command]
pub fn stop(state: State<'_, Arc<AppState>>) -> Result<()> {
    state.stop();
    Ok(())
}

#[tauri::command]
pub fn seek(state: State<'_, Arc<AppState>>, position_secs: f64) -> Result<()> {
    state.seek(std::time::Duration::from_secs_f64(position_secs));
    Ok(())
}

#[tauri::command]
pub fn set_volume(state: State<'_, Arc<AppState>>, volume: f32) -> Result<()> {
    state.set_volume(volume);
    Ok(())
}

#[tauri::command]
pub fn get_audio_state(state: State<'_, Arc<AppState>>) -> Result<AudioStateDto> {
    Ok(state.audio_state().into())
}

#[tauri::command]
pub async fn set_repeat_mode(state: State<'_, Arc<AppState>>, mode: String) -> Result<()> {
    use nexus_player_backend::playlist::playback_queue::RepeatMode;

    let repeat_mode = match mode.as_str() {
        "off" => RepeatMode::Off,
        "all" => RepeatMode::All,
        "one" => RepeatMode::One,
        _ => return Err("Invalid repeat mode".into()),
    };

    state.set_repeat_mode(repeat_mode).await;
    Ok(())
}

#[tauri::command]
pub async fn toggle_shuffle(state: State<'_, Arc<AppState>>) -> Result<()> {
    state.toggle_shuffle().await;
    Ok(())
}

#[tauri::command]
pub async fn start_radio(state: State<'_, Arc<AppState>>, mode: String) -> Result<()> {
    use nexus_player_backend::playlist::cursor::RadioMode;

    let radio_mode = match mode.as_str() {
        "tracks" => RadioMode::Tracks,
        "playlists" => RadioMode::Playlists,
        _ => return Err("Invalid radio mode".into()),
    };

    state
        .start_radio(radio_mode)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_radio(state: State<'_, Arc<AppState>>) -> Result<()> {
    state.stop_radio().await;
    Ok(())
}
