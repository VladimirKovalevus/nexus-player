use super::decoder::{AudioDecoder, DecoderType};
use super::hls_decoder::HlsAudioDecoder;
use crate::audio::types::{AudioSource, MediaSource, SampleRate, TrackSource};
use crate::error::AudioError;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::result;
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

pub struct PlaybackEngine {
    decoder: Arc<Mutex<Option<DecoderType>>>,
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
    volume: Arc<Mutex<f32>>,
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
            volume: Arc::new(Mutex::new(1.0)),
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

    pub fn play_audio_source(
        &mut self,
        audio_source: Box<dyn MediaSource>,
    ) -> Result<(), AudioError> {
        info!("Загрузка аудио из AudioSource");

        let decoder = AudioDecoder::from_reader(audio_source, None)?;
        let duration = decoder.duration().map(Duration::from_secs_f64);
        let sample_rate = decoder.sample_rate();
        let channels = decoder.channels();

        info!(
            "Sample rate: {}, channels: {}, duration: {:?}",
            sample_rate, channels, duration
        );

        self.duration = duration;
        *self.decoder.lock().unwrap() = Some(DecoderType::from_standard(decoder));
        *self.is_playing.lock().unwrap() = true;
        *self.current_position.lock().unwrap() = Duration::ZERO;
        *self.sample_rate.lock().unwrap() = sample_rate;
        *self.samples_played.lock().unwrap() = 0;

        self.sample_buffer.lock().unwrap().clear();
        *self.buffer_position.lock().unwrap() = 0;

        self.create_stream(sample_rate, channels)?;

        Ok(())
    }

    pub fn play_hls(&mut self, hls_source: HlsSource) -> Result<(), AudioError> {
        info!("Загрузка HLS потока");

        // Prepare source (wait for metadata/first segment)
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _ = rt.block_on(hls_source.prepare());

        let decoder = HlsAudioDecoder::new(hls_source)?;
        let duration = decoder.duration().map(Duration::from_secs_f64);
        let sample_rate = decoder.sample_rate();
        let channels = decoder.channels();

        info!(
            "HLS: sample_rate={}, channels={}, duration={:?}",
            sample_rate, channels, duration
        );

        self.duration = duration;
        *self.decoder.lock().unwrap() = Some(DecoderType::from_hls(decoder));
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
        *self.decoder.lock().unwrap() = Some(DecoderType::from_standard(decoder));
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
        let volume = Arc::clone(&self.volume);

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

                        let volume = *volume.lock().unwrap();
                        data[output_pos..output_pos + to_copy]
                            .iter_mut()
                            .zip(&buffer[*buf_pos..*buf_pos + to_copy])
                            .for_each(|(out, sample)| *out = sample * volume);

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
        self.sample_buffer.lock().unwrap().clear();
        *self.buffer_position.lock().unwrap() = 0;
    }

    pub fn seek(&mut self, position: Duration) -> Result<(), AudioError> {
        info!("Запрос перемотки на {:?}", position);

        // Сохраняем состояние воспроизведения
        let was_playing = *self.is_playing.lock().unwrap();

        // Приостанавливаем стрим на время перемотки
        if let Some(ref stream) = self.stream {
            let _ = stream.pause();
        }

        let mut decoder_guard = self.decoder.lock().unwrap();
        if let Some(ref mut dec) = *decoder_guard {
            info!("Выполнение перемотки в декодере...");
            dec.seek(position.as_secs_f64())?;

            // Очищаем буферы
            self.sample_buffer.lock().unwrap().clear();
            *self.buffer_position.lock().unwrap() = 0;

            // Цикл ожидания данных
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
                        // Данные еще не загружены, ждем
                        std::thread::sleep(std::time::Duration::from_millis(50));
                        continue;
                    }
                    Err(e) => {
                        warn!("Ошибка при ожидании данных после перемотки: {}", e);
                        return Err(e);
                    }
                }
            }

            // Обновляем позицию и счетчик сэмплов
            *self.current_position.lock().unwrap() = position;
            let sr = *self.sample_rate.lock().unwrap();
            let new_samples = (position.as_secs_f64() * sr as f64) as u64;
            *self.samples_played.lock().unwrap() = new_samples;
        }

        // Возобновляем стрим, если он был активен
        if was_playing {
            if let Some(ref stream) = self.stream {
                let _ = stream.play();
            }
        }

        info!("Перемотка завершена");
        Ok(())
    }

    pub fn position(&self) -> Duration {
        *self.current_position.lock().unwrap()
    }

    pub fn duration(&self) -> Option<Duration> {
        self.duration
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
}
