use std::time::{Duration, Instant};

use streaming::state::manager::DownloadManager;

fn pick_track_page_url(items: Vec<bandcamp::SearchResultItem>) -> Option<String> {
    for item in items {
        if let bandcamp::SearchResultItem::Track(track) = item {
            return Some(track.url.item_url);
        }
    }
    None
}

async fn resolve_stream_url(track_page_url: &str) -> Result<String, Box<dyn std::error::Error>> {
    let album = bandcamp::track_from_url(track_page_url).await?;
    let first_track = album
        .tracks
        .first()
        .ok_or("No tracks found in Bandcamp track page response")?;

    let stream_url = first_track
        .streaming_url
        .values()
        .next()
        .cloned()
        .ok_or("No stream URL in Bandcamp track metadata")?;

    Ok(stream_url)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let query = "cage of creation";
    println!("Searching Bandcamp for: {query}");

    let search_results = bandcamp::search(query).await?;
    let Some(track_page_url) = pick_track_page_url(search_results) else {
        return Err("No track results found for query".into());
    };
    println!("Track page URL: {track_page_url}");

    let stream_url = resolve_stream_url(&track_page_url).await?;
    println!("Resolved stream URL: {stream_url}");

    let manager = DownloadManager::new_default();
    let track_id = 1_u64;
    let source = manager
        .add_download_task(&stream_url, track_id, None)
        .await?;
    source.prepare().await?;
    println!("Download started, waiting for completion...");

    let timeout = Duration::from_secs(180);
    let started = Instant::now();
    loop {
        if started.elapsed() > timeout {
            return Err("Timeout waiting for full download".into());
        }

        let tasks = manager.list_tasks().await;
        if let Some(task) = tasks.iter().find(|t| t.track_id == track_id) {
            if task.state == "Complete" {
                break;
            }
            if task.state.starts_with("Failed:") || task.state == "Cancelled" {
                return Err(format!("Download failed: {}", task.state).into());
            }
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    source.save_to_file("test.mp3")?;
    println!("Saved: test.mp3");

    Ok(())
}
