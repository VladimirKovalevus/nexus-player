use std::sync::Arc;

use nexus_player_backend::audio::hls_decoder::HlsAudioDecoder;
use streaming::{buffers::hls::HlsBuffer, multi_downloader::source::HlsSource};

#[test]
fn opus_starts_with_only_startup_segments_buffered() {
    let data = include_bytes!("../../vendor/symphonia-adapters/assets/sample.opus");
    let chunks: Vec<_> = data.chunks(data.len().div_ceil(10)).collect();
    let buffer = Arc::new(HlsBuffer::new(false));
    buffer.init(chunks.len(), vec![1.38456; chunks.len()], None);
    buffer.set_container("opus".into());
    buffer.set_bitrate(408_000);
    for i in [0, 1, 2, chunks.len() - 2, chunks.len() - 1] {
        buffer.write_chunk(i as u64, chunks[i].to_vec());
    }
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let source = HlsSource::new(buffer.clone(), 1, Arc::new(tx));
    let mut decoder = HlsAudioDecoder::new(source, 48_000).expect("probe buffered Opus start");
    let mut sample_count = 0;
    let mut audible = false;
    for _ in 0..100 {
        let samples = decoder
            .decode_next_block()
            .expect("decode Opus")
            .expect("audio before EOF");
        sample_count += samples.len();
        if samples.iter().any(|s| s.abs() > 0.0001) {
            audible = true;
            break;
        }
    }
    assert!(
        audible,
        "no audible samples decoded from the buffered Opus start"
    );

    // Finish downloading after playback has started. Every byte must still be
    // decoded across HLS segment boundaries, including the end of the track.
    for (i, chunk) in chunks.iter().enumerate() {
        if !buffer.has_segment(i) {
            buffer.write_chunk(i as u64, chunk.to_vec());
        }
    }
    while let Some(samples) = decoder.decode_next_block().expect("decode complete Opus") {
        sample_count += samples.len();
    }
    let seconds = sample_count as f64 / decoder.channels() as f64 / 48_000.0;
    assert!(
        (13.8..13.9).contains(&seconds),
        "decoded {seconds}s instead of the full 13.85s track"
    );
}
