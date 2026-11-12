//! Download HLS with `streaming` and play it through the application's audio engine.
//!
//! python3 scripts/serve_hls_opus.py
//! cargo run -p nexus-player-backend --example hls_play
//! cargo run -p nexus-player-backend --example hls_play -- --decode-only
//! cargo run -p nexus-player-backend --example hls_play -- https://host/playlist.m3u8
//!
//! This example lives in the backend because `streaming` downloads media; the
//! backend provides HlsAudioDecoder and PlaybackEngine for decoding and output.

use std::time::{Duration, Instant};

use nexus_player_backend::audio::{hls_decoder::HlsAudioDecoder, playback::PlaybackEngine};
use streaming::{
    multi_downloader::source::{HlsSource, Source},
    state::manager::DownloadManager,
};

const DEFAULT_URL: &str = "http://127.0.0.1:8765/playlist.m3u8";
const TIMEOUT: Duration = Duration::from_secs(60);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut url = None;
    let mut decode_only = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--decode-only" => decode_only = true,
            "--help" | "-h" => {
                println!("Usage: hls_play [HLS_URL] [--decode-only]\nDefault: {DEFAULT_URL}");
                return Ok(());
            }
            _ if !arg.starts_with('-') && url.is_none() => url = Some(arg),
            _ => return Err(format!("Unknown argument: {arg}").into()),
        }
    }
    let url = url.as_deref().unwrap_or(DEFAULT_URL);
    println!("Downloading HLS via streaming::DownloadManager: {url}");
    let manager = DownloadManager::new_default();
    let source = tokio::time::timeout(TIMEOUT, manager.add_download_task(url, 1, None)).await??;
    let Source::Hls(source) = source else {
        return Err("URL did not resolve to an HLS stream".into());
    };

    // Keep the manager alive until playback ends. Wait for the complete download
    // so this example also works with short playlists and slow connections.
    let started = Instant::now();
    loop {
        let task = manager
            .list_tasks()
            .await
            .into_iter()
            .find(|task| task.track_id == 1)
            .ok_or("Download task disappeared")?;
        if task.state == "Complete" {
            println!(
                "Downloaded {} bytes; container={:?}; segments={}; duration={:.3}s",
                task.byte_size,
                source.container(),
                source.durations().len(),
                source.total_duration_ms() as f64 / 1000.0,
            );
            break;
        }
        if task.state.starts_with("Failed") || task.state == "Cancelled" {
            return Err(format!("Download failed: {}", task.state).into());
        }
        if started.elapsed() > TIMEOUT {
            return Err("Timed out waiting for the complete HLS download".into());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    if decode_only {
        check_decode(source)?;
    } else {
        play(source).await?;
    }
    Ok(())
}

fn check_decode(source: HlsSource) -> Result<(), Box<dyn std::error::Error>> {
    let mut decoder = HlsAudioDecoder::new(source, 48_000)?;
    let expected = decoder.duration().ok_or("Missing HLS duration")?;
    let channels = decoder.channels();
    let started = Instant::now();
    let mut samples = 0usize;
    let mut peak = 0.0f32;
    while let Some(block) = decoder.decode_next_block()? {
        samples += block.len();
        peak = block
            .iter()
            .fold(peak, |peak, sample| peak.max(sample.abs()));
        if started.elapsed() > TIMEOUT {
            return Err("Timed out decoding the downloaded track".into());
        }
    }
    let seconds = samples as f64 / channels as f64 / 48_000.0;
    if samples == 0 || (seconds - expected).abs() > 0.25 {
        return Err(
            format!("Incomplete audio: decoded {seconds:.3}s, playlist {expected:.3}s").into(),
        );
    }
    println!("Decode OK: {samples} samples, {channels} channels, {seconds:.3}s, peak={peak:.4}");
    Ok(())
}

async fn play(source: HlsSource) -> Result<(), Box<dyn std::error::Error>> {
    let mut player = PlaybackEngine::new()?;
    player.play_hls(source)?;
    let duration = player.duration().ok_or("Missing HLS duration")?;
    let started = Instant::now();
    println!("Playing on the default audio device ({duration:.3?})...");
    let result = loop {
        let position = player.position();
        if position + Duration::from_millis(50) >= duration {
            // Let the device finish its last output buffer before stopping.
            tokio::time::sleep(Duration::from_millis(200)).await;
            println!("Playback finished at {position:.3?}");
            break Ok(());
        }
        if !player.is_playing() || player.needs_recreation() {
            break Err("Audio output stopped before the track finished".into());
        }
        if started.elapsed() > duration + TIMEOUT {
            break Err("Playback timed out (decoder or output stalled)".into());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    player.stop();
    result
}
