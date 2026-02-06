use chrono::Utc;

use crate::audio::types as audio;
use crate::library::models::{Metadata, Track as LibraryTrack};
use crate::library::types::{AudioInfo, CacheStatus, CodecFormat, TrackId, TrackSource};

impl From<audio::Track> for LibraryTrack {
    fn from(track: audio::Track) -> Self {
        let audio_info = track.duration.map(|duration| AudioInfo {
            format: CodecFormat::Other,
            duration_secs: Some(duration.as_secs_f64()),
            sample_rate: None,
            channels: None,
            bitrate: None,
        });

        let cache = match &track.source {
            TrackSource::Cached { cache_path, .. } => CacheStatus::Cached {
                path: cache_path.clone(),
                downloaded_at: Utc::now(),
            },
            _ => CacheStatus::None,
        };

        let artwork = track.artwork_url.map(|url| crate::library::types::Artwork {
            id: crate::library::types::ArtworkId(0),
            source: crate::library::types::ArtworkSource::Url { url },
            width: None,
            height: None,
            created_at: Utc::now(),
        });

        LibraryTrack {
            id: TrackId(0),
            metadata: Metadata {
                title: track.title,
                description: None,
                artwork,
            },
            artist: track.artist,
            album: track.album,
            album_artist: None,
            year: None,
            genre: None,
            track_number: None,
            source: track.source,
            cache,
            audio_info,
            play_count: 0,
            last_played: None,
            added_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}
