use crate::dto::{
    AudioStateDto, ContainerMetaDto, CreatePlaylistRequest, CreatePlaylistWithTracksRequest,
    CurrentPositionDto, PlaylistDto, PlaylistWithTracksDto, QueueEntryDto, QueueStateDto,
    SearchContextDto, SearchResultDto, TrackDto,
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
use tauri::{AppHandle, Emitter, Manager, State};
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
            Ok(tracks.into_iter().map(|t| TrackDto::from(t)).collect())
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
                    nexus_player_backend::playlist::playback_queue::QueueEntry::Container { tracks: Some(ts), .. } => {
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
pub async fn get_queue_state(state: State<'_, Arc<AppState>>) -> Result<QueueStateDto> {
    use nexus_player_backend::playlist::playback_queue::{ActiveQueue, ContainerKind, QueueEntry};

    let queue = state.lock_queue().await;
    build_queue_state_dto(&queue)
}

fn build_queue_state_dto(
    queue: &nexus_player_backend::playlist::playback_queue::ActiveQueue,
) -> Result<QueueStateDto> {
    use nexus_player_backend::playlist::playback_queue::{ActiveQueue, ContainerKind, QueueEntry};

    match queue {
        ActiveQueue::Cursor(cq) => {
            let (tracks, current_index) = cq.snapshot();
            let entries: Vec<QueueEntryDto> = tracks
                .into_iter()
                .map(|t| QueueEntryDto::Track { track: t.into() })
                .collect();

            let current_position = current_index.map(|idx| CurrentPositionDto {
                item_index: idx,
                track_within_container: None,
            });

            let repeat_mode = match cq.repeat_mode {
                nexus_player_backend::playlist::playback_queue::RepeatMode::Off => "off",
                nexus_player_backend::playlist::playback_queue::RepeatMode::All => "all",
                nexus_player_backend::playlist::playback_queue::RepeatMode::One => "one",
            };

            Ok(QueueStateDto {
                entries,
                current_position,
                repeat_mode: repeat_mode.to_string(),
                shuffle_enabled: cq.shuffle_enabled,
                search_context: None,
            })
        }
        ActiveQueue::Search(sq) => {
            let (entries, position) = sq.snapshot();

            let entries_dto: Vec<QueueEntryDto> = entries
                .into_iter()
                .map(|entry| match entry {
                    QueueEntry::Track(track) => QueueEntryDto::Track { track: track.into() },
                    QueueEntry::Container { info, tracks, play_index } => {
                        let kind_str = match info.kind {
                            ContainerKind::Album => "album",
                            ContainerKind::Playlist => "playlist",
                        };
                        QueueEntryDto::Container {
                            meta: ContainerMetaDto {
                                kind: kind_str.to_string(),
                                id: info.id,
                                system: info.system,
                                title: info.title,
                                artist: info.artist,
                                track_count: info.track_count,
                                thumbnail: info.thumbnail,
                            },
                            tracks: tracks.map(|ts| ts.into_iter().map(|t| t.into()).collect()),
                            play_index,
                        }
                    }
                })
                .collect();

            let current_position = position.map(|p| CurrentPositionDto {
                item_index: p.item_index,
                track_within_container: p.track_within_container,
            });

            Ok(QueueStateDto {
                entries: entries_dto,
                current_position,
                repeat_mode: "off".to_string(),
                shuffle_enabled: false,
                search_context: Some(SearchContextDto {
                    system: sq.system.clone(),
                    query: sq.query.clone(),
                }),
            })
        }
    }
}

#[tauri::command]
pub async fn handle_search(
    state: State<'_, Arc<AppState>>,
    system: String,
    query: String,
    selected_index: usize,
) -> Result<()> {
    state
        .load_search_queue(system, query, selected_index)
        .await
        .map_err(|e| e.to_string())?;

    state.play_current_search().await;
    Ok(())
}

#[tauri::command]
pub async fn expand_queue_container(
    state: State<'_, Arc<AppState>>,
    item_index: usize,
) -> Result<QueueStateDto> {
    state
        .expand_search_container(item_index)
        .await
        .map_err(|e| e.to_string())?;

    let queue = state.lock_queue().await;
    build_queue_state_dto(&queue)
}

#[tauri::command]
pub async fn play_queue_item(
    state: State<'_, Arc<AppState>>,
    item_index: usize,
    track_index_in_container: Option<usize>,
) -> Result<()> {
    state.play_queue_item(item_index, track_index_in_container).await;
    Ok(())
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
        .search(&provider, content_type_enum, &query, limit.unwrap_or(10), 0)
        .await
        .map_err(|e| e.to_string())?;

    Ok(results.into_iter().map(|r| r.into()).collect())
}

#[tauri::command]
pub async fn handle_streaming_track(
    state: State<'_, Arc<AppState>>,
    search_result: SearchResultDto,
    strategy: String,
    query: Option<String>,
) -> Result<Option<i64>> {
    use nexus_player_backend::library::types::DownloadStrategy;

    let download_strategy = match strategy.as_str() {
        "stream" => DownloadStrategy::Stream,
        "cache" => DownloadStrategy::Cache,
        "progressive" => DownloadStrategy::Progressive,
        _ => return Err("Invalid download strategy".into()),
    };
    tracing::info!("SEARCH RESULT {:?}",search_result);

    match download_strategy {
        DownloadStrategy::Cache => {
            let backend_search_result: nexus_player_backend::streaming::types::SearchResult =
                search_result.into();

            match backend_search_result {
                SearchResult::Track(track) => {
                    if let Some(q) = query {
                        let audio_track: nexus_player_backend::audio::types::Track = track.into();
                        let target_id = audio_track.id;
                        let system = audio_track.source.system().to_string();

                        state.load_search_to_queue(system, q, target_id).await.map_err(|e| e.to_string())?;
                    } else {
                        use nexus_player_backend::playlist::playback_queue::{ActiveQueue, CursorQueue};
                        let mut queue = state.lock_queue().await;
                        match &mut *queue {
                            ActiveQueue::Cursor(cq) => {
                                cq.add_track(track.into());
                            }
                            _ => {
                                let mut cq = CursorQueue::new();
                                cq.add_track(track.into());
                                *queue = ActiveQueue::Cursor(cq);
                            }
                        }
                        tracing::info!("Track added to queue");
                    }
                }
                SearchResult::Playlist(playlist) => {
                    state.load_context_to_queue(ContainerContext::ExternalPlaylist { 
                        system: playlist.service.to_string(), 
                        id: playlist.id 
                    }).await.map_err(|e| e.to_string())?;
                }
                SearchResult::Album(album) => {
                     state.load_context_to_queue(ContainerContext::ExternalAlbum { 
                        system: album.service.to_string(), 
                        id: album.id 
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
pub async fn open_vk_login(app: AppHandle) -> Result<()> {
    let url = vk_down::get_default_auth_url();
    let app_handle = app.clone();

    let _login_window = tauri::WebviewWindowBuilder::new(
        &app,
        "vk-login",
        tauri::WebviewUrl::External(url.parse().expect("Failed to parse VK URL")),
    )
    .title("VK Авторизация")
    .inner_size(500.0, 700.0)
    .always_on_top(true)
    .on_navigation(move |url| {
        let url_str = url.as_str();
        if url_str.contains("access_token=") {
            let token = vk_down::extract_token_from_url(url_str);
            let _ = app_handle.emit("vk-token-received", token);
            if let Some(w) = app_handle.get_webview_window("vk-login") {
                let _ = w.close();
            }
            return false;
        }
        true
    })
    .build()
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn get_vk_auth_url() -> String {
    vk_down::auth::get_oauth_url(&vk_down::auth::AuthConfig::default())
}

#[tauri::command]
pub fn get_vk_auth_html() -> String {
    vk_down::get_auth_html()
}

#[tauri::command]
pub fn extract_vk_token(url_or_token: String) -> String {
    vk_down::extract_token_from_url(&url_or_token)
}

#[tauri::command]
pub fn get_app_config(state: State<'_, Arc<AppState>>) -> Result<AppConfig> {
    Ok(state.config())
}

#[tauri::command]
pub async fn update_app_config(state: State<'_, Arc<AppState>>, config: AppConfig) -> Result<()> {
    state.update_config(config).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_vk_profile(
    state: State<'_, Arc<AppState>>,
) -> Result<(Vec<TrackDto>, Vec<PlaylistDto>)> {
    tracing::info!("get_vk_profile command called");
    let vk_provider = state
        .streaming()
        .get_provider("vk")
        .ok_or_else(|| {
            tracing::error!("VK provider not found in streaming service");
            "VK provider not found".to_string()
        })?;

    let (tracks, playlists) = vk_provider.get_profile_content().await.map_err(|e| {
        tracing::error!("Failed to get VK profile content from provider: {}", e);
        e.to_string()
    })?;

    tracing::info!("Successfully retrieved VK profile data. Converting {} tracks and {} playlists to DTOs...", tracks.len(), playlists.len());

    let tracks_dto = tracks.into_iter().map(TrackDto::from).collect();
    let playlists_dto = playlists.into_iter().map(PlaylistDto::from).collect();

    tracing::info!("Conversion complete.");

    Ok((tracks_dto, playlists_dto))
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
