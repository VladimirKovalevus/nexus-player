use crate::dto::{
    AudioStateDto, BufferedRangeDto, QueueEntryDto, QueueStateDto, TrackDto, WaveformDto,
};
use nexus_player_backend::audio::types::{AudioEvent, AudioState};
use nexus_player_backend::state::state::AppState;
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

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
    AudioStateUpdated {
        state: AudioStateDto,
        queue: QueueStateDto,
    },

    #[serde(rename_all = "camelCase")]
    Error {
        message: String,
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
                        cl.handle_track_finished(0).await;
                        emit_state_update = true;
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
                use crate::dto::{
                    ContainerMetaDto, CurrentPositionDto, QueueEntryDto, SearchContextDto,
                };
                use nexus_player_backend::playlist::playback_queue::{
                    ActiveQueue, ContainerKind, QueueEntry,
                };

                let queue = state.lock_queue().await;
                match &*queue {
                    ActiveQueue::Cursor(cq) => {
                        let (tracks, current_index) = cq.snapshot();
                        let entries: Vec<QueueEntryDto> = tracks
                            .into_iter()
                            .map(|t| QueueEntryDto::Track { track: t.into() })
                            .collect();
                        let current_position = current_index.map(|idx| CurrentPositionDto {
                            item_index: idx,
                            track_within_container: None,
                        });
                        let repeat_mode = match cq.repeat_mode {
                            nexus_player_backend::playlist::playback_queue::RepeatMode::Off => {
                                "off"
                            }
                            nexus_player_backend::playlist::playback_queue::RepeatMode::All => {
                                "all"
                            }
                            nexus_player_backend::playlist::playback_queue::RepeatMode::One => {
                                "one"
                            }
                        };
                        QueueStateDto {
                            entries,
                            current_position,
                            repeat_mode: repeat_mode.to_string(),
                            shuffle_enabled: cq.shuffle_enabled,
                            search_context: None,
                        }
                    }
                    ActiveQueue::Search(sq) => {
                        let (entries, position) = sq.snapshot();
                        let entries_dto: Vec<QueueEntryDto> = entries
                            .into_iter()
                            .map(|entry| match entry {
                                QueueEntry::Track(track) => QueueEntryDto::Track {
                                    track: track.into(),
                                },
                                QueueEntry::Container {
                                    info,
                                    tracks,
                                    play_index,
                                } => {
                                    let kind_str = match info.kind {
                                        ContainerKind::Album => "album",
                                        ContainerKind::Playlist => "playlist",
                                    };
                                    QueueEntryDto::Container {
                                        meta: ContainerMetaDto {
                                            kind: kind_str.to_string(),
                                            id: info.id,
                                            system: info.system,
                                            title: info.title,
                                            artist: info.artist,
                                            track_count: info.track_count,
                                            thumbnail: info.thumbnail,
                                        },
                                        tracks: tracks
                                            .map(|ts| ts.into_iter().map(|t| t.into()).collect()),
                                        play_index,
                                    }
                                }
                            })
                            .collect();
                        let current_position = position.map(|p| CurrentPositionDto {
                            item_index: p.item_index,
                            track_within_container: p.track_within_container,
                        });
                        QueueStateDto {
                            entries: entries_dto,
                            current_position,
                            repeat_mode: "off".to_string(),
                            shuffle_enabled: false,
                            search_context: Some(SearchContextDto {
                                system: sq.system.clone(),
                                query: sq.query.clone(),
                            }),
                        }
                    }
                }
            };

            if let Err(e) = app_handle.emit(
                "audio-event",
                &FrontendEvent::AudioStateUpdated {
                    state: audio_state,
                    queue: queue_state,
                },
            ) {
                tracing::error!("Failed to emit audio state: {}", e);
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });
}
