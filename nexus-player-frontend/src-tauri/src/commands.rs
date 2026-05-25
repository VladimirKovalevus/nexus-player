use nexus_player_backend::sync::{
    SyncApplier, SyncBuilder, SyncStore, SyncType, TelegramConfig, TelegramSync,
};

use crate::dto::{
    hash_str_to_i64, AudioStateDto, CachedAlbumDto, CleanupResultDto, ContainerMetaDto,
    CreatePlaylistRequest, CreatePlaylistWithTracksRequest, CurrentPositionDto, EqPresetDto,
    EqSettingsDto, LibraryPageDto, PlayHistoryEntryDto, PlaylistDto, PlaylistItemDto,
    PlaylistWithItemsDto, PlaylistWithTracksDto, QueueEntryDto, QueueStateDto, ResolvedArtworkDto,
    SearchContextDto, SearchResultDto, TaskInfoDto, TrackDto, TrackSourceDto,
};
use crate::events::FrontendEvent;
use nexus_player_backend::library::local_library::{AndroidMediaEntry, MusicSource, ScanResult};
use nexus_player_backend::library::models::{Metadata, Track as LibraryTrack};
use nexus_player_backend::library::types::{
    AudioInfo, CacheStatus, CodecFormat, PlaylistId, TrackId, TrackOrder, TrackSource,
};
use nexus_player_backend::playlist::cursor::ContainerContext;
use nexus_player_backend::state::state::AppState;
use nexus_player_backend::streaming::types::{ContentType, ResolvedContent, SearchResult};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use streaming::images::ImageVariant;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

type Result<T> = std::result::Result<T, String>;

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
    Err("Дополнительные папки на Android управляются через MediaStore".into())
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

#[tauri::command]
pub async fn get_all_playlists(state: State<'_, Arc<AppState>>) -> Result<Vec<PlaylistDto>> {
    let playlists = state.get_all_playlists().await.map_err(|e| e.to_string())?;

    Ok(playlists.into_iter().map(|p| p.into()).collect())
}

#[tauri::command]
pub async fn get_playlist_with_tracks(
    state: State<'_, Arc<AppState>>,
    playlist_id: i64,
) -> Result<Option<PlaylistWithItemsDto>> {
    let pwi = state
        .get_playlist_with_items(PlaylistId(playlist_id))
        .await
        .map_err(|e| e.to_string())?;

    let Some(pwi) = pwi else {
        return Ok(None);
    };

    let mut items = Vec::new();
    for item in &pwi.items {
        match item.item_kind {
            nexus_player_backend::library::models::PlaylistItemKind::Track => {
                if let Some(track) = state
                    .get_library_track(TrackId(item.item_id))
                    .await
                    .map_err(|e| e.to_string())?
                {
                    items.push(PlaylistItemDto::Track {
                        id: item.id,
                        item_id: item.item_id,
                        track: track.into(),
                        position: item.position,
                    });
                }
            }
            nexus_player_backend::library::models::PlaylistItemKind::Playlist => {
                if let Some(playlist) = state
                    .get_playlist(PlaylistId(item.item_id))
                    .await
                    .map_err(|e| e.to_string())?
                {
                    items.push(PlaylistItemDto::Playlist {
                        id: item.id,
                        item_id: item.item_id,
                        playlist: playlist.into(),
                        position: item.position,
                    });
                }
            }
        }
    }

    let mut playlist_dto: PlaylistDto = pwi.playlist.into();
    playlist_dto.track_count = items.len();

    Ok(Some(PlaylistWithItemsDto {
        playlist: playlist_dto,
        items,
    }))
}

