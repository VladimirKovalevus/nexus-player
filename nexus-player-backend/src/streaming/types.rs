use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

use crate::{audio::types::Track, library::TrackSource, playlist::playlist::Playlist};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveOptions {
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum ContentType {
    Track,

    Playlist,

    Album,

    Channel,
}

impl ContentType {
    pub fn is_collection(&self) -> bool {
        !matches!(self, ContentType::Track)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum StreamingService {
    SoundCloud,
    Bandcamp,
    Vk,
}

impl std::fmt::Display for StreamingService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SoundCloud => write!(f, "soundcloud"),
            Self::Bandcamp => write!(f, "bandcamp"),
            Self::Vk => write!(f, "vk"),
        }
    }
}

impl std::str::FromStr for StreamingService {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "soundcloud" => Ok(Self::SoundCloud),
            "bandcamp" => Ok(Self::Bandcamp),
            "vk" => Ok(Self::Vk),
            _ => Err(()),
        }
    }
}

impl StreamingService {
    pub fn name(&self) -> &'static str {
        match self {
            Self::SoundCloud => "SoundCloud",
            Self::Bandcamp => "Bandcamp",
            Self::Vk => "VK",
        }
    }

    pub fn search_prefix(&self, limit: usize) -> String {
        match self {
            Self::SoundCloud => format!("scsearch{}", limit),
            Self::Bandcamp => String::new(),
            Self::Vk => String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum SearchResult {
    Track(TrackInfo),

    Playlist(PlaylistInfo),

    Album(AlbumInfo),

    Channel(ChannelInfo),
}

impl SearchResult {
    pub fn content_type(&self) -> ContentType {
        match self {
            SearchResult::Track(_) => ContentType::Track,
            SearchResult::Playlist(_) => ContentType::Playlist,
            SearchResult::Album(_) => ContentType::Album,
            SearchResult::Channel(_) => ContentType::Channel,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TrackInfo {
    pub source: TrackSource,

    pub title: String,

    pub artist: String,

    pub duration: Option<std::time::Duration>,

    pub thumbnail: Option<String>,

    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PlaylistInfo {
    pub id: String,

    pub url: String,

    pub service: StreamingService,

    pub title: String,

    pub creator: String,

    pub tracks: Vec<TrackInfo>,

    pub track_count: usize,

    pub thumbnail: Option<String>,

    pub description: Option<String>,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AudioQuality {
    Best,

    High,

    Medium,

    Low,
}

impl Default for AudioQuality {
    fn default() -> Self {
        AudioQuality::High
    }
}

impl AudioQuality {
    pub fn approximate_bitrate(&self) -> u32 {
        match self {
            AudioQuality::Best => 320,
            AudioQuality::High => 256,
            AudioQuality::Medium => 128,
            AudioQuality::Low => 64,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AlbumInfo {
    pub id: String,

    pub tracks: Vec<TrackInfo>,

    pub url: String,

    pub service: StreamingService,

    pub title: String,

    pub artist: String,

    pub track_count: usize,

    pub year: Option<u32>,

    pub thumbnail: Option<String>,

    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ChannelInfo {
    pub id: String,

    pub url: String,

    pub service: StreamingService,

    pub name: String,

    pub subscriber_count: Option<u64>,

    pub track_count: Option<usize>,

    pub thumbnail: Option<String>,

    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StreamMetadata {
    pub title: Option<String>,

    pub artist: Option<String>,

    pub album: Option<String>,

    pub duration: Option<std::time::Duration>,

    pub artwork_url: Option<String>,

    pub sample_rate: Option<u32>,

    pub channels: Option<usize>,
}

impl Default for StreamMetadata {
    fn default() -> Self {
        Self {
            title: None,
            artist: None,
            album: None,
            duration: None,
            artwork_url: None,
            sample_rate: None,
            channels: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ResolvedContent {
    Track(Track),
    Playlist(Playlist),
}

impl From<&PlaylistInfo> for Playlist {
    fn from(row: &PlaylistInfo) -> Self {
        Playlist {
            title: row.title.clone(),
            artwork: row.thumbnail.clone(),
            description: row.description.clone(),
            tracks: row.tracks.iter().map(|e| e.into()).collect(),
        }
    }
}
impl From<PlaylistInfo> for Playlist {
    fn from(row: PlaylistInfo) -> Self {
        Playlist {
            title: row.title,
            artwork: row.thumbnail,
            description: row.description,
            tracks: row.tracks.into_iter().map(|e| e.into()).collect(),
        }
    }
}
fn track_id_from_source(source: &crate::audio::types::TrackSource) -> usize {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    source.as_str().hash(&mut hasher);
    hasher.finish() as usize
}

impl From<TrackInfo> for Track {
    fn from(row: TrackInfo) -> Self {
        let id = track_id_from_source(&row.source);
        Track {
            id,
            title: row.title,
            artist: Some(row.artist),
            album: None,
            duration: row.duration,
            artwork_url: row.thumbnail,
            format: None,
            source: row.source,
            state: crate::audio::types::TrackState::Streaming,
            liked: false,
        }
    }
}
impl From<&TrackInfo> for Track {
    fn from(row: &TrackInfo) -> Self {
        let id = track_id_from_source(&row.source);
        Track {
            id,
            title: row.title.clone(),
            artist: Some(row.artist.clone()),
            album: None,
            duration: row.duration,
            artwork_url: row.thumbnail.clone(),
            format: None,
            source: row.source.clone(),
            state: crate::audio::types::TrackState::Streaming,
            liked: false,
        }
    }
}
