use anyhow::{Context, Result};
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::mp4;
use lofty::probe::Probe;
use lofty::tag::Accessor;
use std::path::Path;

use super::models::{Metadata, Track};
use super::types::{AudioInfo, CacheStatus, CodecFormat, TrackId, TrackSource};

pub struct ParsedMetadata {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub year: Option<u32>,
    pub genre: Option<String>,
    pub track_number: Option<u32>,
    pub audio_info: Option<AudioInfo>,
    pub artwork_data: Option<Vec<u8>>,
}

pub fn parse_from_path(path: &Path) -> Result<ParsedMetadata> {
    let tagged_file = Probe::open(path)
        .context("Failed to open audio file")?
        .read()
        .context("Failed to read audio file metadata")?;

    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag());

    let mut result = ParsedMetadata {
        title: "Unknown".to_string(),
        artist: None,
        album: None,
        album_artist: None,
        year: None,
        genre: None,
        track_number: None,
        audio_info: None,
        artwork_data: None,
    };
    let properties = tagged_file.properties();
    if let Some(tags) = tag {
        result.title = tags
            .title()
            .map(|s| s.to_string())
            .unwrap_or("Unknown".to_string());
        result.artist = tags.artist().map(|s| s.to_string());
        result.album = tags.album().map(|s| s.to_string());
        result.genre = tags.genre().map(|s| s.to_string());
        result.year = tags.year();
        result.track_number = tags.track();
        result.genre = tags.genre().map(|s| s.to_string());
        if let Some(picture) = tags.pictures().first() {
            result.artwork_data = Some(picture.data().to_vec());
            // TODO parse mime
            // picture
            //     .mime_type()
            //     .map(|m| m.to_string())
            //     .unwrap_or_else(|| "image/jpeg".to_string()),
        }
    };
    let properties = tagged_file.properties();
    let audio_info = AudioInfo {
        duration_secs: Some(properties.duration().as_secs_f64()),
        bitrate: properties.audio_bitrate(),
        sample_rate: properties.sample_rate(),
        channels: properties.channels(),
        format: CodecFormat::Mp3,
    };
    result.audio_info = Some(audio_info);

    Ok(result)
}

pub fn parse_from_buffer(buffer: &[u8], file_extension: &str) -> Result<ParsedMetadata> {
    let tagged_file = Probe::new(std::io::Cursor::new(buffer))
        .guess_file_type()
        .context("Failed to guess file type from buffer")?
        .read()
        .context("Failed to read audio metadata from buffer")?;

    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag());

    let mut result = ParsedMetadata {
        title: "Unknown".to_string(),
        artist: None,
        album: None,
        album_artist: None,
        year: None,
        genre: None,
        track_number: None,
        audio_info: None,
        artwork_data: None,
    };
    let properties = tagged_file.properties();
    if let Some(tags) = tag {
        result.title = tags
            .title()
            .map(|s| s.to_string())
            .unwrap_or("Unknown".to_string());
        result.artist = tags.artist().map(|s| s.to_string());
        result.album = tags.album().map(|s| s.to_string());
        result.genre = tags.genre().map(|s| s.to_string());
        result.year = tags.year();
        result.track_number = tags.track();
        result.genre = tags.genre().map(|s| s.to_string());
        if let Some(picture) = tags.pictures().first() {
            result.artwork_data = Some(picture.data().to_vec());
            // TODO parse mime
            // picture
            //     .mime_type()
            //     .map(|m| m.to_string())
            //     .unwrap_or_else(|| "image/jpeg".to_string()),
        }
    };
    let properties = tagged_file.properties();
    let audio_info = AudioInfo {
        duration_secs: Some(properties.duration().as_secs_f64()),
        bitrate: properties.audio_bitrate(),
        sample_rate: properties.sample_rate(),
        channels: properties.channels(),
        format: CodecFormat::Mp3,
    };
    result.audio_info = Some(audio_info);

    Ok(result)
}

pub fn create_track_from_parsed(path: std::path::PathBuf, parsed: ParsedMetadata) -> Track {
    Track {
        id: TrackId(0),
        metadata: Metadata {
            title: parsed.title,
            description: None,
            artwork: None,
        },
        artist: parsed.artist,
        album: parsed.album,
        album_artist: parsed.album_artist,
        year: parsed.year,
        genre: parsed.genre,
        track_number: parsed.track_number,
        source: TrackSource::Local(path),
        cache: CacheStatus::None,
        audio_info: parsed.audio_info,
        play_count: 0,
        last_played: None,
        added_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        liked: false,
        liked_position: None,
        unavailable: false,
    }
}
