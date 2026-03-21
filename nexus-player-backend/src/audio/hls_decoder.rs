use crate::audio::playback::HlsReaderAdapter;
use crate::error::AudioError;
use resampler::{ResamplerFir, SampleRate};
use std::io::Cursor;
use streaming::multi_downloader::source::HlsSource;
use symphonia::core::audio::AudioBufferRef;
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::errors::Error;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;
use tracing::{debug, info, warn};

pub struct HlsAudioDecoder {
    source: HlsSource,
    format_readers: Vec<Option<Box<dyn FormatReader>>>,
    continuous_reader: Option<Box<dyn FormatReader>>,
    current_segment_index: usize,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    sample_rate: u32,
    channels: usize,
    accumulated_duration: f64,
    total_duration: f64,
    is_continuous: bool,
    resampler: Option<ResamplerFir>,
    output_sample_rate: u32,
}

impl HlsAudioDecoder {
    pub fn new(source: HlsSource, output_sample_rate: u32) -> Result<Self, AudioError> {
        info!("Создание HLS декодера");

        let durations = source.durations();
        let segment_count = durations.len();
        let total_duration: f64 = durations.iter().sum();

        let is_continuous = source.is_continuous();

        let (mut format_reader, track_id, sample_rate, channels) = if is_continuous {
            let adapter = HlsReaderAdapter::new(source.clone());
            let mss = MediaSourceStream::new(Box::new(adapter), Default::default());
            let hint = Hint::new();
            let format_opts = FormatOptions {
                enable_gapless: true,
                ..Default::default()
            };
            let metadata_opts = MetadataOptions::default();

            let probed = symphonia::default::get_probe()
                .format(&hint, mss, &format_opts, &metadata_opts)
                .map_err(|e| {
                    if let Error::IoError(io_e) = &e {
                        if io_e.kind() == std::io::ErrorKind::WouldBlock {
                            return AudioError::InProgress;
                        }
                    }
                    AudioError::DecodingError(format!("Failed to probe continuous stream: {}", e))
                })?;

            let format_reader = probed.format;
            let track = format_reader
                .tracks()
                .iter()
                .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
                .ok_or_else(|| AudioError::DecodingError("No audio track found".to_string()))?;

            let track_id = track.id;
            let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
            let channels = track
                .codec_params
                .channels
                .map(|ch| ch.count())
                .unwrap_or(2);

            (format_reader, track_id, sample_rate, channels)
        } else {
            let (first_segment_data, _first_duration) = source.get_segment(0).ok_or_else(|| {
                AudioError::DecodingError("No segments available in HlsSource".to_string())
            })?;

            Self::create_format_reader_for_segment(first_segment_data)?
        };

        let track = format_reader
            .tracks()
            .iter()
            .find(|t| t.id == track_id)
            .ok_or_else(|| AudioError::DecodingError("Track not found".to_string()))?;

        let decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())
            .map_err(|e| AudioError::DecodingError(format!("Failed to create decoder: {}", e)))?;

        let (format_readers, continuous_reader) = if is_continuous {
            (vec![], Some(format_reader))
        } else {
            let mut readers: Vec<Option<Box<dyn FormatReader>>> =
                (0..segment_count).map(|_| None).collect();
            readers[0] = Some(format_reader);
            (readers, None)
        };

        let resampler = if sample_rate != output_sample_rate {
            Some(ResamplerFir::new(
                channels,
                SampleRate::try_from(sample_rate).unwrap_or(SampleRate::Hz44100),
                SampleRate::try_from(output_sample_rate).unwrap_or(SampleRate::Hz44100),
                resampler::Latency::Sample64,
                resampler::Attenuation::Db120,
            ))
        } else {
            None
        };

        info!(
            "HLS декодер создан (continuous={}): segments={}, sample_rate={}, channels={}, duration={:.2}s",
            is_continuous, segment_count, sample_rate, channels, total_duration
        );

