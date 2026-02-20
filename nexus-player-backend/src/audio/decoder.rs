use crate::audio::hls_decoder::HlsAudioDecoder;
use crate::audio::types::MediaSource;
use crate::error::AudioError;
use lofty::probe;
use std::io::{Read, Seek};
use std::path::Path;
use streaming::multi_downloader::source::HlsSource;
use symphonia::core::audio::AudioBufferRef;
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::errors::Error;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::{MediaSourceStream, ReadOnlySource};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;
use tracing::{debug, info, warn};

pub struct AudioDecoder {
    format_reader: Box<dyn FormatReader>,

    decoder: Box<dyn Decoder>,

    reader: Option<Box<dyn MediaSource>>,

    track_id: u32,

    sample_rate: u32,

    channels: usize,

    total_frames: Option<u64>,
}

impl AudioDecoder {
    pub fn new(path: &Path) -> Result<Self, AudioError> {
        info!("Создание декодера для файла: {:?}", path);

        if !path.exists() {
            return Err(AudioError::DecodingError(format!(
                "Файл не найден: {:?}",
                path
            )));
        }

        let file = std::fs::File::open(path)
            .map_err(|e| AudioError::DecodingError(format!("Не удалось открыть файл: {}", e)))?;

        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.extension() {
            if let Some(ext_str) = ext.to_str() {
                hint.with_extension(ext_str);
            }
        }

        let format_opts = FormatOptions::default();
        let metadata_opts = MetadataOptions::default();

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &format_opts, &metadata_opts)
            .map_err(|e| {
                AudioError::DecodingError(format!("Не удалось определить формат: {}", e))
            })?;

        let format_reader = probed.format;

        let track = format_reader
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
            .ok_or_else(|| AudioError::DecodingError("Не найден аудио трек".to_string()))?;

        let track_id = track.id;

        let codec_params = &track.codec_params;
        let sample_rate = codec_params.sample_rate.unwrap_or(44100);
        let channels = codec_params.channels.map(|ch| ch.count()).unwrap_or(2);
        let total_frames = codec_params.n_frames;

        info!(
            "Трек найден: sample_rate={}, channels={}, frames={:?}",
            sample_rate, channels, total_frames
        );

        let decoder = symphonia::default::get_codecs()
            .make(codec_params, &DecoderOptions::default())
            .map_err(|e| AudioError::DecodingError(format!("Не удалось создать декодер: {}", e)))?;

