use crate::error::AudioError;
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

/// HLS Audio Decoder
///
/// This decoder handles HLS streams by creating a separate FormatReader
/// for each segment, following the VLC architecture approach.
/// FormatReaders are cached in a Vec to avoid recreating them when seeking back.
pub struct HlsAudioDecoder {
    /// HLS source providing access to segments
    source: HlsSource,

    /// Cached FormatReaders for each segment (None if not yet created)
    format_readers: Vec<Option<Box<dyn FormatReader>>>,

    /// Current segment index being played
    current_segment_index: usize,

    /// Audio decoder
    decoder: Box<dyn Decoder>,

    /// Track ID
    track_id: u32,

    /// Sample rate
    sample_rate: u32,

    /// Number of channels
    channels: usize,

    /// Timeline tracking (like VLC approach)
    accumulated_duration: f64,
    total_duration: f64,
}

impl HlsAudioDecoder {
    /// Create HLS decoder from HlsSource
    ///
    /// # Arguments
    /// * `source` - HlsSource providing access to segments
    pub fn new(source: HlsSource) -> Result<Self, AudioError> {
        info!("Создание HLS декодера");

        let (first_segment_data, _first_duration) = source
            .get_segment(0)
            .ok_or_else(|| AudioError::DecodingError("No segments available in HlsSource".to_string()))?;

        let (mut format_reader, track_id, sample_rate, channels) =
            Self::create_format_reader_for_segment(first_segment_data)?;

        let track = format_reader
            .tracks()
            .iter()
            .find(|t| t.id == track_id)
            .ok_or_else(|| AudioError::DecodingError("Track not found".to_string()))?;

        let decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())
            .map_err(|e| AudioError::DecodingError(format!("Failed to create decoder: {}", e)))?;

        let durations = source.durations();
        let segment_count = durations.len();
        let total_duration: f64 = durations.iter().sum();

        let mut format_readers: Vec<Option<Box<dyn FormatReader>>> =
            (0..segment_count).map(|_| None).collect();
        format_readers[0] = Some(format_reader);

        info!(
            "HLS декодер создан: segments={}, sample_rate={}, channels={}, duration={:.2}s",
            segment_count, sample_rate, channels, total_duration
        );

        Ok(Self {
            source,
            format_readers,
            current_segment_index: 0,
            decoder,
            track_id,
            sample_rate,
            channels,
            accumulated_duration: 0.0,
            total_duration,
        })
    }

    pub fn decode_next_block(&mut self) -> Result<Option<Vec<f32>>, AudioError> {
        loop {
            let format_reader = self.format_readers[self.current_segment_index]
                .as_mut()
                .ok_or_else(|| AudioError::DecodingError("Format reader not available for current segment".to_string()))?;

            match format_reader.next_packet() {
                Ok(packet) => {
                    if packet.track_id() == self.track_id {
                        match self.decoder.decode(&packet) {
                            Ok(decoded) => {
                                let samples = Self::convert_to_f32(decoded)?;
                                return Ok(Some(samples));
                            }
                            Err(Error::DecodeError(e)) => {
                                warn!("Decode error (skipping): {}", e);
                                return Ok(Some(vec![]));
                            }
                            Err(e) => {
                                return Err(AudioError::DecodingError(format!("Decoder error: {}", e)));
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

                Err(e) => {
                    return Err(AudioError::DecodingError(format!("Failed to read packet: {}", e)));
                }
            }
        }
    }

    pub fn seek(&mut self, time_secs: f64) -> Result<u64, AudioError> {
        debug!("HLS seek to {:.2}s", time_secs);

        self.source.seek((time_secs * 1000.0) as u64)
            .map_err(|e| AudioError::DecodingError(format!("HLS seek failed: {}", e)))?;

        let durations = self.source.durations();

        // Find which segment contains the target time
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
                .ok_or_else(|| AudioError::DecodingError("Format reader not available".to_string()))?;

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

        info!("HLS seek completed to {:.2}s", time_secs);
        Ok((time_secs * 1000.0) as u64)
    }
    pub fn duration(&self) -> Option<f64> {
        Some(self.total_duration)
    }

    /// Get sample rate
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get number of channels
    pub fn channels(&self) -> usize {
        self.channels
    }

    // Private helper methods

    fn create_format_reader_for_segment(
        segment_data: Vec<u8>,
    ) -> Result<(Box<dyn FormatReader>, u32, u32, usize), AudioError> {
        let cursor = Cursor::new(segment_data);
        let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

        let mut hint = Hint::new();
        hint.with_extension("m4s");

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
            .ok_or_else(|| AudioError::DecodingError("No audio track found in segment".to_string()))?;

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
        let durations = self.source.durations();
        let current_duration = durations
            .get(self.current_segment_index)
            .copied()
            .unwrap_or(0.0);

        self.accumulated_duration += current_duration;
        self.current_segment_index += 1;

        if self.current_segment_index >= self.format_readers.len() {
            return Ok(false);
        }

        debug!("Switching to segment {}", self.current_segment_index);
        self.get_or_create_format_reader(self.current_segment_index)?;
        self.decoder.reset();

        Ok(true)
    }

    fn get_or_create_format_reader(&mut self, segment_index: usize) -> Result<(), AudioError> {
        if self.format_readers[segment_index].is_some() {
            return Ok(());
        }

        let (segment_data, _duration) = self
            .source
            .get_segment(segment_index)
            .ok_or_else(|| AudioError::DecodingError(format!("Segment {} not available", segment_index)))?;

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
