use crate::state::types::Waveform;
use std::io::Cursor;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::errors::Error;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub struct StatefulWaveformAnalyzer {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    sample_rate: u32,
    current_time: f64,
}

impl StatefulWaveformAnalyzer {
    pub fn new(source: Box<dyn MediaSource>) -> Option<Self> {
        let mss = MediaSourceStream::new(source, Default::default());
        let hint = Hint::new();
        let format_opts = FormatOptions::default();
        let metadata_opts = MetadataOptions::default();
        let decoder_opts = DecoderOptions::default();

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &format_opts, &metadata_opts)
            .ok()?;

        let format_reader = probed.format;

        let track = format_reader
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)?;

        let track_id = track.id;
        let sample_rate = track.codec_params.sample_rate.unwrap_or(48000);

        let decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &decoder_opts)
            .ok()?;

        Some(Self {
            format: format_reader,
            decoder,
            track_id,
            sample_rate,
            current_time: 0.0,
        })
    }

    pub fn analyze_segment(&mut self, duration: f64) -> Option<Waveform> {
        let mut amplitudes = Vec::new();
        let target_end_time = self.current_time + duration;

        // Safety break to prevent infinite loops if something goes wrong with timestamps
        let mut packets_processed = 0;
        let max_packets_per_segment = 1000;

        while self.current_time < target_end_time && packets_processed < max_packets_per_segment {
            let packet = match self.format.next_packet() {
                Ok(packet) => packet,
                Err(_) => break,
            };

            if packet.track_id() != self.track_id {
                continue;
            }

            match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    let rms = calculate_rms(&decoded);
                    amplitudes.push(rms);

                    // Update current time based on decoded frames
                    let frames = decoded.frames();
                    self.current_time += frames as f64 / self.sample_rate as f64;
                }
                Err(Error::IoError(_)) => break,
                Err(Error::DecodeError(_)) => continue,
                Err(_) => break,
            }

            packets_processed += 1;
        }

        // Ensure we don't get stuck if segment was shorter or packet durations were slightly off
        if self.current_time < target_end_time && packets_processed > 0 {
            // If we processed some data but didn't reach target time (e.g. end of segment reached in stream)
            // we still want to progress to next segment in the next call.
            // But we only force it if we are very close or if we suspect we hit the next segment's data.
        }

        // If we didn't process anything, we might be stuck.
        if amplitudes.is_empty() {
            // We can try to advance time anyway to not block the loop in process_waveforms
            self.current_time = target_end_time;
            return None;
        }

        Some(Waveform {
            data: amplitudes,
            start_time: 0.0,
            end_time: 0.0,
            byte_start: None,
            byte_end: None,
        })
    }
}

pub fn analyze_waveform(data: &[u8]) -> Option<Waveform> {
    let cursor = Cursor::new(data.to_vec());
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

    let hint = Hint::new();
    let format_opts = FormatOptions::default();
    let metadata_opts = MetadataOptions::default();
    let decoder_opts = DecoderOptions::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &format_opts, &metadata_opts)
        .ok()?;

    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)?;

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &decoder_opts)
        .ok()?;

    let track_id = track.id;
    let mut amplitudes = Vec::new();

    // We only decode a limited number of packets to keep it fast
    let max_packets = 50;
    let mut packets_count = 0;

    while let Ok(packet) = format.next_packet() {
        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                let rms = calculate_rms(&decoded);
                amplitudes.push(rms);
            }
            Err(Error::IoError(_)) => break,
            Err(Error::DecodeError(_)) => continue,
            Err(_) => break,
        }

        packets_count += 1;
        if packets_count >= max_packets {
            break;
        }
    }

    if amplitudes.is_empty() {
        return None;
    }

    // Return a simplified waveform (e.g. average RMS for the chunk)
    // Or just the list of RMS values if we want more detail.
    Some(Waveform {
        data: amplitudes,
        start_time: 0.0, // These will be set by the caller
        end_time: 0.0,
        byte_start: None,
        byte_end: None,
    })
}

fn calculate_rms(decoded: &AudioBufferRef) -> f32 {
    match decoded {
        AudioBufferRef::F32(buf) => {
            let samples = buf.chan(0);
            if samples.is_empty() {
                return 0.0;
            }
            let sum_sq: f32 = samples.iter().map(|&s| s * s).sum();
            (sum_sq / samples.len() as f32).sqrt()
        }
        AudioBufferRef::U8(buf) => {
            let samples = buf.chan(0);
            if samples.is_empty() {
                return 0.0;
            }
            let sum_sq: f32 = samples
                .iter()
                .map(|&s| {
                    let f = (s as f32 - 128.0) / 128.0;
                    f * f
                })
                .sum();
            (sum_sq / samples.len() as f32).sqrt()
        }
        AudioBufferRef::U16(buf) => {
            let samples = buf.chan(0);
            if samples.is_empty() {
                return 0.0;
            }
            let sum_sq: f32 = samples
                .iter()
                .map(|&s| {
                    let f = (s as f32 - 32768.0) / 32768.0;
                    f * f
                })
                .sum();
            (sum_sq / samples.len() as f32).sqrt()
        }
        AudioBufferRef::S8(buf) => {
            let samples = buf.chan(0);
            if samples.is_empty() {
                return 0.0;
            }
            let sum_sq: f32 = samples
                .iter()
                .map(|&s| {
                    let f = s as f32 / 128.0;
                    f * f
                })
                .sum();
            (sum_sq / samples.len() as f32).sqrt()
        }
        AudioBufferRef::S16(buf) => {
            let samples = buf.chan(0);
            if samples.is_empty() {
                return 0.0;
            }
            let sum_sq: f32 = samples
                .iter()
                .map(|&s| {
                    let f = s as f32 / 32768.0;
                    f * f
                })
                .sum();
            (sum_sq / samples.len() as f32).sqrt()
        }
        _ => 0.0, // Other formats not implemented for brevity
    }
}
