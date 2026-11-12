use super::Result;
use crate::dto::{hash_str_to_i64, TrackDto};
use crate::events::FrontendEvent;
use nexus_player_backend::library::types::{CacheStatus, TrackId, TrackSource};
use nexus_player_backend::state::state::AppState;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};

#[tauri::command]
pub async fn retry_playback(state: State<'_, Arc<AppState>>) -> Result<()> {
    state
        .play_current()
        .await
        .map_err(crate::error::CommandError::from)
}

#[tauri::command]
pub async fn save_track_to_file(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    track: TrackDto,
) -> Result<()> {
    let downloads_dir = app.path().cache_dir()?.join("nexus-downloads");
    std::fs::create_dir_all(&downloads_dir)?;

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
                app.emit("download-finish", (track_id, true, track.title.clone()))?;
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
                let app_handle = app_handle.clone();
                let state_arc = state_arc.clone();
                let track_title = track_title.clone();
                let file_path = res.as_ref().ok().map(|buffer| {
                    downloads_dir.join(format!(
                        "{} - {}.{}",
                        track.artist.clone().unwrap_or("Unknown".to_string()),
                        track.title,
                        buffer.container()
                    ))
                });
                // Completion means the file and library status have both been persisted.
                tauri::async_runtime::spawn(async move {
                    let outcome = async {
                        let buffer = res.map_err(|e| e.to_string())?;
                        let path = file_path.ok_or("Missing cache path")?;
                        tokio::task::spawn_blocking({
                            let path = path.clone();
                            move || buffer.save_to_file(path)
                        })
                        .await
                        .map_err(|e| e.to_string())?
                        .map_err(|e| e.to_string())?;
                        let mut lib_track = state_arc
                            .library()
                            .get_track(TrackId(track_id))
                            .await
                            .map_err(|e| e.to_string())?
                            .ok_or("Track not found in library")?;
                        lib_track.cache = CacheStatus::Cached {
                            path,
                            downloaded_at: chrono::Utc::now(),
                        };
                        state_arc
                            .library()
                            .update_track(lib_track)
                            .await
                            .map_err(|e| e.to_string())?;
                        Ok::<(), String>(())
                    }
                    .await;
                    if let Err(error) = &outcome {
                        tracing::error!(track_id, %error, "Failed to cache track");
                    }
                    let success = outcome.is_ok();
                    let _ = app_handle.emit(
                        "audio-event",
                        FrontendEvent::DownloadFinished {
                            track_id,
                            title: track_title.clone(),
                            success,
                        },
                    );
                    let _ = app_handle.emit("download-finish", (track_id, success, track_title));
                });
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
            "Track not cached yet, queueing download for {:?}",
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
            .await?;

        tracing::info!("Track download initiated for {:?}", track_id);
    }

    Ok(())
}

#[tauri::command]
pub async fn delete_cached_track(state: State<'_, Arc<AppState>>, track_id: i64) -> Result<()> {
    let mut lib_track = state
        .library()
        .get_track(TrackId(track_id))
        .await?
        .ok_or_else(|| "Track not found in library".to_string())?;

    if let CacheStatus::Cached { path, .. } = &lib_track.cache {
        if let Err(e) = std::fs::remove_file(path) {
            if e.kind() != std::io::ErrorKind::NotFound {
                return Err(e.to_string().into());
            }
        }
    }

    lib_track.cache = CacheStatus::None;
    state.library().update_track(lib_track).await?;

    tracing::info!("Deleted cached file for track {}", track_id);
    Ok(())
}
