use super::eq::EqSettings;
use super::playback::PlaybackEngine;
use super::sync::{read_recover, write_recover};
use super::types::{
    AudioCommand, AudioEvent, AudioState, BufferedRange, PlaybackState, Track, TrackSource,
};
use crate::audio::types::MediaSource;
use crate::error::AudioError;
use crossbeam::channel;
use crossbeam::channel::{Receiver, Sender};
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;
use streaming::multi_downloader::source::HlsSource;
use tracing::{info, warn};

const COMMAND_BUFFER_SIZE: usize = 32;
const EVENT_BUFFER_SIZE: usize = 128;

pub struct AudioEngine {
    command_tx: Sender<AudioCommand>,
    event_tx: Sender<AudioEvent>,
    event_rx: Receiver<AudioEvent>,
    state: Arc<RwLock<AudioState>>,
}

impl AudioEngine {
    pub fn new() -> Result<Self, AudioError> {
        info!("Initializing audio engine");

        let (command_tx, command_rx) = crossbeam::channel::bounded(COMMAND_BUFFER_SIZE);
        let (event_tx, event_rx) = crossbeam::channel::bounded(EVENT_BUFFER_SIZE);

        let state = Arc::new(RwLock::new(AudioState::default()));

        // The audio thread runs detached for the lifetime of the process.
        Self::spawn_audio_thread(command_rx, event_tx.clone(), Arc::clone(&state))?;

        Ok(Self {
            command_tx,
            event_tx,
            event_rx,
            state,
        })
    }

    pub fn play(&self, track: Track) {
        info!("Command: play track {}", track.id);
        let _ = self.command_tx.send(AudioCommand::Play(track));
    }
    pub fn play_source(&self, track: Track, source: Box<dyn MediaSource>) {
        info!("Command: play track ");
        let _ = self
            .command_tx
            .send(AudioCommand::PlaySource { track, source });
    }
    pub fn play_hls(&self, track: Track, source: HlsSource) {
        info!("Command: play track ");
        let _ = self
            .command_tx
            .send(AudioCommand::PlayHLS { track, source });
    }

    pub fn preload(&self, track: Track) {
        let _ = self.command_tx.send(AudioCommand::Preload(track));
    }

    pub fn preload_hls(&self, track: Track, source: HlsSource) {
        let _ = self
            .command_tx
            .send(AudioCommand::PreloadHLS { track, source });
    }

    pub fn preload_source(&self, track: Track, source: Box<dyn MediaSource>) {
        let _ = self
            .command_tx
            .send(AudioCommand::PreloadSource { track, source });
    }

    pub fn set_loading(&self, track: Track) {
        let _ = self.command_tx.send(AudioCommand::SetLoading(track));
    }

    pub fn pause(&self) {
        info!("Command: pause");
        let _ = self.command_tx.send(AudioCommand::Pause);
    }

    pub fn resume(&self) {
        info!("Command: resume");
        let _ = self.command_tx.send(AudioCommand::Resume);
    }

    pub fn stop(&self) {
        info!("Command: stop");
        let _ = self.command_tx.send(AudioCommand::Stop);
    }

    pub fn seek(&self, position: Duration) {
        info!("Command: seek to {:?}", position);
        let _ = self.command_tx.send(AudioCommand::Seek(position));
    }

    pub fn set_volume(&self, volume: f32) {
        let _ = self.command_tx.send(AudioCommand::SetVolume(volume));
    }

    pub fn set_fade(&self, fade_in_ms: u32, fade_out_ms: u32) {
        let _ = self.command_tx.send(AudioCommand::SetFade {
            fade_in_ms,
            fade_out_ms,
        });
    }

    pub fn set_eq(&self, settings: EqSettings) {
        let _ = self.command_tx.send(AudioCommand::SetEq(settings));
    }

    pub fn get_eq(&self) -> EqSettings {
        read_recover(&self.state).eq_settings.clone()
    }

    pub fn state(&self) -> AudioState {
        read_recover(&self.state).clone()
    }

    pub fn poll_event(&self) -> Option<AudioEvent> {
        self.event_rx.try_recv().ok()
    }

    pub fn poll_all_events(&self) -> Vec<AudioEvent> {
        self.event_rx.try_iter().collect()
    }

