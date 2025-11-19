use super::decoder::AudioDecoder;
use crate::audio::types::{SampleRate, TrackSource};
use crate::error::AudioError;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig};
use std::path::Path;
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
        todo!()
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
