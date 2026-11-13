use crate::state::types::Waveform;
use std::io::Cursor;
use symphonia::core::audio::{AudioBufferRef, SampleBuffer};
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::errors::Error;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

const WAVEFORM_POINTS: usize = 50;

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
                    append_frame_energy(&decoded, &mut amplitudes);

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
            data: uniform_rms(&amplitudes, WAVEFORM_POINTS),
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

    // Decode the whole chunk: a packet limit would discard its tail.

    while let Ok(packet) = format.next_packet() {
        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                append_frame_energy(&decoded, &mut amplitudes);
            }
            Err(Error::IoError(_)) => break,
            Err(Error::DecodeError(_)) => continue,
            Err(_) => break,
        }
    }

    if amplitudes.is_empty() {
        return None;
    }

    Some(Waveform {
        data: uniform_rms(&amplitudes, WAVEFORM_POINTS),
        start_time: 0.0, // These will be set by the caller
        end_time: 0.0,
        byte_start: None,
        byte_end: None,
    })
}

// Store channel-averaged energy per frame, so packet sizes and sample formats do
// not affect the time axis. Averaging energy also preserves opposite-phase stereo.
fn append_frame_energy(decoded: &AudioBufferRef, energies: &mut Vec<f32>) {
    let channels = decoded.spec().channels.count();
    if channels == 0 || decoded.frames() == 0 {
        return;
    }
    let mut samples = SampleBuffer::<f32>::new(decoded.frames() as u64, *decoded.spec());
    samples.copy_interleaved_ref(decoded.clone());
    energies.extend(
        samples
            .samples()
            .chunks_exact(channels)
            .map(|frame| frame.iter().map(|&s| s * s).sum::<f32>() / channels as f32),
    );
}

// Each point represents an equal interval of the complete decoded chunk. Use
// every frame in the interval instead of picking samples that can miss transients.
fn uniform_rms(energies: &[f32], points: usize) -> Vec<f32> {
    let points = points.min(energies.len());
    (0..points)
        .map(|i| {
            let start = i * energies.len() / points;
            let end = (i + 1) * energies.len() / points;
            let energy: f64 = energies[start..end].iter().map(|&e| f64::from(e)).sum();
            (energy / (end - start) as f64).sqrt() as f32
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(samples: &[i16], channels: u16) -> Vec<u8> {
        let data_len = (samples.len() * 2) as u32;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&8000u32.to_le_bytes());
        bytes.extend_from_slice(&(8000 * u32::from(channels) * 2).to_le_bytes());
        bytes.extend_from_slice(&(channels * 2).to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn waveform_covers_the_end_of_a_long_chunk() {
        let mut samples = vec![0; 160_000];
        samples[80_000..].fill(16384);
        let waveform = analyze_waveform(&wav(&samples, 1)).expect("valid WAV");
        assert_eq!(waveform.data.len(), 50);
        assert!(waveform.data[..25].iter().all(|&a| a < 0.001));
        assert!(waveform.data[25..].iter().all(|&a| (a - 0.5).abs() < 0.001));
    }

    #[test]
    fn waveform_includes_energy_from_both_channels() {
        let samples: Vec<i16> = (0..8000).flat_map(|_| [0, 16384]).collect();
        let waveform = analyze_waveform(&wav(&samples, 2)).expect("valid stereo WAV");
        let expected = (0.125_f32).sqrt();
        assert_eq!(waveform.data.len(), WAVEFORM_POINTS);
        assert!(waveform.data.iter().all(|&a| (a - expected).abs() < 0.001));
    }

    #[test]
    fn stateful_waveform_covers_the_whole_segment() {
        let mut samples = vec![0; 160_000];
        samples[80_000..].fill(16384);
        let mut analyzer = StatefulWaveformAnalyzer::new(Box::new(Cursor::new(wav(&samples, 1))))
            .expect("valid WAV");
        let waveform = analyzer.analyze_segment(20.0).expect("segment waveform");
        assert_eq!(waveform.data.len(), WAVEFORM_POINTS);
        assert!(waveform.data[..25].iter().all(|&a| a < 0.001));
        assert!(waveform.data[25..].iter().all(|&a| (a - 0.5).abs() < 0.001));
    }

    #[test]
    fn short_and_empty_chunks_do_not_produce_empty_bins() {
        assert!(uniform_rms(&[], WAVEFORM_POINTS).is_empty());
        let waveform = analyze_waveform(&wav(&[0, 16384, 0], 1)).expect("short WAV");
        assert_eq!(waveform.data, vec![0.0, 0.5, 0.0]);
    }
}
