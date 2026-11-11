use super::decoder::{AudioDecoder, DecoderType};
use super::eq::{EqProcessor, EqSettings};
use super::hls_decoder::HlsAudioDecoder;
use super::sync::lock_recover;
use super::volume::{default_volume, effective_volume, IS_ANDROID};
use crate::audio::types::{MediaSource, SampleRate as MSR, TrackSource};
use crate::error::AudioError;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use streaming::multi_downloader::source::HlsSource;
use tracing::{info, warn};

mod hls_reader;
pub use hls_reader::HlsReaderAdapter;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// How long `seek` blocks waiting for decoded data at the new position.
const SEEK_DATA_TIMEOUT: Duration = Duration::from_secs(5);

pub struct PlaybackEngine {
    decoder: Arc<Mutex<Option<DecoderType>>>,
    next_decoder: Arc<Mutex<Option<DecoderType>>>,
    track_switched: Arc<AtomicBool>,
    stream: Option<Stream>,
    device: Device,
    config: StreamConfig,
    is_playing: Arc<Mutex<bool>>,
    current_position: Arc<Mutex<Duration>>,
    duration: Arc<Mutex<Option<Duration>>>,
    sample_rate: Arc<Mutex<u32>>,
    samples_played: Arc<Mutex<u64>>,
    sample_buffer: Arc<Mutex<Vec<f32>>>,
    buffer_position: Arc<Mutex<usize>>,
    volume: Arc<Mutex<f32>>,
    recent_samples: Arc<Mutex<Vec<f32>>>,
    needs_recreation: Arc<AtomicBool>,
    eq_settings: Arc<Mutex<EqSettings>>,
    eq_dirty: Arc<AtomicBool>,
    /// Total samples for fade-in after a track switch (0 = disabled).
    fade_in_samples: Arc<AtomicU64>,
    /// Total samples for fade-out before track end (0 = disabled).
    fade_out_samples: Arc<AtomicU64>,
    /// Set to true by play() calls so the callback initialises fade_in_remaining.
    fade_in_pending: Arc<AtomicBool>,
}