        Ok(Self {
            format_reader,
            decoder,
            reader: None,
            track_id,
            sample_rate,
            channels,
            total_frames,
        })
    }

    pub fn from_reader(
        mut reader: Box<dyn MediaSource>,
        hint: Option<Hint>,
    ) -> Result<Self, AudioError> {
        let reader_clone = reader.clone_box();
        let mss = MediaSourceStream::new(reader, Default::default());
        let mut format_opts = FormatOptions::default();
        format_opts.enable_gapless = true;
        format_opts.prebuild_seek_index = true;

        let metadata_opts = MetadataOptions::default();

        let hint = hint.unwrap_or(Hint::new());
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &format_opts, &metadata_opts)
            .map_err(|e| {
                AudioError::DecodingError(format!("Не удалось определить формат потока: {}", e))
            })?;

        let format_reader = probed.format;

        let track = format_reader
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
            .ok_or_else(|| {
                AudioError::DecodingError("Не найден аудио трек в потоке".to_string())
            })?;

        let track_id = track.id;

        let codec_params = &track.codec_params;
        let sample_rate = codec_params.sample_rate.unwrap_or(44100);
        let channels = codec_params.channels.map(|ch| ch.count()).unwrap_or(2);
        let total_frames = codec_params.n_frames;

        info!(
            "Chunked streaming трек готов: sample_rate={}, channels={}, frames={:?}, codec={:?}",
            sample_rate, channels, total_frames, codec_params.codec
        );

        let decoder = symphonia::default::get_codecs()
            .make(codec_params, &DecoderOptions::default())
            .map_err(|e| AudioError::DecodingError(format!("Не удалось создать декодер: {}", e)))?;

        Ok(Self {
            format_reader,
            decoder,
            reader: Some(reader_clone),
            track_id,
            sample_rate,
            channels,
            total_frames,
        })
    }

    pub fn seek(&mut self, time_secs: f64) -> Result<u64, AudioError> {
        debug!("Перемотка на {:.2} сек", time_secs);

        let ts = Time::from(time_secs);

        let seeked_to = self
            .format_reader
            .seek(
                SeekMode::Coarse,
                SeekTo::Time {
                    time: ts,
                    track_id: Some(self.track_id),
                },
            )
            .map_err(|e| AudioError::InProgress)?;
        self.decoder.reset();
        info!("Перемотка выполнена на позицию: {:?}", seeked_to.actual_ts);

        Ok(seeked_to.actual_ts)
    }

    pub fn duration(&self) -> Option<f64> {
        self.total_frames
            .map(|frames| frames as f64 / self.sample_rate as f64)
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    pub fn buffered_ranges(&self) -> Vec<(f64, f64)> {
        if let Some(duration) = self.duration() {
            vec![(0.0, duration)]
        } else {
            vec![]
        }
    }

    pub fn waveforms(&self) -> Vec<crate::audio::types::Waveform> {
        if let Some(ref reader) = self.reader {
            reader.get_waveforms()
        } else {
            vec![]
        }
    }

    pub fn decode_next_block(&mut self) -> Result<Option<Vec<f32>>, AudioError> {
        let packet = loop {
            match self.format_reader.next_packet() {
                Ok(packet) => {
                    if packet.track_id() == self.track_id {
                        break packet;
                    }
                }
                Err(Error::IoError(e)) => {
                    if e.kind() == std::io::ErrorKind::UnexpectedEof {
                        return Ok(None);
                    }
                    if e.kind() == std::io::ErrorKind::WouldBlock {
                        self.decoder.reset();
                        return Err(AudioError::InProgress);
                    }
                    return Err(AudioError::DecodingError(format!("IO Error: {}", e)));
                }
                Err(Error::ResetRequired) => {
                    debug!("Format reader requires reset, resetting decoder...");
                    self.decoder.reset();
                    // Continue loop to get next packet
                }
                Err(e) => {
                    return Err(AudioError::DecodingError(format!(
                        "Ошибка чтения пакета: {}",
                        e
                    )));
                }
            }
        };

        match self.decoder.decode(&packet) {
            Ok(decoded) => {
                let samples = Self::convert_to_f32(decoded)?;
                Ok(Some(samples))
            }
            Err(Error::ResetRequired) => {
                debug!("Decoder requires reset, resetting and skipping this packet.");
                self.decoder.reset();
                // We return an empty block instead of error to keep the stream alive
                // The next call will pick up the next packet.
                Ok(Some(vec![]))
            }
            Err(Error::DecodeError(e)) => {
                warn!("Ошибка декодирования: {}", e);
                Ok(Some(vec![]))
            }
            Err(e) => Err(AudioError::DecodingError(format!(
                "Критическая ошибка декодирования: {}",
                e
            ))),
        }
    }

    fn convert_to_f32(buffer: AudioBufferRef) -> Result<Vec<f32>, AudioError> {
        use symphonia::core::audio::SampleBuffer;

        let mut sample_buf = SampleBuffer::<f32>::new(buffer.capacity() as u64, *buffer.spec());

        sample_buf.copy_interleaved_ref(buffer);

        Ok(sample_buf.samples().to_vec())
    }
}

pub enum DecoderType {
    Standard(AudioDecoder),
    Hls(HlsAudioDecoder),
}

impl DecoderType {
    pub fn from_standard(decoder: AudioDecoder) -> Self {
        Self::Standard(decoder)
    }

    pub fn from_hls(decoder: HlsAudioDecoder) -> Self {
        Self::Hls(decoder)
    }

    pub fn decode_next_block(&mut self) -> Result<Option<Vec<f32>>, AudioError> {
        match self {
            Self::Standard(d) => d.decode_next_block(),
            Self::Hls(d) => d.decode_next_block(),
        }
    }

    pub fn seek(&mut self, time_secs: f64) -> Result<u64, AudioError> {
        match self {
            Self::Standard(d) => d.seek(time_secs),
            Self::Hls(d) => d.seek(time_secs),
        }
    }

    pub fn duration(&self) -> Option<f64> {
        match self {
            Self::Standard(d) => d.duration(),
            Self::Hls(d) => d.duration(),
        }
    }

    pub fn sample_rate(&self) -> u32 {
        match self {
            Self::Standard(d) => d.sample_rate(),
            Self::Hls(d) => d.sample_rate(),
        }
    }

    pub fn channels(&self) -> usize {
        match self {
            Self::Standard(d) => d.channels(),
            Self::Hls(d) => d.channels(),
        }
    }

    pub fn buffered_ranges(&self) -> Vec<(f64, f64)> {
        match self {
            Self::Standard(d) => d.buffered_ranges(),
            Self::Hls(d) => d.buffered_ranges(),
        }
    }

    pub fn waveforms(&self) -> Vec<crate::audio::types::Waveform> {
        match self {
            Self::Standard(d) => d.waveforms(),
            Self::Hls(d) => d.waveforms(),
        }
    }
}