#[tauri::command]
pub async fn add_playlist_to_playlist(
    state: State<'_, Arc<AppState>>,
    parent_playlist_id: i64,
    child_playlist_id: i64,
) -> Result<()> {
    state
        .add_playlist_to_playlist(
            PlaylistId(parent_playlist_id),
            PlaylistId(child_playlist_id),
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn remove_playlist_from_playlist(
    state: State<'_, Arc<AppState>>,
    parent_playlist_id: i64,
    child_playlist_id: i64,
) -> Result<()> {
    state
        .remove_playlist_from_playlist(
            PlaylistId(parent_playlist_id),
            PlaylistId(child_playlist_id),
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn create_playlist(
    state: State<'_, Arc<AppState>>,
    request: CreatePlaylistRequest,
) -> Result<i64> {
    let playlist_id = state
        .create_playlist(
            request.title,
            request.description,
            request.creator,
            request.artwork_url,
        )
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
        .and_then(ImageVariant::from_str)
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

#[tauri::command]
pub async fn create_playlist_from_container(
    state: State<'_, Arc<AppState>>,
    provider: String,
    content_type: ContentType,
    id: String,
    title: String,
    description: Option<String>,
    creator: Option<String>,
    artwork_url: Option<String>,
) -> Result<i64> {
    state
        .create_playlist_from_container(provider, content_type, id, title, description, artwork_url)
        .await
        .map(|id| id.0)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_playlist_with_tracks(
    state: State<'_, Arc<AppState>>,
    request: CreatePlaylistWithTracksRequest,
) -> Result<i64> {
    let audio_tracks: Vec<_> = request
        .tracks
        .iter()
        .map(|t| t.try_to_audio_track())
        .collect::<std::result::Result<_, _>>()?;

    let playlist_id = state
        .create_playlist_with_tracks(
            request.title,
            request.description,
            request.artwork_url,
            audio_tracks,
        )
        .await
        .map_err(|e| e.to_string())?;

    Ok(playlist_id.0)
}

#[tauri::command]
pub async fn add_track_to_playlist(
    state: State<'_, Arc<AppState>>,
    playlist_id: i64,
    track: TrackDto,
) -> Result<()> {
    state
        .add_track_to_playlist(PlaylistId(playlist_id), track.try_to_audio_track()?)
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

#[tauri::command]
pub async fn get_queue_state(state: State<'_, Arc<AppState>>) -> Result<QueueStateDto> {
    use nexus_player_backend::playlist::playback_queue::{ActiveQueue, ContainerKind, QueueEntry};

    let queue = state.lock_queue().await;
    build_queue_state_dto(&queue)
}

/// Reads the active radio mode out of the queue context. Called on the 100ms event loop
/// too, so it only borrows — never clones the context (which carries the whole order).
pub(crate) fn radio_mode_label(
    cq: &nexus_player_backend::playlist::playback_queue::CursorQueue,
) -> Option<String> {
    match &cq.context {
        Some(ContainerContext::Radio { mode, .. }) => Some(
            match mode {
                nexus_player_backend::playlist::cursor::RadioMode::Tracks => "tracks",
                nexus_player_backend::playlist::cursor::RadioMode::Playlists => "playlists",
            }
            .to_string(),
        ),
        _ => None,
    }
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
                radio_mode: radio_mode_label(cq),
            })
        }
        ActiveQueue::Search(sq) => {
            let (entries, position) = sq.snapshot();

            let entries_dto: Vec<QueueEntryDto> = entries
                .into_iter()
                .map(|entry| match entry {
                    QueueEntry::Track(track) => QueueEntryDto::Track {
                        track: track.into(),
                    },
                    QueueEntry::Container {
                        info,
                        tracks,
                        play_index,
                    } => {
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

            let repeat_mode = match sq.repeat_mode {
                nexus_player_backend::playlist::playback_queue::RepeatMode::Off => "off",
                nexus_player_backend::playlist::playback_queue::RepeatMode::All => "all",
                nexus_player_backend::playlist::playback_queue::RepeatMode::One => "one",
            };

            Ok(QueueStateDto {
                entries: entries_dto,
                current_position,
                repeat_mode: repeat_mode.to_string(),
                shuffle_enabled: sq.shuffle_enabled,
                search_context: Some(SearchContextDto {
                    system: sq.system.clone(),
                    query: sq.query.clone(),
                }),
                radio_mode: None,
            })
        }
    }
}

#[tauri::command]
pub async fn handle_search(
    state: State<'_, Arc<AppState>>,
    system: String,
    content_type: Option<String>,
    query: String,
    limit: Option<usize>,
    offset: Option<usize>,
    selected_index: usize,
) -> Result<()> {
    use nexus_player_backend::streaming::types::ContentType;

    let content_type_enum = content_type.and_then(|ct| match ct.as_str() {
        "track" => Some(ContentType::Track),
        "playlist" => Some(ContentType::Playlist),
        "album" => Some(ContentType::Album),
        "channel" => Some(ContentType::Channel),
        _ => None,
    });

    state
        .search_and_play(
            system,
            query,
            content_type_enum,
            limit.unwrap_or(50),
            offset.unwrap_or(0),
            selected_index,
        )
        .await
        .map_err(|e| e.to_string())?;

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
    state
        .play_queue_item(item_index, track_index_in_container)
        .await;
    Ok(())
}

#[tauri::command]
pub async fn select_sync_file(app: tauri::AppHandle) -> Result<Option<String>> {
    let file_path = app
        .dialog()
        .file()
        .add_filter("Sync File", &["json"])
        .blocking_pick_file();

    Ok(file_path.map(|path| path.to_string()))
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
pub async fn select_image_file(app: tauri::AppHandle) -> Result<Option<String>> {
    let file_path = app
        .dialog()
        .file()
        .add_filter("Images", &["jpg", "jpeg", "png", "webp", "gif", "bmp"])
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
    offset: Option<usize>,
) -> Result<Vec<SearchResultDto>> {
    let content_type_enum = content_type.and_then(|ct| match ct.as_str() {
        "track" => Some(ContentType::Track),
        "playlist" => Some(ContentType::Playlist),
        "album" => Some(ContentType::Album),
        "channel" => Some(ContentType::Channel),
        _ => None,
    });

    let results = state
        .search_streaming_cancellable(
            &provider,
            content_type_enum,
            &query,
            limit.unwrap_or(10),
            offset.unwrap_or(0),
        )
        .await
        .map_err(|e| e.to_string())?;

    let mut dtos: Vec<SearchResultDto> = Vec::new();
    let mut tracks_to_enrich: Vec<nexus_player_backend::audio::types::Track> = Vec::new();
    let mut track_indices = Vec::new();

    for (i, r) in results.into_iter().enumerate() {
        let dto = SearchResultDto::from(r);
        if let SearchResultDto::Track(ref t) = dto {
            tracks_to_enrich.push(t.to_audio_track());
            track_indices.push(i);
        }
        dtos.push(dto);
    }

    if !tracks_to_enrich.is_empty() {
        let enriched = state
            .enrich_tracks(tracks_to_enrich)
            .await
            .map_err(|e| e.to_string())?;
        for (i, track) in track_indices.into_iter().zip(enriched) {
            if let SearchResultDto::Track(ref mut t) = dtos[i] {
                t.liked = track.liked;
            }
        }
    }

    Ok(dtos)
}

#[tauri::command]
pub async fn handle_streaming_track(
    state: State<'_, Arc<AppState>>,
    search_result: SearchResultDto,
    strategy: String,
    query: Option<String>,
    search_type: Option<String>,
    play: Option<bool>,
) -> Result<Option<i64>> {
    use nexus_player_backend::library::types::DownloadStrategy;
    use nexus_player_backend::streaming::types::ContentType;

    let download_strategy = match strategy.as_str() {
        "stream" => DownloadStrategy::Stream,
        "cache" => DownloadStrategy::Cache,
        "progressive" => DownloadStrategy::Progressive,
        _ => return Err("Invalid download strategy".into()),
    };
    let should_play = play.unwrap_or(false);
    tracing::info!("SEARCH RESULT {:?}", search_result);

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

                        let content_type = match search_type.as_deref() {
                            Some("track") => ContentType::Track,
                            Some("playlist") => ContentType::Playlist,
                            Some("album") => ContentType::Album,
                            Some("channel") => ContentType::Channel,
                            _ => ContentType::Track,
                        };

                        if should_play {
                            state
                                .load_search_and_play(system, q, content_type, target_id)
                                .await
                                .map_err(|e| e.to_string())?;
                        } else {
                            state
                                .load_search_to_queue(system, q, content_type, target_id)
                                .await
                                .map_err(|e| e.to_string())?;
                        }
                    } else {
                        let audio_track: nexus_player_backend::audio::types::Track = track.into();
                        if should_play {
                            state
                                .add_track_and_play(audio_track)
                                .await
                                .map_err(|e| e.to_string())?;
                        } else {
                            use nexus_player_backend::playlist::playback_queue::{
                                ActiveQueue, CursorQueue,
                            };
                            let mut queue = state.lock_queue().await;
                            match &mut *queue {
                                ActiveQueue::Cursor(cq) => {
                                    cq.add_track(audio_track);
                                }
                                _ => {
                                    let mut cq = CursorQueue::new();
                                    cq.add_track(audio_track);
                                    *queue = ActiveQueue::Cursor(cq);
                                }
                            }
                            tracing::info!("Track added to queue");
                        }
                    }
                }
                SearchResult::Playlist(playlist) => {
                    let context = ContainerContext::ExternalPlaylist {
                        system: playlist.service.to_string(),
                        id: playlist.id,
                    };
                    if should_play {
                        state
                            .load_context_and_play(context, 0)
                            .await
                            .map_err(|e| e.to_string())?;
                    } else {
                        state
                            .load_context_to_queue(context)
                            .await
                            .map_err(|e| e.to_string())?;
                    }
                }
                SearchResult::Album(album) => {
                    let context = ContainerContext::ExternalAlbum {
                        system: album.service.to_string(),
                        id: album.id,
                    };
                    if should_play {
                        state
                            .load_context_and_play(context, 0)
                            .await
                            .map_err(|e| e.to_string())?;
                    } else {
                        state
                            .load_context_to_queue(context)
                            .await
                            .map_err(|e| e.to_string())?;
                    }
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
pub async fn update_app_config(state: State<'_, Arc<AppState>>, config: AppConfig) -> Result<()> {
    state.update_config(config).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_provider_auth(
    state: State<'_, Arc<AppState>>,
    provider: String,
    auth_data: String,
) -> Result<()> {
    tracing::info!(
        "Updating auth for provider {}: (data len={})",
        provider,
        auth_data.len()
    );

    // 1. Update the active provider in memory
    state
        .streaming()
        .update_provider_auth(&provider, auth_data.clone())
        .await;

    // 2. Persist to configuration file
    let mut config = state.config();
    match provider.as_str() {
        "soundcloud" => {
            config.providers.soundcloud.credentials = Some(auth_data);
        }
        "bandcamp" => {
            config.providers.bandcamp.credentials = Some(auth_data);
        }
        _ => {
            tracing::warn!(
                "Unknown provider: {}, skipping config persistence",
                provider
            );
        }
    }

    state.update_config(config).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn submit_challenge_result(
    app: tauri::AppHandle,
    client_id: String,
    label: String,
) -> Result<()> {
    tracing::info!("Challenge result submitted from {}: {}", label, client_id);
    // Emit a global event from the backend which is more reliable
    app.emit(
        "challenge-solved-global",
        serde_json::json!({
            "clientId": client_id,
            "label": label
        }),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn open_challenge_window(
    app: tauri::AppHandle,
    label: String,
    url: String,
    script: String,
) -> Result<()> {
    tracing::info!("Opening challenge window: {} for {}", label, url);

    #[cfg(desktop)]
    {
        if let Some(win) = tauri::Manager::get_webview_window(&app, &label) {
            let _ = win.close();
        }

        tauri::WebviewWindowBuilder::new(
            &app,
            &label,
            tauri::WebviewUrl::External(url.parse().unwrap()),
        )
        .initialization_script(&script)
        .visible(false)
        .build()
        .map_err(|e: tauri::Error| e.to_string())?;
    }

    // On Android the native SoundCloudChallengePlugin handles this:
    // it opens a Dialog with a WebView, injects the XHR/fetch interception script,
    // and calls submit_challenge_result via JavascriptInterface when client_id is found.
    // The Kotlin plugin fires the resolve() callback which the frontend awaits.
    // Nothing to do here on mobile — the frontend calls the Kotlin command directly.

    Ok(())
}

#[tauri::command]
pub async fn retry_playback(state: State<'_, Arc<AppState>>) -> Result<()> {
    state.play_current().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_track_to_file(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    track: TrackDto,
) -> Result<()> {
    let downloads_dir = app
        .path()
        .cache_dir()
        .map_err(|e| e.to_string())?
        .join("nexus-downloads");
    std::fs::create_dir_all(&downloads_dir).map_err(|e| e.to_string())?;

    let track_id = if track.id == 0 {
        let src = track.source.as_ref().ok_or("Track has no source")?;
        hash_str_to_i64(&src.url)
    } else {
        track.id
    };

    if let Ok(Some(lib_track)) = state.library().get_track(TrackId(track_id)).await {
        if let CacheStatus::Cached { path, .. } = &lib_track.cache {
            if path.exists() {
                tracing::info!("Track {} already cached, skipping download", track_id);
                return Ok(());
            }
        }
    }

    let src = track.source.as_ref().ok_or("Track has no source")?;
    let track_title = track.title.clone();
    let state_arc = state.inner().clone();
    let has_track = state.streaming().has_track(track_id as u64).await;
    let options = streaming::state::types::DownloadOptions {
        user_agent: None,
        meta: None,
        on_start: Some(Arc::new({
            let app_handle = app.clone();
            let track_title = track_title.clone();
            move |_, _| {
                let _ = app_handle.emit(
                    "audio-event",
                    FrontendEvent::DownloadStarted {
                        title: track_title.clone(),
                    },
                );
                let _ = app_handle.emit("download-start", (track_id, track_title.clone()));
            }
        }) as Arc<streaming::state::types::TaskCallback>),
        on_progress: Some(Arc::new({
            let app_handle = app.clone();
            move |res: std::result::Result<
                streaming::state::types::BufferType,
                streaming::error::StreamingError,
            >,
                  _| {
                if let Ok(buffer) = res {
                    let progress = buffer.progress();
                    let byte_size = buffer.byte_size();
                    let _ = app_handle.emit(
                        "audio-event",
                        FrontendEvent::DownloadProgress {
                            track_id,
                            progress: progress as f32,
                        },
                    );
                    let _ = app_handle.emit("download-progress", (track_id, progress, byte_size));
                }
            }
        }) as Arc<streaming::state::types::TaskCallback>),
        on_finish: Some(Arc::new({
            let app_handle = app.clone();
            let state_arc = state_arc.clone();
            let track_title = track_title.clone();
            move |res: std::result::Result<
                streaming::state::types::BufferType,
                streaming::error::StreamingError,
            >,
                  _| {
                let success = res.is_ok();
                if let Ok(buffer) = res {
                    let file_path = downloads_dir.join(format!(
                        "{} - {}.{}",
                        track.artist.clone().unwrap_or("Unknown".to_string()),
                        track.title,
                        buffer.container()
                    ));
                    let _ = buffer.save_to_file(file_path.clone());

                    // Update database in background
                    let file_path_clone = file_path;
                    let state_arc_clone = state_arc.clone();
                    tauri::async_runtime::spawn(async move {
                        use nexus_player_backend::library::types::CacheStatus;
                        if let Ok(Some(mut lib_track)) = state_arc_clone
                            .library()
                            .get_track(nexus_player_backend::library::TrackId(track_id))
                            .await
                        {
                            lib_track.cache = CacheStatus::Cached {
                                path: file_path_clone,
                                downloaded_at: chrono::Utc::now(),
                            };
                            let _ = state_arc_clone.library().update_track(lib_track).await;
                        }
                    });
                }
                let _ = app_handle.emit(
                    "audio-event",
                    FrontendEvent::DownloadFinished {
                        track_id,
                        title: track_title.clone(),
                        success,
                    },
                );
                let _ =
                    app_handle.emit("download-finish", (track_id, success, track_title.clone()));
            }
        }) as Arc<streaming::state::types::TaskCallback>),
    };

    if has_track {
        state
            .streaming()
            .update_task(track_id as u64, options)
            .await;
        tracing::info!("Track saved");
    } else {
        tracing::info!(
            "POEBOTA for {:?}",
            TrackSource::External {
                system: src.system.clone(),
                id: src.url.clone(),
            }
        );
        state
            .streaming()
            .add_download_task(
                TrackSource::External {
                    system: src.system.clone(),
                    id: src.url.clone(),
                },
                track_id as u64,
                Some(options),
            )
            .await
            .map_err(|e| e.to_string())?;

        tracing::info!("Track download initiated for {:?}", track_id);
    }

    Ok(())
}

#[tauri::command]
pub async fn delete_cached_track(state: State<'_, Arc<AppState>>, track_id: i64) -> Result<()> {
    let mut lib_track = state
        .library()
        .get_track(TrackId(track_id))
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Track not found in library".to_string())?;

    if let CacheStatus::Cached { path, .. } = &lib_track.cache {
        if let Err(e) = std::fs::remove_file(path) {
            if e.kind() != std::io::ErrorKind::NotFound {
                return Err(e.to_string());
            }
        }
    }

    lib_track.cache = CacheStatus::None;
    state
        .library()
        .update_track(lib_track)
        .await
        .map_err(|e| e.to_string())?;

    tracing::info!("Deleted cached file for track {}", track_id);
    Ok(())
}

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

#[tauri::command]
pub async fn export_sync(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    sync_type: SyncType,
    device_name: Option<String>,
) -> Result<String> {
    let sync_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("sync");

    let builder = SyncBuilder::new(state.library());
    let file = match sync_type {
        SyncType::Full => builder.build_full(device_name).await,
        SyncType::Diff => builder.build_diff(device_name).await,
    }
    .map_err(|e| e.to_string())?;

    let store = SyncStore::new(sync_dir);
    let path = store.save(&file).map_err(|e| e.to_string())?;

    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn import_sync(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    path: String,
) -> Result<()> {
    let sync_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("sync");

    let store = SyncStore::new(sync_dir);
    let file = store
        .load(std::path::Path::new(&path))
        .map_err(|e| e.to_string())?;

    let applier = SyncApplier::new(state.library());
    applier.apply(file).await.map_err(|e| e.to_string())
}

fn telegram_config_path(app: &AppHandle) -> Result<std::path::PathBuf> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("sync")
        .join("telegram_config.json"))
}

#[tauri::command]
pub async fn telegram_get_config(app: AppHandle) -> Result<Option<serde_json::Value>> {
    let path = telegram_config_path(&app)?;
    match TelegramConfig::load(&path).map_err(|e| e.to_string())? {
        Some(cfg) => Ok(Some(serde_json::json!({ "chatId": cfg.chat_id }))),
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn telegram_save_config(
    app: AppHandle,
    bot_token: String,
    chat_id: String,
) -> Result<()> {
    let path = telegram_config_path(&app)?;
    let config = TelegramConfig {
        bot_token,
        chat_id,
        last_message_id: None,
    };
    config.save(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn telegram_check_config(app: AppHandle) -> Result<bool> {
    let path = telegram_config_path(&app)?;
    match TelegramSync::load(&path).map_err(|e| e.to_string())? {
        Some(tg) => Ok(tg.check().await),
        None => Ok(false),
    }
}

#[tauri::command]
pub async fn telegram_push(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    sync_type: SyncType,
    device_name: Option<String>,
) -> Result<()> {
    let path = telegram_config_path(&app)?;
    let mut tg = TelegramSync::load(&path)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Telegram не настроен".to_string())?;

    let builder = SyncBuilder::new(state.library());
    let file = match sync_type {
        SyncType::Full => builder.build_full(device_name).await,
        SyncType::Diff => builder.build_diff(device_name).await,
    }
    .map_err(|e| e.to_string())?;

    tg.push(&file).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn telegram_pull(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<()> {
    let path = telegram_config_path(&app)?;
    let tg = TelegramSync::load(&path)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Telegram не настроен".to_string())?;

    let file = tg.pull().await.map_err(|e| e.to_string())?;
    let applier = SyncApplier::new(state.library());
    applier.apply(file).await.map_err(|e| e.to_string())
}

// ─── EQ ────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_eq_settings(state: State<'_, Arc<AppState>>) -> Result<EqSettingsDto> {
    Ok(state.get_eq_settings().into())
}

#[tauri::command]
pub async fn set_eq_settings(
    state: State<'_, Arc<AppState>>,
    settings: EqSettingsDto,
) -> Result<()> {
    state
        .set_eq_settings(settings.into())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_eq_presets(_state: State<'_, Arc<AppState>>) -> Result<Vec<EqPresetDto>> {
    let presets = nexus_player_backend::audio::eq::builtin_presets()
        .into_iter()
        .map(|(name, gains)| EqPresetDto {
            name: name.to_string(),
            gains: gains.to_vec(),
        })
        .collect();
    Ok(presets)
}

// ─── Direct Link Resolve ────────────────────────────────────────────────────

#[tauri::command]
pub async fn direct_link_resolve(
    state: State<'_, Arc<AppState>>,
    provider: String,
    url: String,
) -> Result<SearchResultDto> {
    let result = state
        .direct_link_resolve(&provider, &url)
        .await
        .map_err(|e| e.to_string())?;
    Ok(SearchResultDto::from(result))
}