impl PlaybackEngine {
    pub fn new() -> Result<Self, AudioError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| AudioError::OutputError("No audio device found".to_string()))?;

        info!(
            "Audio device: {}",
            device.name().unwrap_or_else(|_| "Unknown".to_string())
        );

        let config = device
            .default_output_config()
            .map_err(|e| AudioError::OutputError(format!("Failed to get configuration: {}", e)))?
            .into();

        Ok(Self {
            decoder: Arc::new(Mutex::new(None)),
            next_decoder: Arc::new(Mutex::new(None)),
            track_switched: Arc::new(AtomicBool::new(false)),
            stream: None,
            device,
            config,
            is_playing: Arc::new(Mutex::new(false)),
            current_position: Arc::new(Mutex::new(Duration::ZERO)),
            duration: Arc::new(Mutex::new(None)),
            sample_rate: Arc::new(Mutex::new(MSR::LOW.as_u32())),
            samples_played: Arc::new(Mutex::new(0)),
            sample_buffer: Arc::new(Mutex::new(Vec::new())),
            buffer_position: Arc::new(Mutex::new(0)),
            volume: Arc::new(Mutex::new(default_volume())),
            recent_samples: Arc::new(Mutex::new(Vec::with_capacity(8192))),
            needs_recreation: Arc::new(AtomicBool::new(false)),
            eq_settings: Arc::new(Mutex::new(EqSettings::default())),
            eq_dirty: Arc::new(AtomicBool::new(false)),
            fade_in_samples: Arc::new(AtomicU64::new(0)),
            fade_out_samples: Arc::new(AtomicU64::new(0)),
            fade_in_pending: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn play(&mut self, path: &TrackSource) -> Result<(), AudioError> {
        match path {
            TrackSource::Local(p) => self.play_local(p),
            _ => Err(AudioError::StreamUnavailable),
        }
    }

    pub fn play_audio_source(
        &mut self,
        audio_source: Box<dyn MediaSource>,
    ) -> Result<(), AudioError> {
        info!("Loading audio from AudioSource");

        let output_sample_rate = self.config.sample_rate.0;
        let decoder = AudioDecoder::from_reader(audio_source, None, output_sample_rate)?;
        let duration = decoder.duration().map(Duration::from_secs_f64);
        let sample_rate = decoder.sample_rate();
        let channels = decoder.channels();

        info!(
            "Sample rate: {}, channels: {}, duration: {:?}",
            sample_rate, channels, duration
        );

        *lock_recover(&self.duration) = duration;
        *lock_recover(&self.decoder) = Some(DecoderType::from_standard(decoder));
        *lock_recover(&self.is_playing) = true;
        *lock_recover(&self.current_position) = Duration::ZERO;
        *lock_recover(&self.sample_rate) = sample_rate;
        *lock_recover(&self.samples_played) = 0;
        self.fade_in_pending.store(true, Ordering::Relaxed);

        lock_recover(&self.sample_buffer).clear();
        *lock_recover(&self.buffer_position) = 0;

        self.create_stream()?;

        Ok(())
    }

    pub fn play_hls(&mut self, hls_source: HlsSource) -> Result<(), AudioError> {
        info!("Loading HLS stream");

        let output_sample_rate = self.config.sample_rate.0;
        let decoder = HlsAudioDecoder::new(hls_source, output_sample_rate)?;
        let duration = decoder.duration().map(Duration::from_secs_f64);
        let sample_rate = decoder.sample_rate();
        let channels = decoder.channels();

        info!(
            "HLS: sample_rate={}, channels={}, duration={:?}",
            sample_rate, channels, duration
        );

        *lock_recover(&self.duration) = duration;
        *lock_recover(&self.decoder) = Some(DecoderType::from_hls(decoder));
        *lock_recover(&self.is_playing) = true;
        *lock_recover(&self.current_position) = Duration::ZERO;
        *lock_recover(&self.sample_rate) = sample_rate;
        *lock_recover(&self.samples_played) = 0;
        self.fade_in_pending.store(true, Ordering::Relaxed);

        lock_recover(&self.sample_buffer).clear();
        *lock_recover(&self.buffer_position) = 0;

        self.create_stream()?;

        Ok(())
    }

    pub fn preload_local(&mut self, path: &std::path::Path) -> Result<(), AudioError> {
        info!("[PlaybackEngine] Starting local file preload: {:?}", path);
        let output_sample_rate = self.config.sample_rate.0;
        info!(
            "[PlaybackEngine] Initializing decoder for local file, output_sample_rate: {}",
            output_sample_rate
        );
        let decoder = AudioDecoder::new(path, output_sample_rate)?;
        info!("[PlaybackEngine] Decoder created successfully. Storing in next_decoder.");
        *lock_recover(&self.next_decoder) = Some(DecoderType::from_standard(decoder));
        info!("[PlaybackEngine] Local file preload finished.");
        Ok(())
    }

    pub fn preload_hls(&mut self, source: HlsSource) -> Result<(), AudioError> {
        info!("[PlaybackEngine] Starting HLS stream preload");
        let output_sample_rate = self.config.sample_rate.0;
        info!(
            "[PlaybackEngine] Initializing HLS decoder, output_sample_rate: {}",
            output_sample_rate
        );
        let decoder = HlsAudioDecoder::new(source, output_sample_rate)?;
        info!("[PlaybackEngine] HLS decoder created successfully. Storing in next_decoder.");
        *lock_recover(&self.next_decoder) = Some(DecoderType::from_hls(decoder));
        info!("[PlaybackEngine] HLS stream preload finished.");
        Ok(())
    }

    pub fn preload_audio_source(&mut self, source: Box<dyn MediaSource>) -> Result<(), AudioError> {
        info!("[PlaybackEngine] Starting preload from AudioSource");
        let output_sample_rate = self.config.sample_rate.0;
        info!(
            "[PlaybackEngine] Initializing decoder for AudioSource, output_sample_rate: {}",
            output_sample_rate
        );
        let decoder = AudioDecoder::from_reader(source, None, output_sample_rate)?;
        info!(
            "[PlaybackEngine] Decoder for AudioSource created successfully. Storing in next_decoder."
        );
        *lock_recover(&self.next_decoder) = Some(DecoderType::from_standard(decoder));
        info!("[PlaybackEngine] Preload from AudioSource finished.");
        Ok(())
    }

    pub fn use_preloaded(&mut self) -> Result<(), AudioError> {
        let next = {
            let mut next_guard = lock_recover(&self.next_decoder);
            next_guard.take()
        };

        if let Some(next) = next {
            info!("[PlaybackEngine] Using preloaded track");
            let duration = next.duration().map(Duration::from_secs_f64);
            let sample_rate = next.sample_rate();

            *lock_recover(&self.duration) = duration;
            *lock_recover(&self.decoder) = Some(next);
            *lock_recover(&self.is_playing) = true;
            *lock_recover(&self.current_position) = Duration::ZERO;
            *lock_recover(&self.sample_rate) = sample_rate;
            *lock_recover(&self.samples_played) = 0;
            self.fade_in_pending.store(true, Ordering::Relaxed);

            lock_recover(&self.sample_buffer).clear();
            *lock_recover(&self.buffer_position) = 0;

            self.create_stream()?;
            Ok(())
        } else {
            Err(AudioError::DecodingError(
                "No preloaded track found".to_string(),
            ))
        }
    }

    fn play_local(&mut self, path: &std::path::Path) -> Result<(), AudioError> {
        info!("Loading file: {:?}", path);

        let output_sample_rate = self.config.sample_rate.0;
        let decoder = AudioDecoder::new(path, output_sample_rate)?;
        let duration = decoder.duration().map(Duration::from_secs_f64);
        let sample_rate = decoder.sample_rate();
        let channels = decoder.channels();

        info!(
            "Sample rate: {}, channels: {}, duration: {:?}",
            sample_rate, channels, duration
        );

        *lock_recover(&self.duration) = duration;
        *lock_recover(&self.decoder) = Some(DecoderType::from_standard(decoder));
        *lock_recover(&self.is_playing) = true;
        *lock_recover(&self.current_position) = Duration::ZERO;
        *lock_recover(&self.sample_rate) = sample_rate;
        *lock_recover(&self.samples_played) = 0;
        self.fade_in_pending.store(true, Ordering::Relaxed);

        lock_recover(&self.sample_buffer).clear();
        *lock_recover(&self.buffer_position) = 0;

        self.create_stream()?;

        Ok(())
    }

    fn create_stream(&mut self) -> Result<(), AudioError> {
        if self.stream.is_some() {
            return Ok(());
        }

        let decoder = Arc::clone(&self.decoder);
        let next_decoder = Arc::clone(&self.next_decoder);
        let track_switched = Arc::clone(&self.track_switched);
        let is_playing = Arc::clone(&self.is_playing);
        let samples_played = Arc::clone(&self.samples_played);
        let current_position = Arc::clone(&self.current_position);
        let duration_arc = Arc::clone(&self.duration);
        let sample_rate_arc = Arc::clone(&self.sample_rate);
        let sample_buffer = Arc::clone(&self.sample_buffer);
        let buffer_position = Arc::clone(&self.buffer_position);
        let volume = Arc::clone(&self.volume);
        let recent_samples = Arc::clone(&self.recent_samples);
        let needs_recreation = Arc::clone(&self.needs_recreation);
        let fade_in_smp = Arc::clone(&self.fade_in_samples);
        let fade_out_smp = Arc::clone(&self.fade_out_samples);
        let fade_in_pending_arc = Arc::clone(&self.fade_in_pending);
        let eq_settings = Arc::clone(&self.eq_settings);
        let eq_dirty = Arc::clone(&self.eq_dirty);

        let config = self.config.clone();
        let sample_rate = config.sample_rate.0;
        let mut current_ramp_gain = 0.0f32;
        // 10ms ramp
        let ramp_step = 1.0 / (sample_rate as f32 * 0.01);
        let mut fade_in_remaining: u64 = 0;
        // EQ processor lives entirely inside this closure; state (z1/z2) is never shared.
        let initial_eq = lock_recover(&eq_settings).clone();
        let mut eq_proc = EqProcessor::new(&initial_eq, sample_rate, config.channels as usize);

        let err_fn = move |err| {
            warn!("Stream error: {}", err);
            needs_recreation.store(true, Ordering::SeqCst);
        };

        let stream = self
            .device
            .build_output_stream(
                &config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let playing = *lock_recover(&is_playing);

                    // If not playing and already faded out, just fill with silence
                    if !playing && current_ramp_gain <= 0.0 {
                        data.fill(0.0);
                        return;
                    }

                    let mut output_pos = 0;
                    let channels = config.channels as usize;

                    while output_pos < data.len() {
                        let mut buffer = lock_recover(&sample_buffer);
                        let mut buf_pos = lock_recover(&buffer_position);

                        if *buf_pos >= buffer.len() {
                            drop(buf_pos);
                            drop(buffer);

                            let mut decoder_guard = lock_recover(&decoder);
                            if let Some(ref mut dec) = *decoder_guard {
                                match dec.decode_next_block() {
                                    Ok(Some(samples)) => {
                                        *lock_recover(&sample_buffer) = samples;
                                        *lock_recover(&buffer_position) = 0;
                                    }
                                    Ok(None) => {
                                        // Current track ended. Checking for a next one.
                                        let mut next_guard = lock_recover(&next_decoder);
                                        if let Some(next) = next_guard.take() {
                                            info!("Gapless transition to the next track");
                                            let next_duration =
                                                next.duration().map(Duration::from_secs_f64);
                                            let next_sr = next.sample_rate();

                                            *dec = next;
                                            *lock_recover(&duration_arc) = next_duration;
                                            *lock_recover(&samples_played) = 0;
                                            *lock_recover(&sample_rate_arc) = next_sr;
                                            track_switched.store(true, Ordering::SeqCst);
                                            fade_in_remaining = fade_in_smp.load(Ordering::Relaxed);
                                            // Continue the loop for the new decoder
                                            continue;
                                        } else {
                                            // No more tracks, but let it fade out if still has gain
                                            if current_ramp_gain > 0.0 {
                                                // We'll fill the rest with silence and let the ramp handle it
                                                data[output_pos..].fill(0.0);
                                            } else {
                                                data[output_pos..].fill(0.0);
                                                *lock_recover(&is_playing) = false;
                                            }
                                            break;
                                        }
                                    }
                                    Err(AudioError::InProgress) => {
                                        data[output_pos..].fill(0.0);
                                        return;
                                    }
                                    Err(e) => {
                                        warn!("Decoding error: {}", e);
                                        data[output_pos..].fill(0.0);
                                        return;
                                    }
                                }
                            } else {
                                data[output_pos..].fill(0.0);
                                return;
                            }

                            buffer = lock_recover(&sample_buffer);
                            buf_pos = lock_recover(&buffer_position);
                        }

                        let available = buffer.len() - *buf_pos;
                        let needed = data.len() - output_pos;
                        let to_copy = available.min(needed);

                        let input = &buffer[*buf_pos..*buf_pos + to_copy];

                        // Volume will be applied in the final ramp loop below
                        data[output_pos..output_pos + to_copy].copy_from_slice(input);

                        *buf_pos += to_copy;
                        output_pos += to_copy;

                        let num_frames = to_copy / channels;
                        *lock_recover(&samples_played) += num_frames as u64;
                        let position_secs =
                            *lock_recover(&samples_played) as f64 / config.sample_rate.0 as f64;
                        *lock_recover(&current_position) = Duration::from_secs_f64(position_secs);
                    }

                    // Rebuild EQ filters when settings changed from the command channel.
                    if eq_dirty.load(Ordering::Relaxed) {
                        eq_dirty.store(false, Ordering::Relaxed);
                        if let Ok(settings) = eq_settings.try_lock() {
                            eq_proc.rebuild(&settings, sample_rate);
                        }
                    }

                    // Apply volume ramp and master volume (per-frame to support fade)
                    let vol = *lock_recover(&volume);
                    let channels = config.channels as usize;
                    let num_frames = data.len() / channels;

                    // Initialise fade-in when a new track has just been started.
                    if fade_in_pending_arc.swap(false, Ordering::Relaxed) {
                        fade_in_remaining = fade_in_smp.load(Ordering::Relaxed);
                        info!(
                            "[fade] pending fired: fade_in_remaining={}",
                            fade_in_remaining
                        );
                    }

                    let fin_total = fade_in_smp.load(Ordering::Relaxed);
                    let fout_total = fade_out_smp.load(Ordering::Relaxed);
                    // frames_so_far already includes this block; subtract to get block start.
                    let block_start =
                        lock_recover(&samples_played).saturating_sub(num_frames as u64);
                    let dur_frames = lock_recover(&duration_arc)
                        .map(|d| (d.as_secs_f64() * sample_rate as f64) as u64);

                    for frame_idx in 0..num_frames {
                        if playing {
                            current_ramp_gain = (current_ramp_gain + ramp_step).min(1.0);
                        } else {
                            current_ramp_gain = (current_ramp_gain - ramp_step).max(0.0);
                        }

                        let fin_gain = if fin_total > 0 && fade_in_remaining > 0 {
                            let g = 1.0 - fade_in_remaining as f32 / fin_total as f32;
                            fade_in_remaining = fade_in_remaining.saturating_sub(1);
                            g.clamp(0.0, 1.0)
                        } else {
                            1.0
                        };

                        let fout_gain = if fout_total > 0 {
                            if let Some(dur) = dur_frames {
                                let abs_frame = block_start + frame_idx as u64;
                                let remaining = dur.saturating_sub(abs_frame);
                                if remaining < fout_total {
                                    (remaining as f32 / fout_total as f32).clamp(0.0, 1.0)
                                } else {
                                    1.0
                                }
                            } else {
                                1.0
                            }
                        } else {
                            1.0
                        };

                        let combined = current_ramp_gain * fin_gain * fout_gain * vol;
                        for c in 0..channels {
                            data[frame_idx * channels + c] *= combined;
                        }
                    }

                    // Apply EQ after volume ramp.
                    eq_proc.process(data, config.channels as usize);

                    {
                        if let Ok(mut recent) = recent_samples.try_lock() {
                            recent.extend_from_slice(data);
                            if recent.len() > 8192 {
                                let drain_len = recent.len() - 8192;
                                recent.drain(0..drain_len);
                            }
                        }
                    }
                },
                err_fn,
                None,
            )
            .map_err(|e| AudioError::OutputError(format!("Failed to create stream: {}", e)))?;

        stream
            .play()
            .map_err(|e| AudioError::OutputError(format!("Failed to start stream: {}", e)))?;

        self.stream = Some(stream);
        self.needs_recreation.store(false, Ordering::SeqCst);

        Ok(())
    }

    pub fn track_switched(&self) -> bool {
        self.track_switched.swap(false, Ordering::SeqCst)
    }

    pub fn needs_recreation(&self) -> bool {
        self.needs_recreation.load(Ordering::SeqCst)
    }

    pub fn recreate_stream(&mut self) -> Result<(), AudioError> {
        info!("Trying to recreate the stream (the device may have changed)...");

        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| AudioError::OutputError("No audio device found".to_string()))?;

        info!(
            "New audio device: {}",
            device.name().unwrap_or_else(|_| "Unknown".to_string())
        );

        let config = device
            .default_output_config()
            .map_err(|e| AudioError::OutputError(format!("Failed to get configuration: {}", e)))?
            .into();

        self.device = device;
        self.config = config;
        self.stream = None;

        let output_sample_rate = self.config.sample_rate.0;
        {
            let mut decoder_guard = lock_recover(&self.decoder);
            if let Some(ref mut dec) = *decoder_guard {
                dec.set_output_sample_rate(output_sample_rate);
            }
        }
        {
            let mut next_guard = lock_recover(&self.next_decoder);
            if let Some(ref mut dec) = *next_guard {
                dec.set_output_sample_rate(output_sample_rate);
            }
        }

        self.create_stream()?;

        if *lock_recover(&self.is_playing) {
            if let Some(ref stream) = self.stream {
                let _ = stream.play();
            }
        }

        info!("Stream recreated successfully");
        Ok(())
    }

    pub fn pause(&mut self) {
        *lock_recover(&self.is_playing) = false;
        if let Some(ref stream) = self.stream {
            let _ = stream.pause();
        }
    }

    pub fn resume(&mut self) {
        *lock_recover(&self.is_playing) = true;
        if let Some(ref stream) = self.stream {
            let _ = stream.play();
        }
    }

    pub fn stop(&mut self) {
        *lock_recover(&self.is_playing) = false;

        self.stream = None;
        *lock_recover(&self.decoder) = None;
        *lock_recover(&self.next_decoder) = None;
        *lock_recover(&self.current_position) = Duration::ZERO;
        *lock_recover(&self.samples_played) = 0;
        lock_recover(&self.sample_buffer).clear();
        *lock_recover(&self.buffer_position) = 0;
    }

    pub fn seek(&mut self, position: Duration) -> Result<(), AudioError> {
        info!("Seek requested to {:?}", position);

        let was_playing = *lock_recover(&self.is_playing);

        if was_playing {
            // Start fade out in callback (it will happen as long as stream is not paused)
            *lock_recover(&self.is_playing) = false;
        }

        let mut decoder_guard = lock_recover(&self.decoder);
        if let Some(ref mut dec) = *decoder_guard {
            info!("Seeking in the decoder...");

            lock_recover(&self.sample_buffer).clear();
            *lock_recover(&self.buffer_position) = 0;

            let res = dec.seek(position.as_secs_f64());
            if let Err(AudioError::OutsideOfBuffer) = res {
                if was_playing {
                    *lock_recover(&self.is_playing) = true;
                }
                return Err(AudioError::OutsideOfBuffer);
            }
            info!("Waiting for data at the new position...");
            let deadline = std::time::Instant::now() + SEEK_DATA_TIMEOUT;
            loop {
                match dec.decode_next_block() {
                    Ok(Some(samples)) => {
                        info!("Data received ({} samples), resuming...", samples.len());
                        *lock_recover(&self.sample_buffer) = samples;
                        *lock_recover(&self.buffer_position) = 0;
                        break;
                    }
                    Ok(None) => {
                        info!("End of stream reached while seeking");
                        break;
                    }
                    Err(AudioError::InProgress) => {
                        if std::time::Instant::now() >= deadline {
                            // The output callback keeps polling the decoder, so playback
                            // resumes once the data arrives.
                            warn!(
                                "No data {:?} after seek, resuming without it",
                                SEEK_DATA_TIMEOUT
                            );
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(50));
                        continue;
                    }
                    Err(e) => {
                        warn!("Error while waiting for data after seek: {}", e);
                        return Err(e);
                    }
                }
            }

            *lock_recover(&self.current_position) = position;
            let sr = self.config.sample_rate.0;
            let new_samples = (position.as_secs_f64() * sr as f64) as u64;
            *lock_recover(&self.samples_played) = new_samples;
        }

        if was_playing {
            // Start fade in
            *lock_recover(&self.is_playing) = true;
        }

        info!("Seek finished");
        Ok(())
    }

    pub fn get_recent_samples(&self) -> Vec<f32> {
        lock_recover(&self.recent_samples).clone()
    }

    pub fn channels(&self) -> usize {
        self.config.channels as usize
    }

    pub fn position(&self) -> Duration {
        *lock_recover(&self.current_position)
    }

    pub fn duration(&self) -> Option<Duration> {
        *lock_recover(&self.duration)
    }

    pub fn buffered_ranges(&self) -> Vec<(f64, f64)> {
        let decoder_guard = lock_recover(&self.decoder);
        if let Some(ref dec) = *decoder_guard {
            dec.buffered_ranges()
        } else {
            vec![]
        }
    }

    pub fn waveforms(&self) -> Vec<crate::audio::types::Waveform> {
        let decoder_guard = lock_recover(&self.decoder);
        if let Some(ref dec) = *decoder_guard {
            dec.waveforms()
        } else {
            vec![]
        }
    }

    pub fn is_playing(&self) -> bool {
        *lock_recover(&self.is_playing)
    }

    pub fn set_volume(&mut self, volume: f32) {
        let applied = effective_volume(volume, IS_ANDROID);
        *lock_recover(&self.volume) = applied;
    }

    pub fn volume(&self) -> f32 {
        *lock_recover(&self.volume)
    }

    pub fn set_fade(&self, fade_in_ms: u32, fade_out_ms: u32) {
        let sr = self.config.sample_rate.0 as f64;
        let fin = (fade_in_ms as f64 / 1000.0 * sr) as u64;
        let fout = (fade_out_ms as f64 / 1000.0 * sr) as u64;
        info!(
            "[fade] set_fade: in={}ms→{}smp, out={}ms→{}smp, sr={}",
            fade_in_ms, fin, fade_out_ms, fout, sr
        );
        self.fade_in_samples.store(fin, Ordering::Relaxed);
        self.fade_out_samples.store(fout, Ordering::Relaxed);
    }

    pub fn set_eq(&self, settings: EqSettings) {
        *lock_recover(&self.eq_settings) = settings;
        self.eq_dirty.store(true, Ordering::SeqCst);
    }

    pub fn get_eq(&self) -> EqSettings {
        lock_recover(&self.eq_settings).clone()
    }

    pub fn current_output_sample_rate(&self) -> u32 {
        self.config.sample_rate.0
    }
}
