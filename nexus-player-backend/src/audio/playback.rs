use super::decoder::{AudioDecoder, DecoderType};
use super::eq::{EqProcessor, EqSettings};
use super::hls_decoder::HlsAudioDecoder;
use crate::audio::types::{AudioSource, MediaSource, SampleRate as MSR, TrackSource};
use crate::error::AudioError;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig};
use std::io::{Read, Seek, SeekFrom};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use streaming::multi_downloader::source::HlsSource;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct HlsReaderAdapter {
    source: HlsSource,
    position: u64,
}

impl HlsReaderAdapter {
    pub fn new(source: HlsSource) -> Self {
        Self {
            source,
            position: 0,
        }
    }
}

impl Read for HlsReaderAdapter {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.source.read(buf) {
            Ok(n) => {
                self.position += n as u64;
                Ok(n)
            }
            Err(e) => Err(e),
        }
    }
}

impl Seek for HlsReaderAdapter {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let total_size = self.source.estimate_total_size().unwrap_or(0);
        let new_pos = match pos {
            SeekFrom::Start(p) => p,
            SeekFrom::End(p) => {
                if total_size == 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "Unknown stream size",
                    ));
                }
                if p >= 0 {
                    total_size.saturating_add(p as u64)
                } else {
                    total_size.saturating_sub(p.abs() as u64)
                }
            }
            SeekFrom::Current(p) => {
                if p >= 0 {
                    self.position.saturating_add(p as u64)
                } else {
                    self.position.saturating_sub(p.abs() as u64)
                }
            }
        };

        if new_pos != self.position {
            info!(
                "HlsReaderAdapter seek: {} -> {} (total: {})",
                self.position, new_pos, total_size
            );
            if let Err(e) = self.source.seek_to_byte_approx(new_pos) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ));
            }
            self.position = new_pos;
        }

        Ok(self.position)
    }
}

impl symphonia::core::io::MediaSource for HlsReaderAdapter {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        self.source.estimate_total_size()
    }
}

impl MediaSource for HlsReaderAdapter {
    fn clone_box(&self) -> Box<dyn MediaSource> {
        Box::new(self.clone())
    }

    fn get_waveforms(&self) -> Vec<crate::audio::types::Waveform> {
        self.source
            .get_waveforms()
            .into_iter()
            .map(|w| crate::audio::types::Waveform {
                data: w.data,
                start_time: w.start_time,
                end_time: w.end_time,
                byte_start: w.byte_start,
                byte_end: w.byte_end,
            })
            .collect()
    }
}