    pub fn push_event(&self, event: AudioEvent) {
        let _ = self.event_tx.send(event);
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
                info!("Audio thread started");
                Self::audio_thread_loop(command_rx, event_tx, state);
                info!("Audio thread finished");
            })
            .map_err(|e| {
                AudioError::StreamBuildError(format!("Failed to create audio stream: {}", e))
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
                warn!("Failed to create playback engine: {}", e);
                return;
            }
        };
        let mut last_position_update = std::time::Instant::now();
        let mut preload_triggered = false;
        let fft_size = 1024;

        loop {
            if playback.needs_recreation() {
                if let Err(e) = playback.recreate_stream() {
                    warn!("Failed to automatically recreate audio stream: {}", e);
                    let _ = event_tx.send(AudioEvent::Error {
                        message: format!("Audio device error: {}", e),
                    });
                }
            }

            if playback.track_switched() {
                info!("Detected gapless track switch in the audio stream");
                let mut s = write_recover(&state);
                let next_track = s.next_track.take();
                if let Some(track) = next_track {
                    s.current_track = Some(track.clone());
                    s.position = Duration::ZERO;
                    s.duration = playback.duration().unwrap_or(Duration::ZERO);
                    preload_triggered = false;
                    // GaplessSwitched instead of PlaybackStarted so the queue cursor is advanced
                    let _ = event_tx.send(AudioEvent::GaplessSwitched { track });
                    if s.duration > Duration::ZERO {
                        let _ = event_tx.send(AudioEvent::DurationChanged {
                            duration: s.duration,
                        });
                    }
                }
            }

            if playback.is_playing() || !playback.buffered_ranges().is_empty() {
                let now = std::time::Instant::now();
                if now.duration_since(last_position_update) >= Duration::from_millis(100) {
                    last_position_update = now;

                    let position = playback.position();
                    let duration = playback.duration().unwrap_or(Duration::ZERO);

                    if !preload_triggered && duration > Duration::from_secs(15) {
                        let remaining = duration.saturating_sub(position);
                        if remaining <= Duration::from_secs(10) {
                            preload_triggered = true;
                            let current_track_id = read_recover(&state)
                                .current_track
                                .as_ref()
                                .map(|t| t.id)
                                .unwrap_or(0);
                            info!("Preload threshold reached (10s left), sending request");
                            let _ =
                                event_tx.send(AudioEvent::PreloadNextTrack { current_track_id });
                        }
                    }

                    let ranges = playback.buffered_ranges();
                    let buffered_ranges: Vec<BufferedRange> = ranges
                        .into_iter()
                        .map(|(start, end)| BufferedRange {
                            start: start as f32,
                            end: end as f32,
                        })
                        .collect();

                    let waveforms = playback.waveforms();

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

                            pcm_data = Some(
                                mono_samples
                                    .iter()
                                    .map(|&s| ((s * 127.0) + 128.0).clamp(0.0, 255.0) as u8)
                                    .collect(),
                            );
                        }
                    }

                    {
                        let mut s = write_recover(&state);
                        s.position = position;

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
                            fft: None,
                            pcm: pcm_data,
                        });
                    }

                    if let Some(duration) = playback.duration() {
                        let threshold = Duration::from_millis(100);
                        if position + threshold >= duration {
                            // If the next track is already preloaded, let the gapless decoder
                            // swap happen naturally inside the audio callback instead of killing
                            // the stream early.
                            let has_next = read_recover(&state).next_track.is_some();
                            if !has_next {
                                info!("Track finished");
                                let _ = event_tx.send(AudioEvent::TrackFinished);
                                write_recover(&state).playback_state = PlaybackState::Stopped;
                                playback.stop();
                            }
                        }
                    }
                }
            }

            match command_rx.recv_timeout(Duration::from_millis(10)) {
                Ok(command) => match command {
                    AudioCommand::Preload(track) => {
                        info!("Preload command for track {}", track.id);
                        match track.source {
                            TrackSource::Local(ref p) => {
                                if let Err(e) = playback.preload_local(p) {
                                    warn!("Preload error: {}", e);
                                } else {
                                    write_recover(&state).next_track = Some(track);
                                }
                            }
                            _ => {
                                warn!("Preloading this source type is not supported yet")
                            }
                        }
                    }
                    AudioCommand::PreloadHLS { track, source } => {
                        info!("PreloadHLS command for track {}", track.id);
                        if let Err(e) = playback.preload_hls(source) {
                            warn!("HLS preload error: {}", e);
                        } else {
                            write_recover(&state).next_track = Some(track);
                        }
                    }
                    AudioCommand::PreloadSource { track, source } => {
                        info!("PreloadSource command for track {}", track.id);
                        if let Err(e) = playback.preload_audio_source(source) {
                            warn!("Source preload error: {}", e);
                        } else {
                            write_recover(&state).next_track = Some(track);
                        }
                    }
                    AudioCommand::PlayHLS { track, source } => {
                        info!("PlayHLS command");
                        preload_triggered = false;

                        let use_preloaded = {
                            let s = read_recover(&state);
                            s.next_track.as_ref().map(|t| t.id) == Some(track.id)
                        };

                        let result = if use_preloaded {
                            info!("Using preloaded HLS track {}", track.id);
                            let ok = playback.use_preloaded();
                            write_recover(&state).next_track = None;
                            ok
                        } else {
                            {
                                let mut s = write_recover(&state);
                                s.playback_state = PlaybackState::Loading;
                                s.position = Duration::ZERO;
                                s.duration = Duration::ZERO;
                                s.buffered_ranges = Vec::new();
                                s.current_track = Some(track.clone());
                                s.next_track = None;
                            }
                            let _ = event_tx.send(AudioEvent::LoadingStarted {
                                track: track.clone(),
                            });
                            playback.stop();
                            info!("Preparing HLS for track {}", track.id);
                            playback.play_hls(source.clone())
                        };

                        match result {
                            Ok(_) => {
                                let duration = playback.duration().unwrap_or(Duration::ZERO);

                                {
                                    let mut s = write_recover(&state);
                                    s.playback_state = PlaybackState::Playing;
                                    s.position = Duration::ZERO;
                                    s.duration = duration;
                                    s.current_track = Some(track.clone());
                                    s.next_track = None;
                                }

                                if duration > Duration::ZERO {
                                    let _ = event_tx.send(AudioEvent::DurationChanged { duration });
                                }
                                last_position_update = std::time::Instant::now();

                                let _ = event_tx.send(AudioEvent::PlaybackStarted {
                                    track: track.clone(),
                                });
                            }
                            Err(e) => {
                                warn!("HLS playback error: {}", e);
                                {
                                    let mut s = write_recover(&state);
                                    s.playback_state = PlaybackState::Stopped;
                                }
                                let _ = event_tx.send(AudioEvent::Error {
                                    message: format!("HLS playback error: {}", e),
                                });
                            }
                        }
                    }
                    AudioCommand::Play(track) => {
                        info!("Play command");
                        preload_triggered = false;

                        let use_preloaded = {
                            let s = read_recover(&state);
                            s.next_track.as_ref().map(|t| t.id) == Some(track.id)
                        };

                        let result = if use_preloaded {
                            info!("Using preloaded local track {}", track.id);
                            playback.use_preloaded()
                        } else {
                            {
                                let mut s = write_recover(&state);
                                s.playback_state = PlaybackState::Loading;
                                s.position = Duration::ZERO;
                                s.duration = Duration::ZERO;
                                s.buffered_ranges = Vec::new();
                                s.current_track = Some(track.clone());
                                s.next_track = None;
                            }
                            let _ = event_tx.send(AudioEvent::LoadingStarted {
                                track: track.clone(),
                            });
                            playback.stop();
                            info!("Starting playback of track {}", track.id);
                            playback.play(&track.source)
                        };

                        match result {
                            Ok(_) => {
                                let duration = playback.duration().unwrap_or(Duration::ZERO);

                                {
                                    let mut s = write_recover(&state);
                                    s.playback_state = PlaybackState::Playing;
                                    s.position = Duration::ZERO;
                                    s.duration = duration;
                                    s.current_track = Some(track.clone());
                                    s.next_track = None;
                                }

                                if duration > Duration::ZERO {
                                    let _ = event_tx.send(AudioEvent::DurationChanged { duration });
                                }
                                last_position_update = std::time::Instant::now();

                                let _ = event_tx.send(AudioEvent::PlaybackStarted {
                                    track: track.clone(),
                                });
                            }
                            Err(e) => {
                                warn!("Playback error: {}", e);
                                {
                                    let mut s = write_recover(&state);
                                    s.playback_state = PlaybackState::Stopped;
                                }
                                let _ = event_tx.send(AudioEvent::Error {
                                    message: format!("Playback error: {}", e),
                                });
                            }
                        }
                    }
                    AudioCommand::PlaySource { track, source } => {
                        info!("PlaySource command");
                        preload_triggered = false;

                        let use_preloaded = {
                            let s = read_recover(&state);
                            s.next_track.as_ref().map(|t| t.id) == Some(track.id)
                        };

                        let result = if use_preloaded {
                            info!("Using preloaded Source track {}", track.id);
                            playback.use_preloaded()
                        } else {
                            {
                                let mut s = write_recover(&state);
                                s.playback_state = PlaybackState::Loading;
                                s.position = Duration::ZERO;
                                s.duration = Duration::ZERO;
                                s.buffered_ranges = Vec::new();
                                s.current_track = Some(track.clone());
                                s.next_track = None;
                            }
                            let _ = event_tx.send(AudioEvent::LoadingStarted {
                                track: track.clone(),
                            });
                            playback.stop();
                            info!("Starting playback from source for track {}", track.id);
                            playback.play_audio_source(source)
                        };

                        match result {
                            Ok(_) => {
                                let duration = playback.duration().unwrap_or(Duration::ZERO);

                                {
                                    let mut s = write_recover(&state);
                                    s.playback_state = PlaybackState::Playing;
                                    s.position = Duration::ZERO;
                                    s.duration = duration;
                                    s.current_track = Some(track.clone());
                                    s.next_track = None;
                                }

                                if duration > Duration::ZERO {
                                    let _ = event_tx.send(AudioEvent::DurationChanged { duration });
                                }
                                last_position_update = std::time::Instant::now();

                                let _ = event_tx.send(AudioEvent::PlaybackStarted {
                                    track: track.clone(),
                                });
                            }
                            Err(e) => {
                                warn!("Playback error: {}", e);
                                {
                                    let mut s = write_recover(&state);
                                    s.playback_state = PlaybackState::Stopped;
                                }
                                let _ = event_tx.send(AudioEvent::Error {
                                    message: format!("Playback error: {}", e),
                                });
                            }
                        }
                    }

                    AudioCommand::SetLoading(track) => {
                        info!("SetLoading command");
                        playback.stop();
                        {
                            let mut s = write_recover(&state);
                            s.playback_state = PlaybackState::Loading;
                            s.position = Duration::ZERO;
                            s.duration = Duration::ZERO;
                            s.buffered_ranges = Vec::new();
                            s.current_track = Some(track.clone());
                            s.next_track = None;
                        }
                        let _ = event_tx.send(AudioEvent::LoadingStarted {
                            track: track.clone(),
                        });
                    }

                    AudioCommand::Pause => {
                        info!("Pause");
                        playback.pause();
                        write_recover(&state).playback_state = PlaybackState::Paused;
                        let _ = event_tx.send(AudioEvent::PlaybackPaused);
                    }

                    AudioCommand::Resume => {
                        info!("Resume");
                        playback.resume();
                        write_recover(&state).playback_state = PlaybackState::Playing;

                        let _ = event_tx.send(AudioEvent::PlaybackResumed);
                    }

                    AudioCommand::Stop => {
                        info!("Stop");
                        playback.stop();

                        {
                            let mut s = write_recover(&state);
                            s.playback_state = PlaybackState::Stopped;
                            s.position = Duration::ZERO;
                            s.current_track = None;
                            s.next_track = None;
                        }

                        let _ = event_tx.send(AudioEvent::PlaybackStopped);
                    }

                    AudioCommand::Seek(position) => {
                        info!("Seek to {:?}", position);

                        match playback.seek(position) {
                            Ok(_) => {
                                write_recover(&state).position = position;
                                let _ = event_tx.send(AudioEvent::PositionUpdated {
                                    position,
                                    fft: None,
                                    pcm: None,
                                });
                            }
                            Err(AudioError::InProgress) => {
                                info!("INPROGRESS error:");
                                write_recover(&state).position = position;
                                let _ = event_tx.send(AudioEvent::PositionUpdated {
                                    position,
                                    fft: None,
                                    pcm: None,
                                });
                            }
                            Err(AudioError::OutsideOfBuffer) => {
                                info!("OutsideOfBuffer error:");
                                let _ = event_tx.send(AudioEvent::Error {
                                    message: "Seeking outside the buffer is not supported for opus"
                                        .to_string(),
                                });
                            }
                            Err(e) => {
                                warn!("Seek error: {}", e);
                            }
                        }
                    }

                    AudioCommand::SetVolume(volume) => {
                        info!("Setting volume: {}", volume);
                        write_recover(&state).volume = volume;
                        playback.set_volume(volume);
                        let _ = event_tx.send(AudioEvent::VolumeChanged { volume });
                    }

                    AudioCommand::SetEq(settings) => {
                        info!("Updating EQ settings, enabled={}", settings.enabled);
                        write_recover(&state).eq_settings = settings.clone();
                        playback.set_eq(settings);
                    }

                    AudioCommand::SetFade {
                        fade_in_ms,
                        fade_out_ms,
                    } => {
                        info!(
                            "Updating fade settings: in={}ms, out={}ms",
                            fade_in_ms, fade_out_ms
                        );
                        playback.set_fade(fade_in_ms, fade_out_ms);
                    }

                    AudioCommand::Shutdown => {
                        info!("Shutting down audio thread");
                        return;
                    }
                },
                Err(channel::RecvTimeoutError::Timeout) => {}
                Err(channel::RecvTimeoutError::Disconnected) => {
                    warn!("Command channel disconnected");
                    break;
                }
            }
        }
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        info!("Shutting down audio engine");
        let _ = self.command_tx.send(AudioCommand::Shutdown);
    }
}
