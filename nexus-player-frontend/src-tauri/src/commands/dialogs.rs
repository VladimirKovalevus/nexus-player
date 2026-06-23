use super::Result;
use tauri_plugin_dialog::DialogExt;

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
