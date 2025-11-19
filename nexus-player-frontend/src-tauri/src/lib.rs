use std::{
    path::PathBuf,
    str::FromStr,
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
    state.update_playlist(|p| {
        p.add_track(Track {
            id: 0,
            format: None,
            source: TrackSource::local(PathBuf::from(path)),
        });
    });
    state.play_next();
    format!("Hello, {}! You've been greeted from Rust!", path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = AppState::new().expect("Не удалось инициализировать AppState");
    let arc = Arc::new(Mutex::new(app));
    tauri::Builder::default()
        .manage(arc)
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, play])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
