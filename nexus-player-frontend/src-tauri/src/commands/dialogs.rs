use super::Result;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

/// Copy a user-picked image into `<app_data_dir>/cache/artwork/custom/<sha256>.<ext>`,
/// the only place the asset protocol may serve from. Same content, same file.
pub(crate) fn copy_into_artwork_cache(
    source: &Path,
    app_data_dir: &Path,
) -> anyhow::Result<PathBuf> {
    let bytes = std::fs::read(source)?;
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let dir = app_data_dir.join("cache").join("artwork").join("custom");
    std::fs::create_dir_all(&dir)?;
    let name = match source.extension().and_then(|e| e.to_str()) {
        Some(ext) => format!("{hash}.{}", ext.to_ascii_lowercase()),
        None => hash,
    };
    let target = dir.join(name);
    if !target.exists() {
        std::fs::write(&target, &bytes)?;
    }
    Ok(target)
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

    let Some(picked) = file_path else {
        return Ok(None);
    };
    let source = picked
        .into_path()
        .map_err(|e| anyhow::anyhow!("Unsupported file path: {e}"))?;
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| anyhow::anyhow!("No app data dir: {e}"))?;
    let copy = copy_into_artwork_cache(&source, &app_data_dir)?;

    Ok(Some(copy.to_string_lossy().into_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_under_custom_dir_once() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let src = tmp.path().join("cover.PNG");
        std::fs::write(&src, b"image-bytes").expect("write");
        let data = tmp.path().join("data");

        let a = copy_into_artwork_cache(&src, &data).expect("copy");
        let b = copy_into_artwork_cache(&src, &data).expect("copy again");

        assert_eq!(a, b);
        assert!(a.starts_with(data.join("cache").join("artwork").join("custom")));
        assert_eq!(std::fs::read(&a).expect("read"), b"image-bytes");
        let count = std::fs::read_dir(a.parent().expect("parent"))
            .expect("dir")
            .count();
        assert_eq!(count, 1);
    }
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
