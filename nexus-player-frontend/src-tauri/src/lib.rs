mod commands;
mod dto;
mod error;
mod events;
#[cfg(test)]
mod wire_format_tests;

use nexus_player_backend::state::state::AppState;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use streaming::state::types::{DefaultCallback, DefaultCallbacks};
use tauri::{Emitter, Manager};

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
            commands::tracks::get_all_tracks,
            commands::tracks::get_cached_tracks,
            commands::tracks::get_cached_albums,
            commands::tracks::get_track,
            commands::tracks::search_tracks,
            commands::tracks::add_track_from_file,
            commands::tracks::get_local_library,
            commands::tracks::scan_local_library,
            commands::tracks::import_android_media,
            commands::tracks::get_music_sources,
            commands::tracks::add_music_source,
            commands::tracks::remove_music_source,
            commands::tracks::delete_track,
            commands::playlists::get_all_playlists,
            commands::playlists::get_playlist_with_tracks,
            commands::playlists::create_playlist,
            commands::playlists::create_playlist_from_container,
            commands::playlists::delete_playlist,
            commands::library::cleanup_library,
            commands::library::resolve_artwork,
            commands::library::clear_artwork_cache,
            commands::playlists::create_playlist_with_tracks,
            commands::playlists::add_track_to_playlist,
            commands::playlists::remove_track_from_playlist,
            commands::playlists::add_playlist_to_playlist,
            commands::playlists::remove_playlist_from_playlist,
            commands::queue::add_to_queue,
            commands::queue::remove_from_queue,
            commands::queue::clear_queue,
            commands::queue::get_queue,
            commands::queue::load_playlist_to_queue,
            commands::queue::load_library_page_to_queue,
            commands::queue::load_library_page,
            commands::queue::load_and_play_library_page_at_track,
            commands::queue::load_and_play_library_page,
            commands::queue::load_playlist_and_play,
            commands::queue::play_library_track,
            commands::queue::load_and_play_local_tracks,
            commands::queue::load_and_play_library_tracks,
            commands::queue::play_at_index,
            commands::queue::play_next,
            commands::queue::play_previous,
            commands::queue::pause,
            commands::queue::resume,
            commands::queue::stop,
            commands::queue::seek,
            commands::queue::set_volume,
            commands::queue::get_audio_state,
            commands::queue::set_repeat_mode,
            commands::queue::toggle_shuffle,
            commands::queue::start_radio,
            commands::queue::stop_radio,
            commands::queue_state::get_queue_state,
            commands::dialogs::select_audio_file,
            commands::dialogs::select_image_file,
            commands::dialogs::select_audio_files,
            commands::search::search_streaming,
            commands::search::handle_streaming_track,
            commands::queue_state::handle_search,
            commands::queue_state::expand_queue_container,
            commands::queue_state::play_queue_item,
            commands::cache::save_track_to_file,
            commands::cache::delete_cached_track,
            commands::config::get_app_config,
            commands::config::update_app_config,
            commands::config::update_provider_auth,
            commands::config::submit_challenge_result,
            commands::config::open_challenge_window,
            commands::cache::retry_playback,
            commands::history::get_play_history,
            commands::history::clear_play_history,
            commands::history::toggle_liked,
            commands::history::reorder_liked_tracks,
            commands::history::reorder_playlist_items,
            commands::history::list_download_tasks,
            commands::history::remove_download_task,
            commands::sync::export_sync,
            commands::sync::import_sync,
            commands::dialogs::select_sync_file,
            commands::sync::telegram_get_config,
            commands::sync::telegram_save_config,
            commands::sync::telegram_check_config,
            commands::sync::telegram_push,
            commands::sync::telegram_pull,
            commands::eq::get_eq_settings,
            commands::eq::set_eq_settings,
            commands::eq::get_eq_presets,
            commands::direct_link::direct_link_resolve,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
