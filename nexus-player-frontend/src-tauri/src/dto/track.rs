use nexus_player_backend::audio::types::TrackState;
use nexus_player_backend::audio::types::{self as audio};
use nexus_player_backend::library::models;
use serde::{Deserialize, Serialize};

use super::{hash_str_to_i64, CacheStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackDto {
    pub id: i64,
    pub title: String,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default)]
    pub duration_secs: Option<f64>,
    #[serde(default)]
    pub artwork_url: Option<String>,
    #[serde(default)]
    pub play_count: u64,
    pub liked: bool,
    pub state: TrackStateDto,
    #[serde(default)]
    pub source: Option<TrackSourceDto>,
    #[serde(default)]
    pub unavailable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedAlbumDto {
    pub id: i64,
    pub title: String,
    pub artist: Option<String>,
    pub artwork_url: Option<String>,
    pub cached_track_count: u64,
}

impl From<models::CachedAlbumSummary> for CachedAlbumDto {
    fn from(summary: models::CachedAlbumSummary) -> Self {
        let artwork_url = summary.artwork.as_ref().and_then(|art| match &art.source {
            nexus_player_backend::library::types::ArtworkSource::Url { url } => Some(url.clone()),
            _ => None,
        });

        Self {
            id: summary.id.0,
            title: summary.title,
            artist: summary.artist,
            artwork_url,
            cached_track_count: summary.cached_track_count,
        }
    }
}

impl TrackDto {
    pub fn try_to_audio_track(&self) -> Result<nexus_player_backend::audio::types::Track, String> {
        use nexus_player_backend::audio::types::{Track, TrackSource, TrackState};

        let src = self
            .source
            .as_ref()
            .ok_or_else(|| format!("Track '{}' has no source", self.title))?;
        let source = match src.system.as_str() {
            "local" => TrackSource::Local(src.url.clone().into()),
            "http" => TrackSource::Http(src.url.clone()),
            // A cached source is already a playable local file. TrackSourceDto does not carry
            // original_url, so preserve playback correctness by treating it as local here.
            "cache" => TrackSource::Local(src.url.clone().into()),
            _ => TrackSource::External {
                id: src.url.clone(),
                system: src.system.clone(),
            },
        };

        let state = match &self.state {
            TrackStateDto::Saved => TrackState::Saved,
            TrackStateDto::Streaming => TrackState::Streaming,
            TrackStateDto::Cached => TrackState::Cached,
        };

        Ok(Track {
            id: self.id as usize,
            title: self.title.clone(),
            artist: self.artist.clone(),
            album: self.album.clone(),
            duration: self.duration_secs.map(std::time::Duration::from_secs_f64),
            artwork_url: self.artwork_url.clone(),
            format: None,
            source,
            state,
            liked: self.liked,
        })
    }
}

impl From<models::Track> for TrackDto {
    fn from(track: models::Track) -> Self {
        let artwork_url = track
            .metadata
            .artwork
            .as_ref()
            .and_then(|art| match &art.source {
                nexus_player_backend::library::types::ArtworkSource::Url { url } => {
                    Some(url.clone())
                }
                _ => None,
            });

        let state = match track.cache {
            CacheStatus::Cached { .. } => TrackStateDto::Cached,
            _ => TrackStateDto::Saved,
        };

        Self {
            id: track.id.0,
            title: track.metadata.title,
            artist: track.artist,
            album: track.album,
            duration_secs: track
                .audio_info
                .as_ref()
                .and_then(|info| info.duration_secs),
            artwork_url,
            play_count: track.play_count,
            liked: track.liked,
            state,
            source: Some(track.source.into()),
            unavailable: track.unavailable,
        }
    }
}

impl From<audio::Track> for TrackDto {
    fn from(track: audio::Track) -> Self {
        Self {
            id: track.id as i64,
            title: track.title,
            artist: track.artist,
            album: track.album,
            duration_secs: track.duration.map(|d| d.as_secs_f64()),
            artwork_url: track.artwork_url,
            play_count: 0,
            liked: track.liked,
            state: track.state.into(),
            source: Some(track.source.into()),
            unavailable: false,
        }
    }
}

impl From<&audio::Track> for TrackDto {
    fn from(track: &audio::Track) -> Self {
        Self {
            id: track.id as i64,
            title: track.title.clone(),
            artist: track.artist.clone(),
            album: track.album.clone(),
            duration_secs: track.duration.map(|d| d.as_secs_f64()),
            artwork_url: track.artwork_url.clone(),
            play_count: 0,
            liked: track.liked,
            state: track.state.clone().into(),
            source: Some(track.source.clone().into()),
            unavailable: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackSourceDto {
    pub url: String,
    pub system: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum TrackStateDto {
    Saved,
    Streaming,
    Cached,
}
impl From<TrackState> for TrackStateDto {
    fn from(value: TrackState) -> Self {
        match value {
            TrackState::Saved => TrackStateDto::Saved,
            TrackState::Streaming => TrackStateDto::Streaming,
            TrackState::Cached => TrackStateDto::Cached,
        }
    }
}

impl From<nexus_player_backend::audio::types::TrackSource> for TrackSourceDto {
    fn from(track_source: nexus_player_backend::audio::types::TrackSource) -> Self {
        match track_source {
            nexus_player_backend::audio::types::TrackSource::Local(path) => Self {
                url: path.display().to_string(),
                system: "local".to_string(),
            },
            nexus_player_backend::audio::types::TrackSource::Http(url) => Self {
                url,
                system: "http".to_string(),
            },
            nexus_player_backend::audio::types::TrackSource::External { id, system } => {
                Self { url: id, system }
            }
            nexus_player_backend::audio::types::TrackSource::Cached { cache_path, .. } => Self {
                url: cache_path.display().to_string(),
                system: "cache".to_string(),
            },
        }
    }
}

impl From<nexus_player_backend::streaming::types::TrackInfo> for TrackDto {
    fn from(track: nexus_player_backend::streaming::types::TrackInfo) -> Self {
        let (url, system) = match &track.source {
            nexus_player_backend::library::TrackSource::External { id, system } => {
                (id.clone(), system.clone())
            }
            _ => (track.source.as_str(), "unknown".to_string()),
        };

        Self {
            id: hash_str_to_i64(&track.source.as_str()),
            unavailable: false,
            title: track.title,
            artist: Some(track.artist),
            album: None,
            duration_secs: track.duration.map(|d| d.as_secs_f64()),
            artwork_url: track.thumbnail,
            play_count: 0,
            liked: false,
            state: TrackStateDto::Streaming,
            source: Some(TrackSourceDto { url, system }),
        }
    }
}
