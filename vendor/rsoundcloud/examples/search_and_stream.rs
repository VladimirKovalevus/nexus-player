use rsoundcloud::{CollectionParams, ResourceId, SearchApi, SoundCloudClient, TracksApi};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Usage: cargo run --example search_and_stream <client_id> <query>
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: cargo run --example search_and_stream <client_id> [query]");
        return Ok(());
    }

    let client_id = &args[1];
    let query = args.get(2).map(|s| s.as_str()).unwrap_or("void of love");

    println!("Initializing SoundCloud client with client_id: {}", client_id);
    let client = SoundCloudClient::with_client_id(client_id.to_string());

    println!("Searching for: '{}'...", query);
    let search_results = client
        .search_tracks(query.to_string(), CollectionParams::new(Some(5), None))
        .await?;

    if search_results.is_empty() {
        println!("No tracks found for: {}", query);
        return Ok(());
    }

    let first_track = &search_results[0];
    println!(
        "Found track: '{}' by '{}'",
        first_track.track.title, first_track.user.username
    );
    println!("Permalink: {}", first_track.track.permalink_url);

    println!("Fetching full track metadata to get media info...");
    let full_track = client
        .get_track(ResourceId::Url(first_track.track.permalink_url.clone()))
        .await?;

    println!("Available transcodings:");
    for t in &full_track.track.media.transcodings {
        println!(" - protocol: {}, mime_type: {}, url: {}", t.format.protocol, t.format.mime_type, t.url);
    }

    // Try to get stream URL for the first HLS transcoding (preferred by nexus-player)
    let hls_transcoding = full_track
        .track
        .media
        .transcodings
        .iter()
        .find(|t| t.format.protocol == "hls")
        .or_else(|| full_track.track.media.transcodings.first());

    if let Some(t) = hls_transcoding {
        println!("Attempting to resolve stream URL for transcoding: {} ({})", t.format.protocol, t.format.mime_type);
        match client.get_stream_url(&t.url).await {
            Ok(stream_url) => {
                println!("SUCCESS! Stream URL: {}", stream_url);
            }
            Err(e) => {
                eprintln!("FAILED to get stream URL: {}", e);
            }
        }
    } else {
        println!("No transcodings found for track.");
    }

    Ok(())
}
