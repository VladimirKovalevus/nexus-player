use super::playback::PlaybackEngine;
use super::types::{AudioCommand, AudioEvent, AudioState, BufferedRange, PlaybackState, Track};
use crate::audio::types::{AudioSource, MediaSource};
use crate::error::AudioError;
use anyhow::Error;
use crossbeam::channel;
use crossbeam::channel::{Receiver, Sender};
use rustfft::{num_complex::Complex, FftPlanner};
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;
use streaming::multi_downloader::source::HlsSource;
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

        let (command_tx, command_rx) = crossbeam::channel::bounded(COMMAND_BUFFER_SIZE);
        let (event_tx, event_rx) = crossbeam::channel::bounded(EVENT_BUFFER_SIZE);

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
    pub fn play_source(&self, track: Track, source: Box<dyn MediaSource>) {
        info!("Команда: воспроизвести трек ");
        let _ = self
            .command_tx
            .send(AudioCommand::PlaySource { track, source });
    }
    pub fn play_hls(&self, track: Track, source: HlsSource) {
        info!("Команда: воспроизвести трек ");
        let _ = self
            .command_tx
            .send(AudioCommand::PlayHLS { track, source });
    }

    pub fn set_loading(&self, track: Track) {
        let _ = self.command_tx.send(AudioCommand::SetLoading(track));
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
        self.state.read().unwrap().clone()
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
        let mut playback = match PlaybackEngine::new() {
            Ok(p) => p,
            Err(e) => {
                warn!("Не удалось создать playback engine: {}", e);
                return;
            }
        };
        let mut last_position_update = std::time::Instant::now();
        let mut fft_planner = FftPlanner::<f32>::new();
        let fft_size = 1024;
        let fft = fft_planner.plan_fft_forward(fft_size);

        loop {
            // Проверка необходимости пересоздания стрима (например, при смене устройства)
            if playback.needs_recreation() {
                if let Err(e) = playback.recreate_stream() {
                    warn!("Не удалось автоматически пересоздать аудио стрим: {}", e);
                    let _ = event_tx.send(AudioEvent::Error {
                        message: format!("Ошибка аудио устройства: {}", e),
                    });
                }
            }

            if playback.is_playing() || playback.buffered_ranges().len() > 0 {
                let now = std::time::Instant::now();
                if now.duration_since(last_position_update) >= Duration::from_millis(100) {
                    last_position_update = now;

                    let position = playback.position();
                    let ranges = playback.buffered_ranges();
                    let buffered_ranges: Vec<BufferedRange> = ranges
                        .into_iter()
                        .map(|(start, end)| BufferedRange {
                            start: start as f32,
                            end: end as f32,
                        })
                        .collect();

                    let waveforms = playback.waveforms();

                    let mut fft_data: Option<Vec<f32>> = None;
                    let mut pcm_data: Option<Vec<u8>> = None;
                    if playback.is_playing() {
                        let samples = playback.get_recent_samples();
                        let channels = playback.channels();
                        let total_frames = samples.len() / channels;

                        if total_frames >= fft_size {
                            let start_frame = total_frames - fft_size;
                            let mut mono_samples: Vec<f32> = Vec::with_capacity(fft_size);

                            for i in 0..fft_size {
                                let frame_idx = start_frame + i;
                                let mut mix = 0.0f32;
                                for c in 0..channels {
                                    mix += samples[frame_idx * channels + c];
                                }
                                mono_samples.push(mix / channels as f32);
                            }

                            // PCM: convert to u8 (0-255) for butterchurn time-domain input
                            pcm_data = Some(
                                mono_samples
                                    .iter()
                                    .map(|&s| ((s * 127.0) + 128.0).clamp(0.0, 255.0) as u8)
                                    .collect(),
                            );

                            // FFT with Hann window
                            let mut buffer: Vec<Complex<f32>> = mono_samples
                                .iter()
                                .enumerate()
                                .map(|(i, &s)| {
                                    let w = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (fft_size - 1) as f32).cos());
                                    Complex { re: s * w, im: 0.0 }
                                })
                                .collect();

                            fft.process(&mut buffer);

                            fft_data = Some(
                                buffer
                                    .iter()
                                    .take(fft_size / 2)
                                    .map(|c| (c.re.powi(2) + c.im.powi(2)).sqrt())
                                    .collect(),
                            );
                        }
                    }

                    {
                        let mut s = state.write().unwrap();
                        s.position = position;

                        if let Some(ref fd) = fft_data {
                            s.fft = fd.clone();
                        }

                        if s.buffered_ranges != buffered_ranges {
                            s.buffered_ranges = buffered_ranges.clone();
                            let _ = event_tx.send(AudioEvent::BufferUpdated {
                                ranges: buffered_ranges,
                            });
                        }

                        if s.waveforms != waveforms {
                            s.waveforms = waveforms.clone();
                            let _ = event_tx.send(AudioEvent::WaveformUpdated { waveforms });
                        }
                    }

                    if playback.is_playing() {
                        let _ = event_tx.send(AudioEvent::PositionUpdated {
                            position,
                            fft: fft_data,
                            pcm: pcm_data,
                        });
                    }

                    if let Some(duration) = playback.duration() {
                        let threshold = Duration::from_millis(100);
                        if position + threshold >= duration {
                            info!("Трек завершен");
                            let _ = event_tx.send(AudioEvent::TrackFinished);
                            state.write().unwrap().playback_state = PlaybackState::Stopped;
                            playback.stop();
                        }
                    }
                }
            }

            // Обработка команд
            match command_rx.recv_timeout(Duration::from_millis(10)) {
                Ok(command) => {
                    match command {
                        AudioCommand::PlayHLS { track, source } => {
                            info!("Команда PlayHLS: сброс и переход в Loading");
                            playback.stop();
                            {
                                let mut s = state.write().unwrap();
                                s.playback_state = PlaybackState::Loading;
                                s.position = Duration::ZERO;
                                s.duration = Duration::ZERO;
                                s.buffered_ranges = Vec::new();
                                s.current_track = Some(track.clone());
                            }
                            let _ = event_tx.send(AudioEvent::LoadingStarted {
                                track: track.clone(),
                            });

                            info!("Начало подготовки HLS для трека {}", track.id);
                            let result = playback.play_hls(source.clone());
                            match result {
                                Ok(_) => {
                                    let duration = playback.duration().unwrap_or(Duration::ZERO);

                                    {
                                        let mut s = state.write().unwrap();
                                        s.playback_state = PlaybackState::Playing;
                                        s.position = Duration::ZERO;
                                        s.duration = duration;
                                        s.current_track = Some(track.clone());
                                    }

                                    if duration > Duration::ZERO {
                                        let _ =
                                            event_tx.send(AudioEvent::DurationChanged { duration });
                                    }
                                    last_position_update = std::time::Instant::now();

                                    let _ = event_tx.send(AudioEvent::PlaybackStarted {
                                        track: track.clone(),
                                    });
                                    info!(
                                        "HLS воспроизведение началось, длительность: {:?}",
                                        duration
                                    );
                                }
                                Err(e) => {
                                    warn!("Ошибка HLS воспроизведения: {}", e);
                                    {
                                        let mut s = state.write().unwrap();
                                        s.playback_state = PlaybackState::Stopped;
                                    }
                                    let _ = event_tx.send(AudioEvent::Error {
                                        message: format!("Ошибка HLS воспроизведения: {}", e),
                                    });
                                }
                            }
                        }
                        AudioCommand::Play(track) => {
                            info!("Команда Play: сброс и переход в Loading");
                            playback.stop();
                            {
                                let mut s = state.write().unwrap();
                                s.playback_state = PlaybackState::Loading;
                                s.position = Duration::ZERO;
                                s.duration = Duration::ZERO;
                                s.buffered_ranges = Vec::new();
                                s.current_track = Some(track.clone());
                            }
                            let _ = event_tx.send(AudioEvent::LoadingStarted {
                                track: track.clone(),
                            });

                            info!("Начало воспроизведения трека {}", track.id);
                            let result = playback.play(&track.source);
                            match result {
                                Ok(_) => {
                                    // Получаем длительность
                                    let duration = playback.duration().unwrap_or(Duration::ZERO);

                                    // Обновляем состояние
                                    {
                                        let mut s = state.write().unwrap();
                                        s.playback_state = PlaybackState::Playing;
                                        s.position = Duration::ZERO;
                                        s.duration = duration;
                                        s.current_track = Some(track.clone());
                                    }

                                    // Отправляем события
                                    if duration > Duration::ZERO {
                                        let _ =
                                            event_tx.send(AudioEvent::DurationChanged { duration });
                                    }
                                    last_position_update = std::time::Instant::now();

                                    // Проверяем завершение трека
                                    let _ = event_tx.send(AudioEvent::PlaybackStarted {
                                        track: track.clone(),
                                    });
                                    info!("Воспроизведение началось, длительность: {:?}", duration);
                                }
                                Err(e) => {
                                    warn!("Ошибка воспроизведения: {}", e);
                                    {
                                        let mut s = state.write().unwrap();
                                        s.playback_state = PlaybackState::Stopped;
                                    }
                                    let _ = event_tx.send(AudioEvent::Error {
                                        message: format!("Ошибка воспроизведения: {}", e),
                                    });
                                }
                            }
                        }
                        AudioCommand::PlaySource { track, source } => {
                            info!("Команда PlaySource: сброс и переход в Loading");
                            playback.stop();
                            {
                                let mut s = state.write().unwrap();
                                s.playback_state = PlaybackState::Loading;
                                s.position = Duration::ZERO;
                                s.duration = Duration::ZERO;
                                s.buffered_ranges = Vec::new();
                                s.current_track = Some(track.clone());
                            }
                            let _ = event_tx.send(AudioEvent::LoadingStarted {
                                track: track.clone(),
                            });

                            info!("Начало воспроизведения из источника для трека {}", track.id);
                            let result = playback.play_audio_source(source);

                            match result {
                                Ok(_) => {
                                    // Получаем длительность
                                    let duration = playback.duration().unwrap_or(Duration::ZERO);

                                    // Обновляем состояние
                                    {
                                        let mut s = state.write().unwrap();
                                        s.playback_state = PlaybackState::Playing;
                                        s.position = Duration::ZERO;
                                        s.duration = duration;
                                        s.current_track = Some(track.clone());
                                    }

                                    // Отправляем события
                                    if duration > Duration::ZERO {
                                        let _ =
                                            event_tx.send(AudioEvent::DurationChanged { duration });
                                    }
                                    last_position_update = std::time::Instant::now();

                                    // Проверяем завершение трека
                                    let _ = event_tx.send(AudioEvent::PlaybackStarted {
                                        track: track.clone(),
                                    });
                                    info!("Воспроизведение началось, длительность: {:?}", duration);
                                }
                                Err(e) => {
                                    warn!("Ошибка воспроизведения: {}", e);
                                    {
                                        let mut s = state.write().unwrap();
                                        s.playback_state = PlaybackState::Stopped;
                                    }
                                    let _ = event_tx.send(AudioEvent::Error {
                                        message: format!("Ошибка воспроизведения: {}", e),
                                    });
                                }
                            }
                        }

                        AudioCommand::SetLoading(track) => {
                            info!("Команда SetLoading: принудительная остановка и смена контекста");
                            playback.stop();
                            {
                                let mut s = state.write().unwrap();
                                s.playback_state = PlaybackState::Loading;
                                s.position = Duration::ZERO;
                                s.duration = Duration::ZERO;
                                s.buffered_ranges = Vec::new();
                                s.current_track = Some(track.clone());
                            }
                            let _ = event_tx.send(AudioEvent::LoadingStarted {
                                track: track.clone(),
                            });
                        }

                        AudioCommand::Pause => {
                            info!("Пауза");
                            playback.pause();
                            state.write().unwrap().playback_state = PlaybackState::Paused;
                            let _ = event_tx.send(AudioEvent::PlaybackPaused);
                        }

                        AudioCommand::Resume => {
                            info!("Возобновление");
                            playback.resume();
                            state.write().unwrap().playback_state = PlaybackState::Playing;

                            let _ = event_tx.send(AudioEvent::PlaybackResumed);
                        }

                        AudioCommand::Stop => {
                            info!("Остановка");
                            playback.stop();

                            {
                                let mut s = state.write().unwrap();
                                s.playback_state = PlaybackState::Stopped;
                                s.position = Duration::ZERO;
                                s.current_track = None;
                            }

                            let _ = event_tx.send(AudioEvent::PlaybackStopped);
                        }

                        AudioCommand::Seek(position) => {
                            info!("Перемотка на {:?}", position);

                            match playback.seek(position) {
                                Ok(_) => {
                                    state.write().unwrap().position = position;
                                    let _ = event_tx.send(AudioEvent::PositionUpdated { position, fft: None, pcm: None });
                                }
                                Err(AudioError::InProgress) => {
                                    info!("Ошибка INPROGRESS:");
                                    state.write().unwrap().position = position;
                                    let _ = event_tx.send(AudioEvent::PositionUpdated { position, fft: None, pcm: None });
                                }
                                Err(AudioError::OutsideOfBuffer) => {
                                    info!("Ошибка OutsideOfBuffer:");
                                    let _ = event_tx.send(AudioEvent::Error {
                                        message: format!(
                                            "Поиск вне буфера не поддерживается для opus"
                                        ),
                                    });
                                }
                                Err(e) => {
                                    warn!("Ошибка перемотки: {}", e);
                                }
                            }
                        }

                        AudioCommand::SetVolume(volume) => {
                            info!("Установка громкости: {}", volume);
                            state.write().unwrap().volume = volume;
                            playback.set_volume(volume);
                            let _ = event_tx.send(AudioEvent::VolumeChanged { volume });
                        }

                        AudioCommand::Shutdown => {
                            info!("Завершение работы аудио потока");
                            return;
                        }
                    }
                }
                Err(channel::RecvTimeoutError::Timeout) => {
                    // Нет команд - продолжаем работу
                }
                Err(channel::RecvTimeoutError::Disconnected) => {
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
