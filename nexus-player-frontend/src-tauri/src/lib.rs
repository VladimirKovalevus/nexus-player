mod commands;
mod dto;
mod events;

use nexus_player_backend::state::state::AppState;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use streaming::state::types::{DefaultCallback, DefaultCallbacks, TaskInfo};
use tauri::{AppHandle, Emitter, Manager};

/// How long after the last audio download callback artwork fetching stays held.
const AUDIO_IDLE_GRACE: Duration = Duration::from_millis(1500);
/// How often the watchdog re-evaluates the gate.
const GATE_POLL_INTERVAL: Duration = Duration::from_millis(400);

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Hold artwork fetching while audio is downloading.
///
/// Artwork is the lowest-priority network user in the app: the moment a track
/// starts downloading, cover fetches stop, and they only resume once the audio
/// callbacks have been quiet for [`AUDIO_IDLE_GRACE`].
fn spawn_artwork_gate_watchdog(state: Arc<AppState>, audio_activity: Arc<AtomicU64>) {
    tauri::async_runtime::spawn(async move {
        // Seeded with the cache's own initial state, so an idle startup doesn't
        // write the gate at all.
        let mut applied = false;

        loop {
            tokio::time::sleep(GATE_POLL_INTERVAL).await;

            let last = audio_activity.load(Ordering::Relaxed);
            let busy = last != 0
                && now_millis().saturating_sub(last) < AUDIO_IDLE_GRACE.as_millis() as u64;

            // Only on change: every call wakes the image worker, and re-sending
            // the same value would restart its settling period forever.
            if applied != busy {
                state.set_artwork_paused(busy);
                applied = busy;
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri::plugin::Builder::<_, ()>::new("soundCloudChallenge").build())
        .setup(|app| {
            let app_handle = app.handle().clone();
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory");

            std::fs::create_dir_all(&app_data_dir).expect("Failed to create app data directory");

            let db_path = app_data_dir.join("nexus.db");

            tracing::info!("Database path: {:?}", db_path);

            // The webview loads cached covers over the asset protocol, which
            // serves only explicitly allowed paths. The `"**/*"` glob in
            // tauri.conf.json never matches an absolute path, so grant this one
            // directory at runtime rather than opening the whole filesystem.
            let artwork_dir = app_data_dir.join("cache").join("artwork");
            std::fs::create_dir_all(&artwork_dir).expect("Failed to create artwork cache directory");
            if let Err(e) = app.asset_protocol_scope().allow_directory(&artwork_dir, true) {
                tracing::error!("Failed to allow artwork cache in asset scope: {}", e);
            }

            // Setup download callbacks
            let h1 = app_handle.clone();
            let h2 = app_handle.clone();
            let h3 = app_handle.clone();

            // Heartbeat for the artwork gate: audio download callbacks stamp this,
            // and the watchdog below holds artwork fetching while it stays fresh.
            // A heartbeat rather than a start/finish counter because a cancelled
            // task never fires `on_finish` — a counter would stick and pause
            // artwork for the rest of the session.
            let audio_activity = Arc::new(AtomicU64::new(0));
            let beat_start = Arc::clone(&audio_activity);
            let beat_progress = Arc::clone(&audio_activity);

            let default_callbacks = DefaultCallbacks {
                on_start: Some(Arc::new(move |id, _res, meta: Option<streaming::state::types::StreamMetadata>| {
                    beat_start.store(now_millis(), Ordering::Relaxed);
                    let title = meta.as_ref().and_then(|m| m.title.clone()).unwrap_or_else(|| format!("Track #{}", id));
                    let _ = h1.emit("download-start", (id, title));
                }) as Arc<DefaultCallback>),
                on_progress: Some(Arc::new(move |id, res: Result<streaming::state::types::BufferType, streaming::error::StreamingError>, _meta| {
                    beat_progress.store(now_millis(), Ordering::Relaxed);
                    if let Ok(buffer) = res {
                        let progress = buffer.progress();
                        let byte_size = buffer.byte_size();
                        let _ = h2.emit("download-progress", (id, progress, byte_size));
                    }
                }) as Arc<DefaultCallback>),
                on_finish: Some(Arc::new(move |id, res: Result<streaming::state::types::BufferType, streaming::error::StreamingError>, meta: Option<streaming::state::types::StreamMetadata>| {
                    let success = res.is_ok();
                    let title = meta.and_then(|m| m.title).unwrap_or_else(|| format!("Track #{}", id));
                    let _ = h3.emit("download-finish", (id, success, title));
                }) as Arc<DefaultCallback>),
            };

            let state = tauri::async_runtime::block_on(async {
                AppState::new(&db_path, default_callbacks)
                    .await
                    .expect("Failed to initialize AppState")
            });

            let state = Arc::new(state);

            // Announce each cover as it lands on disk, so open views can swap the
            // remote URL for the local file without re-querying.
            let h4 = app_handle.clone();
            state.set_artwork_on_ready(Arc::new(
                move |url: String, variant: streaming::images::ImageVariant, path: PathBuf| {
                    let _ = h4.emit(
                        "artwork-cached",
                        (url, variant.as_str(), path.to_string_lossy().into_owned()),
                    );
                },
            ));

            spawn_artwork_gate_watchdog(Arc::clone(&state), audio_activity);

            let prune_state = Arc::clone(&state);
            tauri::async_runtime::spawn(async move {
                if let Err(e) = prune_state.prune_artwork_cache().await {
                    tracing::warn!("Artwork cache prune failed: {}", e);
                }
            });

            events::start_event_loop(app.handle().clone(), Arc::clone(&state));

            app.manage(state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_all_tracks,
            commands::get_cached_tracks,
            commands::get_cached_albums,
            commands::get_track,
            commands::search_tracks,
            commands::add_track_from_file,
            commands::get_local_library,
            commands::scan_local_library,
            commands::import_android_media,
            commands::get_music_sources,
            commands::add_music_source,
            commands::remove_music_source,
            commands::delete_track,
            commands::get_all_playlists,
            commands::get_playlist_with_tracks,
            commands::create_playlist,
            commands::create_playlist_from_container,
            commands::delete_playlist,
            commands::cleanup_library,
            commands::resolve_artwork,
            commands::clear_artwork_cache,
            commands::create_playlist_with_tracks,
            commands::add_track_to_playlist,
            commands::remove_track_from_playlist,
            commands::add_playlist_to_playlist,
            commands::remove_playlist_from_playlist,
            commands::add_to_queue,
            commands::remove_from_queue,
            commands::clear_queue,
            commands::get_queue,
            commands::load_playlist_to_queue,
            commands::load_library_page_to_queue,
            commands::load_library_page,
            commands::load_and_play_library_page_at_track,
            commands::load_and_play_library_page,
            commands::load_playlist_and_play,
            commands::play_library_track,
            commands::load_and_play_local_tracks,
            commands::load_and_play_library_tracks,
            commands::play_at_index,
            commands::play_next,
            commands::play_previous,
            commands::pause,
            commands::resume,
            commands::stop,
            commands::seek,
            commands::set_volume,
            commands::get_audio_state,
            commands::set_repeat_mode,
            commands::toggle_shuffle,
            commands::start_radio,
            commands::stop_radio,
            commands::get_queue_state,
            commands::select_audio_file,
            commands::select_image_file,
            commands::select_audio_files,
            commands::search_streaming,
            commands::handle_streaming_track,
            commands::handle_search,
            commands::expand_queue_container,
            commands::play_queue_item,
            commands::save_track_to_file,
            commands::delete_cached_track,
            commands::get_app_config,
            commands::update_app_config,
            commands::update_provider_auth,
            commands::submit_challenge_result,
            commands::open_challenge_window,
            commands::retry_playback,
            commands::get_play_history,
            commands::clear_play_history,
            commands::toggle_liked,
            commands::reorder_liked_tracks,
            commands::reorder_playlist_items,
            commands::list_download_tasks,
            commands::remove_download_task,
            commands::export_sync,
            commands::import_sync,
            commands::select_sync_file,
            commands::telegram_get_config,
            commands::telegram_save_config,
            commands::telegram_check_config,
            commands::telegram_push,
            commands::telegram_pull,
            commands::get_eq_settings,
            commands::set_eq_settings,
            commands::get_eq_presets,
            commands::direct_link_resolve,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