use std::sync::atomic::{AtomicBool, Ordering};

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
            volume: Arc::new(Mutex::new(0.1)),
            recent_samples: Arc::new(Mutex::new(Vec::with_capacity(8192))),
            needs_recreation: Arc::new(AtomicBool::new(false)),
            eq_settings: Arc::new(Mutex::new(EqSettings::default())),
            eq_dirty: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn play(&mut self, path: &TrackSource) -> Result<(), AudioError> {
        match path {
            TrackSource::Local(p) => self.play_local(&p),
            _ => todo!(),
        }
    }

    pub fn play_audio_source(
        &mut self,
        audio_source: Box<dyn MediaSource>,
    ) -> Result<(), AudioError> {
        info!("Загрузка аудио из AudioSource");

        let output_sample_rate = self.config.sample_rate.0;
        let decoder = AudioDecoder::from_reader(audio_source, None, output_sample_rate)?;
        let duration = decoder.duration().map(Duration::from_secs_f64);
        let sample_rate = decoder.sample_rate();
        let channels = decoder.channels();

        info!(
            "Sample rate: {}, channels: {}, duration: {:?}",
            sample_rate, channels, duration
        );

        *self.duration.lock().unwrap() = duration;
        *self.decoder.lock().unwrap() = Some(DecoderType::from_standard(decoder));
        *self.is_playing.lock().unwrap() = true;
        *self.current_position.lock().unwrap() = Duration::ZERO;
        *self.sample_rate.lock().unwrap() = sample_rate;
        *self.samples_played.lock().unwrap() = 0;

        self.sample_buffer.lock().unwrap().clear();
        *self.buffer_position.lock().unwrap() = 0;

        self.create_stream()?;

        Ok(())
    }

    pub fn play_hls(&mut self, hls_source: HlsSource) -> Result<(), AudioError> {
        info!("Загрузка HLS потока");

        let output_sample_rate = self.config.sample_rate.0;
        let decoder = HlsAudioDecoder::new(hls_source, output_sample_rate)?;
        let duration = decoder.duration().map(Duration::from_secs_f64);
        let sample_rate = decoder.sample_rate();
        let channels = decoder.channels();

        info!(
            "HLS: sample_rate={}, channels={}, duration={:?}",
            sample_rate, channels, duration
        );

        *self.duration.lock().unwrap() = duration;
        *self.decoder.lock().unwrap() = Some(DecoderType::from_hls(decoder));
        *self.is_playing.lock().unwrap() = true;
        *self.current_position.lock().unwrap() = Duration::ZERO;
        *self.sample_rate.lock().unwrap() = sample_rate;
        *self.samples_played.lock().unwrap() = 0;

        self.sample_buffer.lock().unwrap().clear();
        *self.buffer_position.lock().unwrap() = 0;

        self.create_stream()?;

        Ok(())
    }

    pub fn preload_local(&mut self, path: &std::path::Path) -> Result<(), AudioError> {
        info!("[PlaybackEngine] Начало предзагрузки локального файла: {:?}", path);
        let output_sample_rate = self.config.sample_rate.0;
        info!("[PlaybackEngine] Инициализация декодера для локального файла, output_sample_rate: {}", output_sample_rate);
        let decoder = AudioDecoder::new(path, output_sample_rate)?;
        info!("[PlaybackEngine] Декодер создан успешно. Сохранение в next_decoder.");
        *self.next_decoder.lock().unwrap() = Some(DecoderType::from_standard(decoder));
        info!("[PlaybackEngine] Предзагрузка локального файла завершена.");
        Ok(())
    }

    pub fn preload_hls(&mut self, source: HlsSource) -> Result<(), AudioError> {
        info!("[PlaybackEngine] Начало предзагрузки HLS потока");
        let output_sample_rate = self.config.sample_rate.0;
        info!("[PlaybackEngine] Инициализация HLS декодера, output_sample_rate: {}", output_sample_rate);
        let decoder = HlsAudioDecoder::new(source, output_sample_rate)?;
        info!("[PlaybackEngine] HLS декодер создан успешно. Сохранение в next_decoder.");
        *self.next_decoder.lock().unwrap() = Some(DecoderType::from_hls(decoder));
        info!("[PlaybackEngine] Предзагрузка HLS потока завершена.");
        Ok(())
    }

    pub fn preload_audio_source(
        &mut self,
        source: Box<dyn MediaSource>,
    ) -> Result<(), AudioError> {
        info!("[PlaybackEngine] Начало предзагрузки из AudioSource");
        let output_sample_rate = self.config.sample_rate.0;
        info!("[PlaybackEngine] Инициализация декодера для AudioSource, output_sample_rate: {}", output_sample_rate);
        let decoder = AudioDecoder::from_reader(source, None, output_sample_rate)?;
        info!("[PlaybackEngine] Декодер для AudioSource создан успешно. Сохранение в next_decoder.");
        *self.next_decoder.lock().unwrap() = Some(DecoderType::from_standard(decoder));
        info!("[PlaybackEngine] Предзагрузка из AudioSource завершена.");
        Ok(())
    }

    pub fn use_preloaded(&mut self) -> Result<(), AudioError> {
        let next = {
            let mut next_guard = self.next_decoder.lock().unwrap();
            next_guard.take()
        };

        if let Some(next) = next {
            info!("[PlaybackEngine] Использование предзагруженного трека");
            let duration = next.duration().map(Duration::from_secs_f64);
            let sample_rate = next.sample_rate();

            *self.duration.lock().unwrap() = duration;
            *self.decoder.lock().unwrap() = Some(next);
            *self.is_playing.lock().unwrap() = true;
            *self.current_position.lock().unwrap() = Duration::ZERO;
            *self.sample_rate.lock().unwrap() = sample_rate;
            *self.samples_played.lock().unwrap() = 0;

            self.sample_buffer.lock().unwrap().clear();
            *self.buffer_position.lock().unwrap() = 0;

            self.create_stream()?;
            Ok(())
        } else {
            Err(AudioError::DecodingError(
                "No preloaded track found".to_string(),
            ))
        }
    }

    fn play_local(&mut self, path: &std::path::Path) -> Result<(), AudioError> {
        info!("Загрузка файла: {:?}", path);

        let output_sample_rate = self.config.sample_rate.0;
        let decoder = AudioDecoder::new(path, output_sample_rate)?;
        let duration = decoder.duration().map(Duration::from_secs_f64);
        let sample_rate = decoder.sample_rate();
        let channels = decoder.channels();

        info!(
            "Sample rate: {}, channels: {}, duration: {:?}",
            sample_rate, channels, duration
        );

        *self.duration.lock().unwrap() = duration;
        *self.decoder.lock().unwrap() = Some(DecoderType::from_standard(decoder));
        *self.is_playing.lock().unwrap() = true;
        *self.current_position.lock().unwrap() = Duration::ZERO;
        *self.sample_rate.lock().unwrap() = sample_rate;
        *self.samples_played.lock().unwrap() = 0;

        self.sample_buffer.lock().unwrap().clear();
        *self.buffer_position.lock().unwrap() = 0;

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
        let eq_settings = Arc::clone(&self.eq_settings);
        let eq_dirty = Arc::clone(&self.eq_dirty);

        let config = self.config.clone();
        let sample_rate = config.sample_rate.0;
        let mut current_ramp_gain = 0.0f32;
        // 10ms ramp
        let ramp_step = 1.0 / (sample_rate as f32 * 0.01);
        // EQ processor lives entirely inside this closure; state (z1/z2) is never shared.
        let initial_eq = eq_settings.lock().unwrap().clone();
        let mut eq_proc = EqProcessor::new(&initial_eq, sample_rate, config.channels as usize);

        let err_fn = move |err| {
            warn!("Ошибка стрима: {}", err);
            needs_recreation.store(true, Ordering::SeqCst);
        };

        let stream = self
            .device
            .build_output_stream(
                &config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let playing = *is_playing.lock().unwrap();

                    // If not playing and already faded out, just fill with silence
                    if !playing && current_ramp_gain <= 0.0 {
                        data.fill(0.0);
                        return;
                    }

                    let mut output_pos = 0;
                    let channels = config.channels as usize;

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
                                        // Текущий трек закончился. Проверяем наличие следующего.
                                        let mut next_guard = next_decoder.lock().unwrap();
                                        if let Some(next) = next_guard.take() {
                                            info!("Бесшовный переход на следующий трек");
                                            let next_duration = next.duration().map(Duration::from_secs_f64);
                                            let next_sr = next.sample_rate();
                                            
                                            *dec = next;
                                            *duration_arc.lock().unwrap() = next_duration;
                                            *samples_played.lock().unwrap() = 0;
                                            *sample_rate_arc.lock().unwrap() = next_sr;
                                            track_switched.store(true, Ordering::SeqCst);
                                            // Продолжаем цикл для нового декодера
                                            continue;
                                        } else {
                                            // No more tracks, but let it fade out if still has gain
                                            if current_ramp_gain > 0.0 {
                                                // We'll fill the rest with silence and let the ramp handle it
                                                data[output_pos..].fill(0.0);
                                            } else {
                                                data[output_pos..].fill(0.0);
                                                *is_playing.lock().unwrap() = false;
                                            }
                                            break;
                                        }
                                    }
                                    Err(AudioError::InProgress) => {
                                        data[output_pos..].fill(0.0);
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

                        let input = &buffer[*buf_pos..*buf_pos + to_copy];
                        
                        // Volume will be applied in the final ramp loop below
                        data[output_pos..output_pos + to_copy].copy_from_slice(input);

                        *buf_pos += to_copy;
                        output_pos += to_copy;

                        let num_frames = to_copy / channels;
                        *samples_played.lock().unwrap() += num_frames as u64;
                        let sr = *sample_rate_arc.lock().unwrap();
                        let position_secs = *samples_played.lock().unwrap() as f64 / config.sample_rate.0 as f64;
                        *current_position.lock().unwrap() = Duration::from_secs_f64(position_secs);
                    }

                    // Rebuild EQ filters when settings changed from the command channel.
                    if eq_dirty.load(Ordering::Relaxed) {
                        eq_dirty.store(false, Ordering::Relaxed);
                        if let Ok(settings) = eq_settings.try_lock() {
                            eq_proc.rebuild(&settings, sample_rate);
                        }
                    }

                    // Apply volume ramp and master volume
                    let vol = *volume.lock().unwrap();
                    for sample in data.iter_mut() {
                        if playing {
                            current_ramp_gain = (current_ramp_gain + ramp_step).min(1.0);
                        } else {
                            current_ramp_gain = (current_ramp_gain - ramp_step).max(0.0);
                        }
                        *sample *= current_ramp_gain * vol;
                    }

                    // Apply EQ after volume ramp.
                    eq_proc.process(data, config.channels as usize);

                    {
                        if let Ok(mut recent) = recent_samples.try_lock() {
                            recent.extend_from_slice(&data);
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
            .map_err(|e| AudioError::OutputError(format!("Не удалось создать стрим: {}", e)))?;

        stream
            .play()
            .map_err(|e| AudioError::OutputError(format!("Не удалось запустить стрим: {}", e)))?;

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
        info!("Попытка пересоздания стрима (возможно смена устройства)...");

        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| AudioError::OutputError("Не найдено аудио устройство".to_string()))?;

        info!(
            "Новое аудио устройство: {}",
            device.name().unwrap_or_else(|_| "Unknown".to_string())
        );

        let config = device
            .default_output_config()
            .map_err(|e| {
                AudioError::OutputError(format!("Не удалось получить конфигурацию: {}", e))
            })?
            .into();

        self.device = device;
        self.config = config;
        self.stream = None;

        let output_sample_rate = self.config.sample_rate.0;
        {
            let mut decoder_guard = self.decoder.lock().unwrap();
            if let Some(ref mut dec) = *decoder_guard {
                dec.set_output_sample_rate(output_sample_rate);
            }
        }
        {
            let mut next_guard = self.next_decoder.lock().unwrap();
            if let Some(ref mut dec) = *next_guard {
                dec.set_output_sample_rate(output_sample_rate);
            }
        }

        self.create_stream()?;

        if *self.is_playing.lock().unwrap() {
            if let Some(ref stream) = self.stream {
                let _ = stream.play();
            }
        }

        info!("Стрим успешно пересоздан");
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
        *self.next_decoder.lock().unwrap() = None;
        *self.current_position.lock().unwrap() = Duration::ZERO;
        *self.samples_played.lock().unwrap() = 0;
        self.sample_buffer.lock().unwrap().clear();
        *self.buffer_position.lock().unwrap() = 0;
    }

    pub fn seek(&mut self, position: Duration) -> Result<(), AudioError> {
        info!("Запрос перемотки на {:?}", position);

        let was_playing = *self.is_playing.lock().unwrap();

        if was_playing {
            // Start fade out in callback (it will happen as long as stream is not paused)
            *self.is_playing.lock().unwrap() = false;
        }

        let mut decoder_guard = self.decoder.lock().unwrap();
        if let Some(ref mut dec) = *decoder_guard {
            info!("Выполнение перемотки в декодере...");

            self.sample_buffer.lock().unwrap().clear();
            *self.buffer_position.lock().unwrap() = 0;

            let res = dec.seek(position.as_secs_f64());
            if let Err(AudioError::OutsideOfBuffer) = res {
                if was_playing {
                    *self.is_playing.lock().unwrap() = true;
                }
                return Err(AudioError::OutsideOfBuffer);
            }
            info!("Ожидание загрузки данных для новой позиции...");
            loop {
                match dec.decode_next_block() {
                    Ok(Some(samples)) => {
                        info!(
                            "Данные получены ({} сэмплов), возобновление...",
                            samples.len()
                        );
                        *self.sample_buffer.lock().unwrap() = samples;
                        *self.buffer_position.lock().unwrap() = 0;
                        break;
                    }
                    Ok(None) => {
                        info!("Конец потока достигнут при перемотке");
                        break;
                    }
                    Err(AudioError::InProgress) => {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                        continue;
                    }
                    Err(e) => {
                        warn!("Ошибка при ожидании данных после перемотки: {}", e);
                        return Err(e);
                    }
                }
            }

            *self.current_position.lock().unwrap() = position;
            let sr = self.config.sample_rate.0;
            let new_samples = (position.as_secs_f64() * sr as f64) as u64;
            *self.samples_played.lock().unwrap() = new_samples;
        }

        if was_playing {
            // Start fade in
            *self.is_playing.lock().unwrap() = true;
        }

        info!("Перемотка завершена");
        Ok(())
    }

    pub fn get_recent_samples(&self) -> Vec<f32> {
        self.recent_samples.lock().unwrap().clone()
    }

    pub fn channels(&self) -> usize {
        self.config.channels as usize
    }

    pub fn position(&self) -> Duration {
        *self.current_position.lock().unwrap()
    }

    pub fn duration(&self) -> Option<Duration> {
        *self.duration.lock().unwrap()
    }

    pub fn buffered_ranges(&self) -> Vec<(f64, f64)> {
        let decoder_guard = self.decoder.lock().unwrap();
        if let Some(ref dec) = *decoder_guard {
            dec.buffered_ranges()
        } else {
            vec![]
        }
    }

    pub fn waveforms(&self) -> Vec<crate::audio::types::Waveform> {
        let decoder_guard = self.decoder.lock().unwrap();
        if let Some(ref dec) = *decoder_guard {
            dec.waveforms()
        } else {
            vec![]
        }
    }

    pub fn is_playing(&self) -> bool {
        *self.is_playing.lock().unwrap()
    }

    pub fn set_volume(&mut self, volume: f32) {
        let clamped = volume.clamp(0.0, 1.0);
        *self.volume.lock().unwrap() = clamped;
    }

    pub fn volume(&self) -> f32 {
        *self.volume.lock().unwrap()
    }

    pub fn set_eq(&self, settings: EqSettings) {
        *self.eq_settings.lock().unwrap() = settings;
        self.eq_dirty.store(true, Ordering::SeqCst);
    }

    pub fn get_eq(&self) -> EqSettings {
        self.eq_settings.lock().unwrap().clone()
    }

    pub fn current_output_sample_rate(&self) -> u32 {
        self.config.sample_rate.0
    }
}
