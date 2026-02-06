use crate::dto::{
    AudioStateDto, CreatePlaylistRequest, CreatePlaylistWithTracksRequest, PlaylistDto,
    PlaylistWithTracksDto, QueueStateDto, SearchResultDto, TrackDto,
};
use nexus_player_backend::library::models::{Metadata, Track as LibraryTrack};
use nexus_player_backend::library::types::{
    AudioInfo, CacheStatus, CodecFormat, PlaylistId, TrackId, TrackSource,
};
use nexus_player_backend::state::state::AppState;
use nexus_player_backend::streaming::types::{ContentType, ResolvedContent, SearchResult};
use nexus_player_backend::playlist::cursor::ContainerContext;
use std::path::{Path, PathBuf};
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
pub async fn create_playlist_with_tracks(
    state: State<'_, Arc<AppState>>,
    request: CreatePlaylistWithTracksRequest,
) -> Result<i64> {
    let audio_tracks: Vec<_> = request.tracks.iter().map(|t| t.to_audio_track()).collect();

    let playlist_id = state
        .create_playlist_with_tracks(request.title, request.description, audio_tracks)
        .await
        .map_err(|e| e.to_string())?;

    Ok(playlist_id.0)
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
    let removed = state.remove_from_queue(index);
    Ok(removed.is_some())
}

#[tauri::command]
pub async fn clear_queue(state: State<'_, Arc<AppState>>) -> Result<()> {
    state.clear_queue().await;
    Ok(())
}

#[tauri::command]
pub async fn get_queue(state: State<'_, Arc<AppState>>) -> Result<Vec<TrackDto>> {
    let queue = state.lock_queue().await;
    let (tracks, _) = queue.snapshot();
    Ok(tracks.into_iter().map(|t| t.into()).collect())
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

    state.set_repeat_mode(repeat_mode);
    Ok(())
}

#[tauri::command]
pub fn toggle_shuffle(state: State<'_, Arc<AppState>>) -> Result<()> {
    state.toggle_shuffle();
    Ok(())
}

#[tauri::command]
pub async fn get_queue_state(state: State<'_, Arc<AppState>>) -> Result<QueueStateDto> {
    let queue = state.lock_queue().await;
    let (tracks, current_index) = queue.snapshot();
    let tracks_dto: Vec<TrackDto> = tracks.into_iter().map(|t| t.into()).collect();

    // Re-implement getting repeat/shuffle mode if needed, or define default
    // Since we disabled them in state proxy, let's assume default for now
    let repeat_mode = "off"; 
    let shuffle_enabled = false;

    let q = QueueStateDto {
        tracks: tracks_dto,
        current_index,
        repeat_mode: repeat_mode.to_string(),
        shuffle_enabled,
    };

    Ok(q)
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

#[tauri::command]
pub async fn handle_streaming_track(
    state: State<'_, Arc<AppState>>,
    search_result: SearchResultDto,
    strategy: String,
) -> Result<Option<i64>> {
    use nexus_player_backend::library::types::DownloadStrategy;

    let download_strategy = match strategy.as_str() {
        "stream" => DownloadStrategy::Stream,
        "cache" => DownloadStrategy::Cache,
        "progressive" => DownloadStrategy::Progressive,
        _ => return Err("Invalid download strategy".into()),
    };

    match download_strategy {
        DownloadStrategy::Cache => {
            let backend_search_result: nexus_player_backend::streaming::types::SearchResult =
                search_result.into();

            match backend_search_result {
                SearchResult::Track(track) => {
                    let mut queue = state.lock_queue().await;
                    queue.add_track(track.into());
                    tracing::info!("Track added to queue");
                }
                SearchResult::Playlist(playlist) => {
                    state.load_context_to_queue(ContainerContext::ExternalPlaylist { 
                        system: playlist.service.to_string(), 
                        id: playlist.url 
                    }).await.map_err(|e| e.to_string())?;
                }
                SearchResult::Album(album) => {
                     state.load_context_to_queue(ContainerContext::ExternalAlbum { 
                        system: album.service.to_string(), 
                        id: album.url 
                    }).await.map_err(|e| e.to_string())?;
                }
                _ => todo!(),
            }
            Ok(None)
        }
        DownloadStrategy::Stream => Err("Stream strategy not yet implemented".into()),
        DownloadStrategy::Progressive => Err("Progressive strategy not yet implemented".into()),
    }
}

use nexus_player_backend::config::AppConfig;

#[tauri::command]
pub fn get_app_config(state: State<'_, Arc<AppState>>) -> Result<AppConfig> {
    Ok(state.config())
}

#[tauri::command]
pub fn update_app_config(state: State<'_, Arc<AppState>>, config: AppConfig) -> Result<()> {
    state.update_config(config).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_track_to_file(
    state: State<'_, Arc<AppState>>,
    track_id: i64,
) -> Result<()> {
    let downloads_dir = Path::new("/home/syndafloden/.cache/downloads");
    std::fs::create_dir_all(downloads_dir).map_err(|e| e.to_string())?;

    let file_path = state
        .streaming()
        .save_track_to_file(track_id as u64, downloads_dir)
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!("Track saved to {:?}", file_path);
    Ok(())
}