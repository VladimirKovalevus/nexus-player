use crate::dto::{AudioStateDto, BufferedRangeDto, QueueStateDto, TrackDto, WaveformDto};
use nexus_player_backend::audio::types::AudioEvent;
use nexus_player_backend::state::state::AppState;
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "payload")]
pub enum FrontendEvent {
    #[serde(rename_all = "camelCase")]
    PlaybackStarted {
        track_id: usize,
    },

    PlaybackPaused,

    PlaybackResumed,

    PlaybackStopped,

    #[serde(rename_all = "camelCase")]
    TrackFinished {
        track_id: usize,
    },

    #[serde(rename_all = "camelCase")]
    PositionUpdated {
        position_secs: f64,
        fft: Option<Vec<f32>>,
        pcm: Option<Vec<u8>>,
    },

    #[serde(rename_all = "camelCase")]
    DurationChanged {
        duration_secs: f64,
    },

    #[serde(rename_all = "camelCase")]
    VolumeChanged {
        volume: f32,
    },

    #[serde(rename_all = "camelCase")]
    BufferUpdated {
        ranges: Vec<BufferedRangeDto>,
    },

    #[serde(rename_all = "camelCase")]
    WaveformUpdated {
        waveforms: Vec<WaveformDto>,
    },

    #[serde(rename_all = "camelCase")]
    LoadingStarted {
        track: TrackDto,
    },

    #[serde(rename_all = "camelCase")]
    DownloadStarted {
        title: String,
    },

    #[serde(rename_all = "camelCase")]
    DownloadProgress {
        track_id: i64,
        progress: f32,
    },

    #[serde(rename_all = "camelCase")]
    DownloadFinished {
        track_id: i64,
        title: String,
        success: bool,
    },

    #[serde(rename_all = "camelCase")]
    AudioStateUpdated {
        state: Box<AudioStateDto>,
        queue: QueueStateDto,
    },

    #[serde(rename_all = "camelCase")]
    Error {
        message: String,
    },

    #[serde(rename_all = "camelCase")]
    TrackUnavailable {
        track_id: i64,
    },

    #[serde(rename_all = "camelCase")]
    ProviderChallenge {
        provider: String,
        url: String,
    },
}

pub fn start_event_loop(app_handle: AppHandle, state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        tracing::info!("Event loop started");

        loop {
            let events = state.poll_audio_events();

            let mut emit_state_update = false;

            for event in events {
                let frontend_event = match event {
                    AudioEvent::PlaybackStarted { track } => {
                        emit_state_update = true;
                        FrontendEvent::PlaybackStarted { track_id: track.id }
                    }
                    AudioEvent::PlaybackPaused => FrontendEvent::PlaybackPaused,
                    AudioEvent::PlaybackResumed => FrontendEvent::PlaybackResumed,
                    AudioEvent::PlaybackStopped => {
                        emit_state_update = true;
                        FrontendEvent::PlaybackStopped
                    }
                    AudioEvent::TrackFinished => {
                        let cl = state.clone();
                        tokio::spawn(async move {
                            cl.handle_track_finished(0).await;
                        });
                        emit_state_update = true;
                        FrontendEvent::TrackFinished { track_id: 0 }
                    }
                    AudioEvent::GaplessSwitched { track } => {
                        // Gapless switch: the audio engine already switched to the next track
                        // seamlessly. We only need to advance the queue cursor to stay in sync,
                        // without triggering playback (the track is already playing).
                        //
                        // This must be awaited in-place (not spawned) because the queue
                        // snapshot for the `AudioStateUpdated` event below is captured right
                        // after this loop iteration — a detached spawn would race it and the
                        // emitted snapshot would still show the pre-advance cursor position,
                        // making the queue UI lag one track behind on every gapless switch.
                        {
                            use nexus_player_backend::playlist::playback_queue::ActiveQueue;
                            let mut queue = state.lock_queue().await;
                            match &mut *queue {
                                ActiveQueue::Cursor(cq) => {
                                    let _ = cq.next().await;
                                }
                                ActiveQueue::Search(sq) => {
                                    sq.advance();
                                }
                            }
                        }
                        emit_state_update = true;
                        FrontendEvent::PlaybackStarted { track_id: track.id }
                    }
                    AudioEvent::PreloadNextTrack {
                        current_track_id: _,
                    } => {
                        let cl = state.clone();
                        tokio::spawn(async move {
                            cl.preload_next_track().await;
                        });
                        continue;
                    }
                    AudioEvent::PositionUpdated { position, fft, pcm } => {
                        FrontendEvent::PositionUpdated {
                            position_secs: position.as_secs_f64(),
                            fft,
                            pcm,
                        }
                    }
                    AudioEvent::DurationChanged { duration } => FrontendEvent::DurationChanged {
                        duration_secs: duration.as_secs_f64(),
                    },
                    AudioEvent::VolumeChanged { volume } => FrontendEvent::VolumeChanged { volume },
                    AudioEvent::Error { message } => FrontendEvent::Error { message },
                    AudioEvent::BufferUpdated { ranges } => FrontendEvent::BufferUpdated {
                        ranges: ranges.into_iter().map(BufferedRangeDto::from).collect(),
                    },
                    AudioEvent::WaveformUpdated { waveforms } => FrontendEvent::WaveformUpdated {
                        waveforms: waveforms.into_iter().map(WaveformDto::from).collect(),
                    },
                    AudioEvent::LoadingStarted { track } => {
                        emit_state_update = true;
                        FrontendEvent::LoadingStarted {
                            track: TrackDto::from(track),
                        }
                    }
                    AudioEvent::BufferingStarted => {
                        continue;
                    }
                    AudioEvent::BufferingFinished => {
                        continue;
                    }
                    AudioEvent::TrackUnavailable { track_id } => {
                        FrontendEvent::TrackUnavailable { track_id }
                    }
                    AudioEvent::ProviderChallenge { provider, url } => {
                        FrontendEvent::ProviderChallenge { provider, url }
                    }
                };

                if let Err(e) = app_handle.emit("audio-event", &frontend_event) {
                    tracing::error!("Failed to emit event: {}", e);
                }
            }

            if !emit_state_update {
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }

            let audio_state: AudioStateDto = state.audio_state().into();

            let queue_state = {
                let queue = state.lock_queue().await;
                crate::commands::queue_state::build_queue_state_dto(&queue)
            };
            let queue_state = match queue_state {
                Ok(dto) => dto,
                Err(e) => {
                    tracing::error!("Failed to build queue state, skipping update: {e:?}");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
            };

            if let Err(e) = app_handle.emit(
                "audio-event",
                &FrontendEvent::AudioStateUpdated {
                    state: Box::new(audio_state),
                    queue: queue_state,
                },
            ) {
                tracing::error!("Failed to emit audio state: {}", e);
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });
}
