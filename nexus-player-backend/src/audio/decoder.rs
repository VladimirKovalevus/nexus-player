use crate::audio::hls_decoder::HlsAudioDecoder;
use crate::audio::types::MediaSource;
use crate::error::AudioError;
use resampler::{ResamplerFir, SampleRate};
use symphonia::core::audio::AudioBufferRef;
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::errors::Error;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
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
    resampler: Option<ResamplerFir>,
    output_sample_rate: u32,
}

impl AudioDecoder {
    pub fn new(path: &std::path::Path, output_sample_rate: u32) -> Result<Self, AudioError> {
        info!("Creating decoder for file: {:?}", path);

        if !path.exists() {
            return Err(AudioError::DecodingError(format!(
                "File not found: {:?}",
                path
            )));
        }

        let file = std::fs::File::open(path)
            .map_err(|e| AudioError::DecodingError(format!("Failed to open file: {}", e)))?;

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
            .map_err(|e| AudioError::DecodingError(format!("Failed to detect format: {}", e)))?;

        let format_reader = probed.format;

        let track = format_reader
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
            .ok_or_else(|| AudioError::DecodingError("No audio track found".to_string()))?;

        let track_id = track.id;

        let codec_params = &track.codec_params;
        let sample_rate = codec_params.sample_rate.unwrap_or(44100);
        let channels = codec_params.channels.map(|ch| ch.count()).unwrap_or(2);
        let total_frames = codec_params.n_frames;

        info!(
            "Track found: sample_rate={}, channels={}, frames={:?}",
            sample_rate, channels, total_frames
        );

        let decoder = symphonia::default::get_codecs()
            .make(codec_params, &DecoderOptions::default())
            .map_err(|e| AudioError::DecodingError(format!("Failed to create decoder: {}", e)))?;

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

        Ok(Self {
            format_reader,
            decoder,
            reader: None,
            track_id,
            sample_rate,
            channels,
            total_frames,
            resampler,
            output_sample_rate,
        })
    }

    pub fn from_reader(
        mut reader: Box<dyn MediaSource>,
        hint: Option<Hint>,
        output_sample_rate: u32,
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
                AudioError::DecodingError(format!("Failed to detect stream format: {}", e))
            })?;

        let format_reader = probed.format;

        let track = format_reader
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
            .ok_or_else(|| {
                AudioError::DecodingError("No audio track found in the stream".to_string())
            })?;

        let track_id = track.id;

        let codec_params = &track.codec_params;
        let sample_rate = codec_params.sample_rate.unwrap_or(44100);
        let channels = codec_params.channels.map(|ch| ch.count()).unwrap_or(2);
        let total_frames = codec_params.n_frames;

        info!(
            "Chunked streaming track ready: sample_rate={}, channels={}, frames={:?}, codec={:?}",
            sample_rate, channels, total_frames, codec_params.codec
        );

        let decoder = symphonia::default::get_codecs()
            .make(codec_params, &DecoderOptions::default())
            .map_err(|e| AudioError::DecodingError(format!("Failed to create decoder: {}", e)))?;

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

        Ok(Self {
            format_reader,
            decoder,
            reader: Some(reader_clone),
            track_id,
            sample_rate,
            channels,
            total_frames,
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

    pub fn seek(&mut self, time_secs: f64) -> Result<u64, AudioError> {
        debug!("Seeking to {:.2} s", time_secs);

        let ts = Time::from(time_secs);

        match self.format_reader.seek(
            SeekMode::Coarse,
            SeekTo::Time {
                time: ts,
                track_id: Some(self.track_id),
            },
        ) {
            Ok(seeked_to) => {
                self.decoder.reset();
                // Recreate resampler to clear history
                if self.sample_rate != self.output_sample_rate {
                    self.resampler = Some(ResamplerFir::new(
                        self.channels,
                        SampleRate::try_from(self.sample_rate).unwrap_or(SampleRate::Hz44100),
                        SampleRate::try_from(self.output_sample_rate)
                            .unwrap_or(SampleRate::Hz44100),
                        resampler::Latency::Sample64,
                        resampler::Attenuation::Db120,
                    ));
                }
                info!("Seek completed at position: {:?}", seeked_to.actual_ts);
                Ok(seeked_to.actual_ts)
            }
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::WouldBlock =>
            {
                // Even on WouldBlock, reset the decoder because the stream position will change
                self.decoder.reset();
                Err(AudioError::InProgress)
            }
            Err(e) => Err(AudioError::DecodingError(format!("Seek error: {}", e))),
        }
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
            let mut waveforms = reader.get_waveforms();
            if let Some(duration) = self.duration() {
                if let Some(total_bytes) = reader.byte_len() {
                    if total_bytes > 0 {
                        for w in waveforms.iter_mut() {
                            if w.end_time == 0.0 && w.byte_end.is_some() {
                                w.start_time = (w.byte_start.unwrap_or(0) as f64
                                    / total_bytes as f64)
                                    * duration;
                                w.end_time =
                                    (w.byte_end.unwrap() as f64 / total_bytes as f64) * duration;
                            }
                        }
                    }
                }
            }
            waveforms
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
                        "Packet read error: {}",
                        e
                    )));
                }
            }
        };

        match self.decoder.decode(&packet) {
            Ok(decoded) => {
                let samples = Self::convert_to_f32(decoded)?;

                if let Some(ref mut resampler) = self.resampler {
                    let ratio = self.output_sample_rate as f32 / self.sample_rate as f32;
                    let out_capacity = ((samples.len() as f32 / self.channels as f32) * ratio)
                        .ceil() as usize
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
            Err(Error::ResetRequired) => {
                debug!("Decoder requires reset, resetting and skipping this packet.");
                self.decoder.reset();
                Ok(Some(vec![]))
            }
            Err(Error::DecodeError(e)) => {
                warn!("Decoding error: {}", e);
                Ok(Some(vec![]))
            }
            Err(e) => Err(AudioError::DecodingError(format!(
                "Fatal decoding error: {}",
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

    pub fn set_output_sample_rate(&mut self, output_sample_rate: u32) {
        match self {
            Self::Standard(d) => d.set_output_sample_rate(output_sample_rate),
            Self::Hls(d) => d.set_output_sample_rate(output_sample_rate),
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