        Ok(Self {
            source,
            format_readers,
            continuous_reader,
            current_segment_index: 0,
            decoder,
            track_id,
            sample_rate,
            channels,
            accumulated_duration: 0.0,
            total_duration,
            is_continuous,
            resampler,
            output_sample_rate,
        })
    }

    pub fn set_output_sample_rate(&mut self, output_sample_rate: u32) {
        if self.output_sample_rate == output_sample_rate {
            return;
        }

        self.output_sample_rate = output_sample_rate;
        self.resampler = if self.sample_rate != output_sample_rate {
            Some(ResamplerFir::new(
                self.channels,
                SampleRate::try_from(self.sample_rate).unwrap_or(SampleRate::Hz44100),
                SampleRate::try_from(output_sample_rate).unwrap_or(SampleRate::Hz44100),
                resampler::Latency::Sample64,
                resampler::Attenuation::Db120,
            ))
        } else {
            None
        };
    }

    pub fn decode_next_block(&mut self) -> Result<Option<Vec<f32>>, AudioError> {
        let result = if self.is_continuous {
            self.decode_continuous()
        } else {
            self.decode_segmented()
        };

        match result {
            Ok(Some(samples)) => {
                if let Some(ref mut resampler) = self.resampler {
                    let ratio = self.output_sample_rate as f32 / self.sample_rate as f32;
                    let out_capacity =
                        ((samples.len() as f32 / self.channels as f32) * ratio).ceil() as usize
                            * self.channels;
                    let mut resampled = vec![0.0f32; out_capacity];

                    let (_consumed, produced) = resampler
                        .resample(&samples, &mut resampled)
                        .map_err(|e| AudioError::DecodingError(format!("Resample error: {}", e)))?;

                    resampled.truncate(produced);
                    Ok(Some(resampled))
                } else {
                    Ok(Some(samples))
                }
            }
            res => res,
        }
    }

    fn decode_continuous(&mut self) -> Result<Option<Vec<f32>>, AudioError> {
        loop {
            let format_reader = self.continuous_reader.as_mut().unwrap();

            match format_reader.next_packet() {
                Ok(packet) => {
                    if packet.track_id() == self.track_id {
                        match self.decoder.decode(&packet) {
                            Ok(decoded) => {
                                return Ok(Some(Self::convert_to_f32(decoded)?));
                            }
                            Err(Error::ResetRequired) => {
                                debug!(
                                    "Decoder requires reset, resetting and skipping this packet."
                                );
                                self.decoder.reset();
                                return Ok(Some(vec![]));
                            }
                            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                return Err(AudioError::InProgress);
                            }
                            Err(Error::DecodeError(e)) => {
                                warn!("Decode error (skipping): {}", e);
                                return Ok(Some(vec![]));
                            }
                            Err(e) => {
                                return Err(AudioError::DecodingError(format!(
                                    "Decoder error: {}",
                                    e
                                )));
                            }
                        }
                    }
                    continue;
                }

                Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return Ok(None);
                }

                Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    return Err(AudioError::InProgress);
                }

                Err(Error::ResetRequired) => {
                    debug!("Format reader requires reset, resetting decoder...");
                    self.decoder.reset();
                    continue;
                }

                Err(e) => {
                    return Err(AudioError::DecodingError(format!(
                        "Failed to read packet: {}",
                        e
                    )));
                }
            }
        }
    }

    fn decode_segmented(&mut self) -> Result<Option<Vec<f32>>, AudioError> {
        loop {
            let prewarm_idx = self.current_segment_index + 1;
            if prewarm_idx < self.format_readers.len() && self.format_readers[prewarm_idx].is_none()
            {
                let _ = self.get_or_create_format_reader(prewarm_idx);
            }

            let format_reader = self.format_readers[self.current_segment_index]
                .as_mut()
                .ok_or_else(|| {
                    AudioError::DecodingError(
                        "Format reader not available for current segment".to_string(),
                    )
                })?;

            match format_reader.next_packet() {
                Ok(packet) => {
                    if packet.track_id() == self.track_id {
                        match self.decoder.decode(&packet) {
                            Ok(decoded) => {
                                return Ok(Some(Self::convert_to_f32(decoded)?));
                            }
                            Err(Error::ResetRequired) => {
                                debug!(
                                    "Decoder requires reset, resetting and skipping this packet."
                                );
                                self.decoder.reset();
                                return Ok(Some(vec![]));
                            }
                            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                return Err(AudioError::InProgress);
                            }
                            Err(Error::DecodeError(e)) => {
                                warn!("Decode error (skipping): {}", e);
                                return Ok(Some(vec![]));
                            }
                            Err(e) => {
                                return Err(AudioError::DecodingError(format!(
                                    "Decoder error: {}",
                                    e
                                )));
                            }
                        }
                    }
                    continue;
                }

                Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    if !self.switch_to_next_segment()? {
                        return Ok(None);
                    }
                    continue;
                }

                Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    return Err(AudioError::InProgress);
                }

                Err(Error::ResetRequired) => {
                    debug!("Format reader requires reset, resetting decoder...");
                    self.decoder.reset();
                    continue;
                }

                Err(e) => {
                    return Err(AudioError::DecodingError(format!(
                        "Failed to read packet: {}",
                        e
                    )));
                }
            }
        }
    }

    pub fn seek(&mut self, time_secs: f64) -> Result<u64, AudioError> {
        info!(
            "HLS seek to {:.2}s is_cont: {}",
            time_secs, self.is_continuous
        );

        if self.is_continuous {
            let ranges = self.source.get_buffered_ranges();
            let is_buffered = ranges
                .iter()
                .any(|(start, end)| time_secs >= *start && time_secs <= *end);
            if !is_buffered {
                warn!(
                    "Seek to {:.2}s ignored (not buffered in contiguous mode)",
                    time_secs
                );
                return Err(AudioError::OutsideOfBuffer);
            }

            self.source
                .seek((time_secs * 1000.0) as u64)
                .map_err(|e| AudioError::DecodingError(format!("HLS source seek failed: {}", e)))?;

            let reader = self.continuous_reader.as_mut().unwrap();
            let ts = Time::from(time_secs);
            let seeked_to = reader
                .seek(
                    SeekMode::Coarse,
                    SeekTo::Time {
                        time: ts,
                        track_id: Some(self.track_id),
                    },
                )
                .map_err(|e| AudioError::DecodingError(format!("Seek failed: {}", e)))?;

            info!("HLS continuous seek completed to {:.2}s", time_secs);
            return Ok(seeked_to.actual_ts);
        }

        self.source
            .seek((time_secs * 1000.0) as u64)
            .map_err(|e| AudioError::DecodingError(format!("HLS seek failed: {}", e)))?;

        let durations = self.source.durations();

        let mut accumulated = 0.0;
        let mut target_segment = 0;

        for (i, &duration) in durations.iter().enumerate() {
            if time_secs < accumulated + duration {
                target_segment = i;
                break;
            }
            accumulated += duration;
        }

        if target_segment >= durations.len() {
            target_segment = durations.len().saturating_sub(1);
        }

        debug!("Target segment: {}/{}", target_segment, durations.len());
        self.get_or_create_format_reader(target_segment)?;

        self.current_segment_index = target_segment;
        self.accumulated_duration = accumulated;

        let segment_offset = time_secs - accumulated;
        if segment_offset > 0.01 {
            let format_reader = self.format_readers[target_segment]
                .as_mut()
                .ok_or_else(|| {
                    AudioError::DecodingError("Format reader not available".to_string())
                })?;

            let ts = Time::from(segment_offset);
            let _ = format_reader.seek(
                SeekMode::Coarse,
                SeekTo::Time {
                    time: ts,
                    track_id: Some(self.track_id),
                },
            );
        }

        self.decoder.reset();

        // Recreate resampler to clear history
        if self.sample_rate != self.output_sample_rate {
            self.resampler = Some(ResamplerFir::new(
                self.channels,
                SampleRate::try_from(self.sample_rate).unwrap_or(SampleRate::Hz44100),
                SampleRate::try_from(self.output_sample_rate).unwrap_or(SampleRate::Hz44100),
                resampler::Latency::Sample64,
                resampler::Attenuation::Db120,
            ));
        }

        info!("HLS seek completed to {:.2}s", time_secs);
        Ok((time_secs * 1000.0) as u64)
    }

    pub fn duration(&self) -> Option<f64> {
        Some(self.total_duration)
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    pub fn buffered_ranges(&self) -> Vec<(f64, f64)> {
        self.source.get_buffered_ranges()
    }

    pub fn waveforms(&self) -> Vec<crate::audio::types::Waveform> {
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

    fn create_format_reader_for_segment(
        segment_data: Vec<u8>,
    ) -> Result<(Box<dyn FormatReader>, u32, u32, usize), AudioError> {
        let cursor = Cursor::new(segment_data);
        let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

        let mut hint = Hint::new();

        let format_opts = FormatOptions::default();
        let metadata_opts = MetadataOptions::default();

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &format_opts, &metadata_opts)
            .map_err(|e| AudioError::DecodingError(format!("Failed to probe segment: {}", e)))?;

        let format_reader = probed.format;

        let track = format_reader
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
            .ok_or_else(|| {
                AudioError::DecodingError("No audio track found in segment".to_string())
            })?;

        let track_id = track.id;
        let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
        let channels = track
            .codec_params
            .channels
            .map(|ch| ch.count())
            .unwrap_or(2);

        Ok((format_reader, track_id, sample_rate, channels))
    }

    fn switch_to_next_segment(&mut self) -> Result<bool, AudioError> {
        let next_index = self.current_segment_index + 1;

        if next_index >= self.format_readers.len() {
            return Ok(false);
        }

        self.get_or_create_format_reader(next_index)?;

        let durations = self.source.durations();
        let current_duration = durations
            .get(self.current_segment_index)
            .copied()
            .unwrap_or(0.0);

        self.accumulated_duration += current_duration;
        self.current_segment_index = next_index;

        info!("Переключение на сегмент {}", self.current_segment_index);

        let prewarm_idx = next_index + 1;
        if prewarm_idx < self.format_readers.len() {
            let _ = self.get_or_create_format_reader(prewarm_idx);
        }

        Ok(true)
    }

    fn get_or_create_format_reader(&mut self, segment_index: usize) -> Result<(), AudioError> {
        if let Some(ref _reader) = self.format_readers[segment_index] {
            return Ok(());
        }

        let (segment_data, _duration) = self
            .source
            .get_segment(segment_index)
            .ok_or(AudioError::InProgress)?;

        let (format_reader, _, _, _) = Self::create_format_reader_for_segment(segment_data)?;
        self.format_readers[segment_index] = Some(format_reader);

        Ok(())
    }

    fn convert_to_f32(buffer: AudioBufferRef) -> Result<Vec<f32>, AudioError> {
        use symphonia::core::audio::SampleBuffer;

        let mut sample_buf = SampleBuffer::<f32>::new(buffer.capacity() as u64, *buffer.spec());
        sample_buf.copy_interleaved_ref(buffer);

        Ok(sample_buf.samples().to_vec())
    }
}
