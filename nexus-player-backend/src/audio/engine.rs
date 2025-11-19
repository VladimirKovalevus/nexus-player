use super::playback::PlaybackEngine;
use super::types::{AudioCommand, AudioEvent, AudioState, PlaybackState, Track};
use crate::error::AudioError;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;
use tracing::{info, warn};

const COMMAND_BUFFER_SIZE: usize = 32;
const EVENT_BUFFER_SIZE: usize = 128;

pub struct AudioEngine {
    command_tx: Sender<AudioCommand>,
    event_rx: Receiver<AudioEvent>,
    state: Arc<RwLock<AudioState>>,
    audio_thread: Option<std::thread::JoinHandle<()>>,
}

impl AudioEngine {
    pub fn new() -> Result<Self, AudioError> {
        info!("Инициализация аудио движка");

        let (command_tx, command_rx) = mpsc::sync_channel(COMMAND_BUFFER_SIZE);
        let (event_tx, event_rx) = mpsc::sync_channel(EVENT_BUFFER_SIZE);

        let state = Arc::new(RwLock::new(AudioState::default()));

        let audio_thread = Self::spawn_audio_thread(command_rx, event_tx, Arc::clone(&state))?;

        Ok(Self {
            command_tx,
            event_rx,
            state,
            audio_thread: Some(audio_thread),
        })
    }

    pub fn play(&self, track: Track) {
        info!("Команда: воспроизвести трек {}", track.id);
        let _ = self.command_tx.send(AudioCommand::Play(track));
    }

    pub fn pause(&self) {
        info!("Команда: пауза");
        let _ = self.command_tx.send(AudioCommand::Pause);
    }

    pub fn resume(&self) {
        info!("Команда: возобновить");
        let _ = self.command_tx.send(AudioCommand::Resume);
    }

    pub fn stop(&self) {
        info!("Команда: остановить");
        let _ = self.command_tx.send(AudioCommand::Stop);
    }

    pub fn seek(&self, position: Duration) {
        info!("Команда: перемотка на {:?}", position);
        let _ = self.command_tx.send(AudioCommand::Seek(position));
    }

    pub fn set_volume(&self, volume: f32) {
        let _ = self.command_tx.send(AudioCommand::SetVolume(volume));
    }

    pub fn state(&self) -> AudioState {
        self.state.read().clone()
    }

    pub fn poll_event(&self) -> Option<AudioEvent> {
        self.event_rx.try_recv().ok()
    }

    pub fn poll_all_events(&self) -> Vec<AudioEvent> {
        self.event_rx.try_iter().collect()
    }

    fn spawn_audio_thread(
        command_rx: Receiver<AudioCommand>,
        event_tx: Sender<AudioEvent>,
        state: Arc<RwLock<AudioState>>,
    ) -> Result<std::thread::JoinHandle<()>, AudioError> {
        let builder = std::thread::Builder::new()
            .name("audio-thread".to_string())
            .stack_size(4 * 1024 * 1024);

        let handle = builder
            .spawn(move || {
                info!("Аудио поток запущен");
                Self::audio_thread_loop(command_rx, event_tx, state);
                info!("Аудио поток завершен");
            })
            .map_err(|e| {
                AudioError::StreamBuildError(format!("Не удалось создать аудио поток: {}", e))
            })?;

        Ok(handle)
    }

