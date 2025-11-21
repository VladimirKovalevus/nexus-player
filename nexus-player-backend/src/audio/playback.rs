use super::decoder::AudioDecoder;
use crate::audio::types::{AudioSource, SampleRate, TrackSource};
use crate::error::AudioError;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig};
use std::path::{Path, PathBuf};
use std::result;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tracing::{info, warn};

pub struct PlaybackEngine {
    decoder: Arc<Mutex<Option<AudioDecoder>>>,
    stream: Option<Stream>,
    device: Device,
    config: StreamConfig,
    is_playing: Arc<Mutex<bool>>,
    current_position: Arc<Mutex<Duration>>,
    duration: Option<Duration>,
    sample_rate: Arc<Mutex<u32>>,
    samples_played: Arc<Mutex<u64>>,
    sample_buffer: Arc<Mutex<Vec<f32>>>,
    buffer_position: Arc<Mutex<usize>>,
}

impl PlaybackEngine {
    pub fn new() -> Result<Self, AudioError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| AudioError::OutputError("Не найдено аудио устройство".to_string()))?;

        info!(
            "Аудио устройство: {}",
            device.name().unwrap_or_else(|_| "Unknown".to_string())
        );

        let config = device
            .default_output_config()
            .map_err(|e| {
                AudioError::OutputError(format!("Не удалось получить конфигурацию: {}", e))
            })?
            .into();

        Ok(Self {
            decoder: Arc::new(Mutex::new(None)),
            stream: None,
            device,
            config,
            is_playing: Arc::new(Mutex::new(false)),
            current_position: Arc::new(Mutex::new(Duration::ZERO)),
            duration: None,
            sample_rate: Arc::new(Mutex::new(SampleRate::LOW.as_u32())),
            samples_played: Arc::new(Mutex::new(0)),
            sample_buffer: Arc::new(Mutex::new(Vec::new())),
            buffer_position: Arc::new(Mutex::new(0)),
        })
    }

    pub fn play(&mut self, path: &TrackSource) -> Result<(), AudioError> {
        match path {
            TrackSource::Local(p) => self.play_local(&p),
            TrackSource::Http(p) => todo!(),
            TrackSource::External { system, id } => todo!(),
            TrackSource::Cached {
                cache_path,
                original_url,
            } => todo!(),
        }
    }

    pub fn play_audio_source(&mut self, audio_source: AudioSource) -> Result<(), AudioError> {
        info!("Загрузка аудио из AudioSource");

        let decoder = AudioDecoder::from_reader(audio_source.into_reader(), None)?;
        let duration = decoder.duration().map(Duration::from_secs_f64);
        let sample_rate = decoder.sample_rate();
        let channels = decoder.channels();

        info!(
            "Sample rate: {}, channels: {}, duration: {:?}",
            sample_rate, channels, duration
        );

        self.duration = duration;
        *self.decoder.lock().unwrap() = Some(decoder);
        *self.is_playing.lock().unwrap() = true;
        *self.current_position.lock().unwrap() = Duration::ZERO;
        *self.sample_rate.lock().unwrap() = sample_rate;
        *self.samples_played.lock().unwrap() = 0;

        self.sample_buffer.lock().unwrap().clear();
        *self.buffer_position.lock().unwrap() = 0;

        self.create_stream(sample_rate, channels)?;

        Ok(())
    }
    fn play_local(&mut self, path: &Path) -> Result<(), AudioError> {
        info!("Загрузка файла: {:?}", path);

        let decoder = AudioDecoder::new(path)?;
        let duration = decoder.duration().map(Duration::from_secs_f64);
        let sample_rate = decoder.sample_rate();
        let channels = decoder.channels();

        info!(
            "Sample rate: {}, channels: {}, duration: {:?}",
            sample_rate, channels, duration
        );

        self.duration = duration;
        *self.decoder.lock().unwrap() = Some(decoder);
        *self.is_playing.lock().unwrap() = true;
        *self.current_position.lock().unwrap() = Duration::ZERO;
        *self.sample_rate.lock().unwrap() = sample_rate;
        *self.samples_played.lock().unwrap() = 0;

        self.sample_buffer.lock().unwrap().clear();
        *self.buffer_position.lock().unwrap() = 0;

        self.create_stream(sample_rate, channels)?;

        Ok(())
    }
    fn create_stream(&mut self, sample_rate: u32, channels: usize) -> Result<(), AudioError> {
        let decoder = Arc::clone(&self.decoder);
        let is_playing = Arc::clone(&self.is_playing);
        let samples_played = Arc::clone(&self.samples_played);
        let current_position = Arc::clone(&self.current_position);
        let sample_rate_arc = Arc::clone(&self.sample_rate);
        let sample_buffer = Arc::clone(&self.sample_buffer);
        let buffer_position = Arc::clone(&self.buffer_position);

        let mut config = self.config.clone();
        config.sample_rate = cpal::SampleRate(sample_rate);
        config.channels = channels as u16;

        let err_fn = |err| warn!("Ошибка стрима: {}", err);

        let stream = self
            .device
            .build_output_stream(
                &config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    if !*is_playing.lock().unwrap() {
                        data.fill(0.0);
                        return;
                    }

                    let mut output_pos = 0;

                    while output_pos < data.len() {
                        let mut buffer = sample_buffer.lock().unwrap();
                        let mut buf_pos = buffer_position.lock().unwrap();

                        if *buf_pos >= buffer.len() {
                            drop(buf_pos);
                            drop(buffer);

                            let mut decoder_guard = decoder.lock().unwrap();
                            if let Some(ref mut dec) = *decoder_guard {
                                match dec.decode_next_block() {
                                    Ok(Some(samples)) => {
                                        *sample_buffer.lock().unwrap() = samples;
                                        *buffer_position.lock().unwrap() = 0;
                                    }
                                    Ok(None) => {
                                        data[output_pos..].fill(0.0);
                                        *is_playing.lock().unwrap() = false;
                                        return;
                                    }
                                    Err(e) => {
                                        warn!("Ошибка декодирования: {}", e);
                                        data[output_pos..].fill(0.0);
                                        return;
                                    }
                                }
                            } else {
                                data[output_pos..].fill(0.0);
                                return;
                            }

                            buffer = sample_buffer.lock().unwrap();
                            buf_pos = buffer_position.lock().unwrap();
                        }

                        let available = buffer.len() - *buf_pos;
                        let needed = data.len() - output_pos;
                        let to_copy = available.min(needed);

                        data[output_pos..output_pos + to_copy]
                            .copy_from_slice(&buffer[*buf_pos..*buf_pos + to_copy]);

                        *buf_pos += to_copy;
                        output_pos += to_copy;

                        let num_frames = to_copy / channels;
                        *samples_played.lock().unwrap() += num_frames as u64;
                        let sr = *sample_rate_arc.lock().unwrap();
                        let position_secs = *samples_played.lock().unwrap() as f64 / sr as f64;
                        *current_position.lock().unwrap() = Duration::from_secs_f64(position_secs);
                    }
                },
                err_fn,
                None,
            )
            .map_err(|e| AudioError::OutputError(format!("Не удалось создать стрим: {}", e)))?;

        stream
            .play()
            .map_err(|e| AudioError::OutputError(format!("Не удалось запустить стрим: {}", e)))?;

        self.stream = Some(stream);

        Ok(())
    }

    pub fn pause(&mut self) {
        *self.is_playing.lock().unwrap() = false;
        if let Some(ref stream) = self.stream {
            let _ = stream.pause();
        }
    }

    pub fn resume(&mut self) {
        *self.is_playing.lock().unwrap() = true;
        if let Some(ref stream) = self.stream {
            let _ = stream.play();
        }
    }

    pub fn stop(&mut self) {
        *self.is_playing.lock().unwrap() = false;
        self.stream = None;
        *self.decoder.lock().unwrap() = None;
        *self.current_position.lock().unwrap() = Duration::ZERO;
        *self.samples_played.lock().unwrap() = 0;
        self.sample_buffer.lock();
        *self.buffer_position.lock().unwrap() = 0;
    }

    pub fn seek(&mut self, position: Duration) -> Result<(), AudioError> {
        let mut decoder_guard = self.decoder.lock().unwrap();
        if let Some(ref mut dec) = *decoder_guard {
            dec.seek(position.as_secs_f64())?;
            *self.current_position.lock().unwrap() = position;

            let sr = *self.sample_rate.lock().unwrap();
            let new_samples = (position.as_secs_f64() * sr as f64) as u64;
            *self.samples_played.lock().unwrap() = new_samples;

            self.sample_buffer.lock().unwrap().clear();
            *self.buffer_position.lock().unwrap() = 0;
        }
        Ok(())
    }

    pub fn position(&self) -> Duration {
        *self.current_position.lock().unwrap()
    }

    pub fn duration(&self) -> Option<Duration> {
        self.duration
    }

    pub fn is_playing(&self) -> bool {
        *self.is_playing.lock().unwrap()
    }
}
