use crate::dto::*;
use nexus_player_backend::library::models::{Metadata, Track as LibraryTrack};
use nexus_player_backend::library::types::{
    AudioInfo, CacheStatus, CodecFormat, PlaylistId, TrackId, TrackSource,
};
use nexus_player_backend::state::state::AppState;
use nexus_player_backend::streaming::types::ContentType;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

type Result<T> = std::result::Result<T, String>;

#[tauri::command]
pub async fn get_all_tracks(state: State<'_, Arc<AppState>>) -> Result<Vec<TrackDto>> {
    let tracks = state
        .get_all_library_tracks()
        .await
        .map_err(|e| e.to_string())?;

    Ok(tracks.into_iter().map(|t| t.into()).collect())
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

    let file_path = PathBuf::from(&path);

    let parsed =
        parse_from_path(&file_path).map_err(|e| format!("Failed to parse metadata: {}", e))?;

    let track = create_track_from_parsed(file_path, parsed);

    let track_id = state
        .add_track_to_library(track)
        .await
        .map_err(|e| e.to_string())?;

    Ok(track_id.0)
}

#[tauri::command]
pub async fn delete_track(state: State<'_, Arc<AppState>>, track_id: i64) -> Result<()> {
    state
        .delete_track_from_library(TrackId(track_id))
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn get_all_playlists(state: State<'_, Arc<AppState>>) -> Result<Vec<PlaylistDto>> {
    let playlists = state.get_all_playlists().await.map_err(|e| e.to_string())?;

    Ok(playlists.into_iter().map(|p| p.into()).collect())
}

#[tauri::command]
pub async fn get_playlist_with_tracks(
    state: State<'_, Arc<AppState>>,
    playlist_id: i64,
) -> Result<Option<PlaylistWithTracksDto>> {
    let playlist = state
        .get_playlist_with_tracks(PlaylistId(playlist_id))
        .await
        .map_err(|e| e.to_string())?;

    if let Some(pwt) = playlist {
        let mut tracks = Vec::new();
        for pt in &pwt.tracks {
            if let Some(track) = state
                .get_library_track(pt.track_id)
                .await
                .map_err(|e| e.to_string())?
            {
                tracks.push(track.into());
            }
        }

        let mut playlist_dto: PlaylistDto = pwt.playlist.into();
        playlist_dto.track_count = tracks.len();

        Ok(Some(PlaylistWithTracksDto {
            playlist: playlist_dto,
            tracks,
        }))
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub async fn create_playlist(
    state: State<'_, Arc<AppState>>,
    request: CreatePlaylistRequest,
) -> Result<i64> {
    let playlist_id = state
        .create_playlist(request.title, request.description)
        .await
        .map_err(|e| e.to_string())?;

    Ok(playlist_id.0)
}

#[tauri::command]
pub async fn delete_playlist(state: State<'_, Arc<AppState>>, playlist_id: i64) -> Result<()> {
    state
        .delete_playlist(PlaylistId(playlist_id))
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn add_track_to_playlist(
    state: State<'_, Arc<AppState>>,
    playlist_id: i64,
    track_id: i64,
) -> Result<()> {
    state
        .add_track_to_playlist(PlaylistId(playlist_id), TrackId(track_id))
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn remove_track_from_playlist(
    state: State<'_, Arc<AppState>>,
    playlist_id: i64,
    track_id: i64,
) -> Result<()> {
    state
        .remove_track_from_playlist(PlaylistId(playlist_id), TrackId(track_id))
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn add_to_queue(state: State<'_, Arc<AppState>>, track_id: i64) -> Result<usize> {
    let index = state
        .add_to_queue(TrackId(track_id))
        .await
        .map_err(|e| e.to_string())?;

    Ok(index)
}

#[tauri::command]
pub fn remove_from_queue(state: State<'_, Arc<AppState>>, index: usize) -> Result<bool> {
    let removed = state.remove_from_queue(index);
    Ok(removed.is_some())
}

#[tauri::command]
pub fn clear_queue(state: State<'_, Arc<AppState>>) -> Result<()> {
    state.clear_queue();
    Ok(())
}

#[tauri::command]
pub fn get_queue(state: State<'_, Arc<AppState>>) -> Result<Vec<TrackDto>> {
    let queue_tracks = state.get_queue();
    Ok(queue_tracks.into_iter().map(|t| t.into()).collect())
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
pub fn play_at_index(state: State<'_, Arc<AppState>>, index: usize) -> Result<()> {
    state.play_at_index(index);
    Ok(())
}

#[tauri::command]
pub fn play_next(state: State<'_, Arc<AppState>>) -> Result<()> {
    state.play_next();
    Ok(())
}

#[tauri::command]
pub fn play_previous(state: State<'_, Arc<AppState>>) -> Result<()> {
    state.play_previous();
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
pub fn set_repeat_mode(state: State<'_, Arc<AppState>>, mode: String) -> Result<()> {
    use nexus_player_backend::playlist::playback_queue::RepeatMode;

    let repeat_mode = match mode.as_str() {
        "off" => RepeatMode::Off,
        "all" => RepeatMode::All,
        "one" => RepeatMode::One,
        _ => return Err("Invalid repeat mode".into()),
    };

    state.set_repeat_mode(repeat_mode);
    Ok(())
}

#[tauri::command]
pub fn toggle_shuffle(state: State<'_, Arc<AppState>>) -> Result<()> {
    state.toggle_shuffle();
    Ok(())
}

#[tauri::command]
pub fn get_queue_state(state: State<'_, Arc<AppState>>) -> Result<QueueStateDto> {
    let queue = state.queue();
    let tracks: Vec<TrackDto> = queue.tracks().into_iter().map(|t| t.into()).collect();

    let repeat_mode = match state.get_repeat_mode() {
        nexus_player_backend::playlist::playback_queue::RepeatMode::Off => "off",
        nexus_player_backend::playlist::playback_queue::RepeatMode::All => "all",
        nexus_player_backend::playlist::playback_queue::RepeatMode::One => "one",
    };

    Ok(QueueStateDto {
        tracks,
        current_index: state.get_current_track_index(),
        repeat_mode: repeat_mode.to_string(),
        shuffle_enabled: state.is_shuffle_enabled(),
    })
}

#[tauri::command]
pub async fn select_audio_file(app: tauri::AppHandle) -> Result<Option<String>> {
    let file_path = app
        .dialog()
        .file()
        .add_filter("Audio Files", &["mp3", "wav", "flac", "ogg", "m4a", "aac"])
        .blocking_pick_file();

    Ok(file_path.map(|path| path.to_string()))
}

#[tauri::command]
pub async fn select_audio_files(app: tauri::AppHandle) -> Result<Vec<String>> {
    let file_paths = app
        .dialog()
        .file()
        .add_filter("Audio Files", &["mp3", "wav", "flac", "ogg", "m4a", "aac"])
        .blocking_pick_files();

    Ok(file_paths
        .unwrap_or_default()
        .into_iter()
        .map(|path| path.to_string())
        .collect())
}

#[tauri::command]
pub async fn search_streaming(
    state: State<'_, Arc<AppState>>,
    provider: String,
    content_type: Option<String>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<SearchResultDto>> {
    let content_type_enum = content_type.and_then(|ct| match ct.as_str() {
        "track" => Some(ContentType::Track),
        "playlist" => Some(ContentType::Playlist),
        "album" => Some(ContentType::Album),
        "channel" => Some(ContentType::Channel),
        _ => None,
    });

    let results = state
        .streaming()
        .search(&provider, content_type_enum, &query, limit.unwrap_or(10))
        .await
        .map_err(|e| e.to_string())?;

    Ok(results.into_iter().map(|r| r.into()).collect())
}