    fn audio_thread_loop(
        command_rx: Receiver<AudioCommand>,
        event_tx: Sender<AudioEvent>,
        state: Arc<RwLock<AudioState>>,
    ) {
        // Создаем playback engine
        let mut playback = match PlaybackEngine::new() {
            Ok(p) => p,
            Err(e) => {
                warn!("Не удалось создать playback engine: {}", e);
                return;
            }
        };

        loop {
            // Обновление позиции
            if playback.is_playing() {
                let now = std::time::Instant::now();
                if now.duration_since(last_position_update) >= Duration::from_millis(100) {
                    last_position_update = now;

                    let position = playback.position();
                    state.write().position = position;
                    let _ = event_tx.send(AudioEvent::PositionUpdated { position });

                    // Проверяем завершение трека
                    if let Some(duration) = playback.duration() {
                        if position >= duration {
                            info!("Трек завершен");
                            let track_id = state.read().current_track.as_ref().map(|t| t.id);
                            if let Some(id) = track_id {
                                let _ = event_tx.send(AudioEvent::TrackFinished { track_id: id });
                            }

                            state.write().playback_state = PlaybackState::Stopped;
                            playback.stop();
                        }
                    }
                }
            }

            // Обработка команд
            match command_rx.recv_timeout(Duration::from_millis(10)) {
                Ok(AudioCommand::Play(track)) => {
                    if let Some(ref url) = track.stream_url {
                        info!("Начинаем воспроизведение streaming трека: {}", url);
                    } else {
                        info!("Начинаем воспроизведение: {:?}", track.path);
                    }

                    // Используем stream_url если есть, иначе используем path
                    let result = if let Some(url) = &track.stream_url {
                        if track.is_chunked_url {
                            // Chunked URL с bidirectional seek
                            playback.play_chunked_url(url)
                        } else {
                            // Обычный URL (forward-only seek)
                            #[allow(deprecated)]
                            playback.play_url(url)
                        }
                    } else {
                        playback.play(&track.path)
                    };

                    match result {
                        Ok(_) => {
                            // Получаем длительность
                            let duration = playback.duration().unwrap_or(Duration::ZERO);

                            // Обновляем состояние
                            {
                                let mut s = state.write();
                                s.playback_state = PlaybackState::Playing;
                                s.current_track = Some(track.clone());
                                s.position = Duration::ZERO;
                                s.duration = duration;
                            }

                            // Сбрасываем таймер обновления позиции для нового трека
                            last_position_update = std::time::Instant::now();

                            // Отправляем события
                            if duration > Duration::ZERO {
                                let _ = event_tx.send(AudioEvent::DurationChanged { duration });
                            }
                            let _ = event_tx.send(AudioEvent::PlaybackStarted { track });
                            info!("Воспроизведение началось, длительность: {:?}", duration);
                        }
                        Err(e) => {
                            warn!("Ошибка воспроизведения: {}", e);
                            let _ = event_tx.send(AudioEvent::Error {
                                message: format!("Ошибка воспроизведения: {}", e),
                            });
                        }
                    }
                }

                Ok(AudioCommand::Pause) => {
                    info!("Пауза");
                    playback.pause();
                    state.write().playback_state = PlaybackState::Paused;
                    let _ = event_tx.send(AudioEvent::PlaybackPaused);
                }

                Ok(AudioCommand::Resume) => {
                    info!("Возобновление");
                    playback.resume();
                    state.write().playback_state = PlaybackState::Playing;

                    // Сбрасываем таймер обновления позиции
                    last_position_update = std::time::Instant::now();

                    let _ = event_tx.send(AudioEvent::PlaybackResumed);
                }

                Ok(AudioCommand::Stop) => {
                    info!("Остановка");
                    playback.stop();

                    {
                        let mut s = state.write();
                        s.playback_state = PlaybackState::Stopped;
                        s.current_track = None;
                        s.position = Duration::ZERO;
                    }

                    let _ = event_tx.send(AudioEvent::PlaybackStopped);
                }

                Ok(AudioCommand::Seek(position)) => {
                    info!("Перемотка на {:?}", position);

                    match playback.seek(position) {
                        Ok(_) => {
                            state.write().position = position;

                            // Сбрасываем таймер обновления позиции после перемотки
                            last_position_update = std::time::Instant::now();

                            let _ = event_tx.send(AudioEvent::PositionUpdated { position });
                        }
                        Err(e) => {
                            warn!("Ошибка перемотки: {}", e);
                        }
                    }
                }

                Ok(AudioCommand::SetVolume(volume)) => {
                    info!("Установка громкости: {}", volume);
                    // TODO: Реализовать управление громкостью в PlaybackEngine
                    state.write().volume = volume;
                    let _ = event_tx.send(AudioEvent::VolumeChanged { volume });
                }

                Ok(AudioCommand::Shutdown) => {
                    info!("Завершение работы аудио потока");
                    break;
                }

                Err(crossbeam::channel::RecvTimeoutError::Timeout) => {
                    // Нет команд - продолжаем работу
                }

                Err(crossbeam::channel::RecvTimeoutError::Disconnected) => {
                    warn!("Канал команд отключен");
                    break;
                }
            }
        }
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        info!("Завершение работы аудио движка");
        let _ = self.command_tx.send(AudioCommand::Shutdown);
    }
}
