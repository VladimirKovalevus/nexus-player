use rsoundcloud::{PlaylistsApi, ResourceId, SoundCloudClient, TracksApi};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get playlist ID or URL from command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <playlist_id_or_url>", args[0]);
        std::process::exit(1);
    }

    let input = &args[1];

    // Initialize SoundCloud client
    let client = SoundCloudClient::default().await?;

    // Determine ResourceId type
    let resource_id = if let Ok(id) = input.parse::<u64>() {
        ResourceId::Id(id)
    } else if input.starts_with("http") {
        ResourceId::Url(input.to_string())
    } else {
        ResourceId::Uri(input.to_string())
    };

    println!("Fetching tracks for playlist: {}", input);

    // Get playlist tracks
    match client.get_playlist_tracks(resource_id).await {
        Ok(tracks) => {
            println!("Successfully fetched {} tracks:", tracks.len());
            for (i, track) in tracks.iter().enumerate() {
                println!("{}. {} - {}", i + 1, track.user.username, track.track.title);
                println!("   Transcodings:");
                for (j, transcoding) in track.track.media.transcodings.iter().enumerate() {
                    let stream_url = match client.get_stream_url(&transcoding.url).await {
                        Ok(url) => url,
                        Err(e) => format!("Error: {}", e),
                    };
                    println!(
                        "     [{}] protocol: {}, mime: {}, stream_url: {}",
                        j,
                        transcoding.format.protocol,
                        transcoding.format.mime_type,
                        stream_url
                    );
                }
                println!();
            }
        }
        Err(e) => {
            eprintln!("Error fetching playlist tracks: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}
