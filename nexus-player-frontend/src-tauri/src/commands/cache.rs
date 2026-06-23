use super::Result;
use crate::dto::{hash_str_to_i64, TrackDto};
use crate::events::FrontendEvent;
use nexus_player_backend::library::types::{CacheStatus, TrackId, TrackSource};
use nexus_player_backend::state::state::AppState;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};

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
