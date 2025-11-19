use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use ::nexus_player_backend::audio::types::TrackSource;
use ::nexus_player_backend::state::state::AppState;
use nexus_player_backend::audio::types::Track;
use tauri::State;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}
#[tauri::command]
fn play(path: &str, state: State<'_, AppState>) -> String {
    // let state_guard = state.lock().unwrap();
    let state_guard = state;
    state_guard.update_playlist(|p| {
        p.add_track(Track {
            id: 0,
            format: None,
            source: TrackSource::local(PathBuf::from(path)),
        });
    });
    state_guard.play_next();
    format!("Playing: {}", path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = AppState::new().expect("Не удалось инициализировать AppState");
    tauri::Builder::default()
        .manage(app)
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, play])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
