mod commands;
mod dto;
mod events;

use nexus_player_backend::state::state::AppState;
use streaming::state::types::{DefaultCallbacks, TaskInfo};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{Manager, AppHandle, Emitter};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri::plugin::Builder::<_, ()>::new("vkLogin").build())
        .setup(|app| {
            let app_handle = app.handle().clone();
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory");

            std::fs::create_dir_all(&app_data_dir).expect("Failed to create app data directory");

            let db_path = app_data_dir.join("nexus.db");

            tracing::info!("Database path: {:?}", db_path);

            // Setup download callbacks
            let h1 = app_handle.clone();
            let h2 = app_handle.clone();
            let h3 = app_handle.clone();

            let default_callbacks = DefaultCallbacks {
                on_start: Some(Arc::new(move |id, _res| {
                    let _ = h1.emit("download-start", id);
                })),
                on_progress: Some(Arc::new(move |id, res| {
                    if let Ok(buffer) = res {
                        let progress = buffer.progress();
                        let _ = h2.emit("download-progress", (id, progress));
                    }
                })),
                on_finish: Some(Arc::new(move |id, res| {
                    let success = res.is_ok();
                    let _ = h3.emit("download-finish", (id, success));
                })),
            };

            let state = tauri::async_runtime::block_on(async {
                AppState::new(&db_path, default_callbacks)
                    .await
                    .expect("Failed to initialize AppState")
            });

            let state = Arc::new(state);

            events::start_event_loop(app.handle().clone(), Arc::clone(&state));

            app.manage(state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_all_tracks,
            commands::get_track,
            commands::search_tracks,
            commands::add_track_from_file,
            commands::delete_track,
            commands::get_all_playlists,
            commands::get_playlist_with_tracks,
            commands::create_playlist,
            commands::create_playlist_from_container,
            commands::delete_playlist,
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
            commands::get_app_config,
            commands::update_app_config,
            commands::get_vk_auth_url,
            commands::get_vk_auth_html,
            commands::extract_vk_token,
            commands::open_vk_login,
            commands::get_vk_profile,
            commands::get_play_history,
            commands::toggle_liked,
            commands::reorder_liked_tracks,
            commands::reorder_playlist_items,
            commands::list_download_tasks,
            commands::remove_download_task,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
