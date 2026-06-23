use super::Result;
use crate::dto::{CachedAlbumDto, TrackDto};
use nexus_player_backend::library::local_library::{AndroidMediaEntry, MusicSource, ScanResult};
use nexus_player_backend::library::types::{TrackId, TrackOrder};
use nexus_player_backend::state::state::AppState;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub async fn get_all_tracks(
    state: State<'_, Arc<AppState>>,
    order: Option<TrackOrder>,
) -> Result<Vec<TrackDto>> {
    let tracks = state
        .get_all_library_tracks(order.unwrap_or_default())
        .await
        .map_err(|e| e.to_string())?;

    Ok(tracks.into_iter().map(|t| t.into()).collect())
}

#[tauri::command]
pub async fn get_cached_tracks(
    state: State<'_, Arc<AppState>>,
    order: Option<TrackOrder>,
) -> Result<Vec<TrackDto>> {
    let tracks = state
        .get_cached_library_tracks(order.unwrap_or_default())
        .await
        .map_err(|e| e.to_string())?;

    Ok(tracks.into_iter().map(|t| t.into()).collect())
}

#[tauri::command]
pub async fn get_cached_albums(state: State<'_, Arc<AppState>>) -> Result<Vec<CachedAlbumDto>> {
    let albums = state
        .get_cached_library_albums()
        .await
        .map_err(|e| e.to_string())?;

    Ok(albums.into_iter().map(|a| a.into()).collect())
}

#[tauri::command]
pub async fn get_track(state: State<'_, Arc<AppState>>, track_id: i64) -> Result<Option<TrackDto>> {
    let track = state
        .get_library_track(TrackId(track_id))
        .await
        .map_err(|e| e.to_string())?;

    Ok(track.map(|t| t.into()))
}

#[tauri::command]
pub async fn search_tracks(
    state: State<'_, Arc<AppState>>,
    query: String,
) -> Result<Vec<TrackDto>> {
    let tracks = state
        .search_library_tracks(&query)
        .await
        .map_err(|e| e.to_string())?;

    Ok(tracks.into_iter().map(|t| t.into()).collect())
}

#[tauri::command]
pub async fn add_track_from_file(state: State<'_, Arc<AppState>>, path: String) -> Result<i64> {
    use nexus_player_backend::library::metadata::{create_track_from_parsed, parse_from_path};

    tracing::info!("Adding track from file: {}", path);

    let file_path = PathBuf::from(&path);

    let parsed = parse_from_path(&file_path).map_err(|e| {
        tracing::error!("Failed to parse metadata for {}: {}", path, e);
        format!("Failed to parse metadata: {}", e)
    })?;

    tracing::info!(
        "Parsed metadata: title={}, artist={:?}, album={:?}",
        parsed.title,
        parsed.artist,
        parsed.album
    );

    let mut track = create_track_from_parsed(file_path, parsed);
    track.liked = true;

    tracing::info!("Created track object, adding to library...");

    let track_id = state.add_track_to_library(track).await.map_err(|e| {
        tracing::error!("Failed to add track to library: {}", e);
        e.to_string()
    })?;

    tracing::info!(
        "Successfully added track to library with ID: {:?}",
        track_id
    );

    Ok(track_id.0)
}

#[tauri::command]
pub async fn get_local_library(state: State<'_, Arc<AppState>>) -> Result<Vec<TrackDto>> {
    let tracks = state
        .library()
        .local_tracks()
        .await
        .map_err(|e| e.to_string())?;
    Ok(tracks.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub async fn scan_local_library(state: State<'_, Arc<AppState>>) -> Result<ScanResult> {
    #[cfg(target_os = "android")]
    return Err("Android scan must be supplied by MediaStore".into());
    #[cfg(not(target_os = "android"))]
    state
        .library()
        .scan_desktop_music()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn import_android_media(
    state: State<'_, Arc<AppState>>,
    entries: Vec<AndroidMediaEntry>,
) -> Result<ScanResult> {
    state
        .library()
        .import_android_media(entries)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_music_sources(state: State<'_, Arc<AppState>>) -> Result<Vec<MusicSource>> {
    state
        .library()
        .music_sources()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[cfg(not(target_os = "android"))]
pub async fn add_music_source(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<MusicSource>> {
    let Some(folder) = app.dialog().file().blocking_pick_folder() else {
        return Ok(None);
    };
    let path = folder.into_path().map_err(|e| e.to_string())?;
    state
        .library()
        .add_music_source(&path)
        .await
        .map_err(|e| e.to_string())?;
    let sources = state
        .library()
        .music_sources()
        .await
        .map_err(|e| e.to_string())?;
    Ok(sources
        .into_iter()
        .find(|source| Path::new(&source.path) == path))
}

#[tauri::command]
#[cfg(target_os = "android")]
pub async fn add_music_source(
    _app: AppHandle,
    _state: State<'_, Arc<AppState>>,
) -> Result<Option<MusicSource>> {
    Err("Extra folders on Android are managed through MediaStore".into())
}

#[tauri::command]
pub async fn remove_music_source(state: State<'_, Arc<AppState>>, source_id: i64) -> Result<()> {
    state
        .library()
        .remove_music_source(source_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_track(state: State<'_, Arc<AppState>>, track_id: i64) -> Result<()> {
    state
        .delete_track_from_library(TrackId(track_id))
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}
