use crate::dto::AudioStateDto;
use nexus_player_backend::audio::types::{AudioEvent, AudioState};
use nexus_player_backend::state::state::AppState;
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "payload")]
pub enum FrontendEvent {
    PlaybackStarted { track_id: usize },

    PlaybackPaused,

    PlaybackResumed,

    PlaybackStopped,

    TrackFinished { track_id: usize },

    PositionUpdated { position_secs: f64 },

    DurationChanged { duration_secs: f64 },

    VolumeChanged { volume: f32 },

    AudioStateUpdated { state: AudioStateDto },

    Error { message: String },
}

pub fn start_event_loop(app_handle: AppHandle, state: Arc<AppState>) {
    std::thread::spawn(move || {
        tracing::info!("Event loop started");

        loop {
            let events = state.poll_audio_events();

            for event in events {
                let frontend_event = match event {
                    AudioEvent::PlaybackStarted => FrontendEvent::PlaybackStarted { track_id: 0 },
                    AudioEvent::PlaybackPaused => FrontendEvent::PlaybackPaused,
                    AudioEvent::PlaybackResumed => FrontendEvent::PlaybackResumed,
                    AudioEvent::PlaybackStopped => FrontendEvent::PlaybackStopped,
                    AudioEvent::TrackFinished => {
                        state.handle_track_finished(0);
                        FrontendEvent::TrackFinished { track_id: 0 }
                    }
                    AudioEvent::PositionUpdated { position } => FrontendEvent::PositionUpdated {
                        position_secs: position.as_secs_f64(),
                    },
                    AudioEvent::DurationChanged { duration } => FrontendEvent::DurationChanged {
                        duration_secs: duration.as_secs_f64(),
                    },
                    AudioEvent::VolumeChanged { volume } => FrontendEvent::VolumeChanged { volume },
                    AudioEvent::Error { message } => FrontendEvent::Error { message },
                    AudioEvent::BufferingStarted => {
                        continue;
                    }
                    AudioEvent::BufferingFinished => {
                        continue;
                    }
                };

                if let Err(e) = app_handle.emit("audio-event", &frontend_event) {
                    tracing::error!("Failed to emit event: {}", e);
                }
            }

            let audio_state: AudioStateDto = state.audio_state().into();
            if let Err(e) = app_handle.emit(
                "audio-event",
                &FrontendEvent::AudioStateUpdated { state: audio_state },
            ) {
                tracing::error!("Failed to emit audio state: {}", e);
            }

            std::thread::sleep(Duration::from_millis(100));
        }
    });
}
