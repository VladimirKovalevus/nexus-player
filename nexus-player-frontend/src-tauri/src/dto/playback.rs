use super::{TrackDto, TrackSourceDto};
use nexus_player_backend::audio::types::{self as audio};
use nexus_player_backend::library::models;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupResultDto {
    pub deleted_tracks: u64,
    pub deleted_artworks: u64,
}

/// One entry of an artwork batch resolution.
///
/// `path` is the local file to render, or `None` while the cover is still being
/// fetched — the UI keeps showing the remote `url` until then.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedArtworkDto {
    pub url: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioStateDto {
    pub state: String,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub volume: f32,
    pub current_track: Option<TrackDto>,
    pub buffered_ranges: Vec<BufferedRangeDto>,
    pub waveforms: Vec<WaveformDto>,
    pub fft: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaveformDto {
    pub data: Vec<f32>,
    pub start_time: f64,
    pub end_time: f64,
    pub byte_start: Option<u64>,
    pub byte_end: Option<u64>,
}

impl From<nexus_player_backend::audio::types::Waveform> for WaveformDto {
    fn from(w: nexus_player_backend::audio::types::Waveform) -> Self {
        Self {
            data: w.data,
            start_time: w.start_time,
            end_time: w.end_time,
            byte_start: w.byte_start,
            byte_end: w.byte_end,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BufferedRangeDto {
    pub start: f32,
    pub end: f32,
}

impl From<nexus_player_backend::audio::types::BufferedRange> for BufferedRangeDto {
    fn from(range: nexus_player_backend::audio::types::BufferedRange) -> Self {
        Self {
            start: range.start,
            end: range.end,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayHistoryEntryDto {
    pub id: i64,
    pub title: String,
    pub artist: Option<String>,
    pub source: TrackSourceDto,
    pub artwork_url: Option<String>,
    pub played_at: i64,
}

impl From<models::PlayHistoryEntry> for PlayHistoryEntryDto {
    fn from(entry: models::PlayHistoryEntry) -> Self {
        Self {
            id: entry.id,
            title: entry.title,
            artist: entry.artist,
            source: entry.source.into(),
            artwork_url: entry.artwork_url,
            played_at: entry.played_at,
        }
    }
}

impl From<audio::AudioState> for AudioStateDto {
    fn from(state: audio::AudioState) -> Self {
        let state_str = match state.playback_state {
            audio::PlaybackState::Playing => "playing",
            audio::PlaybackState::Paused => "paused",
            audio::PlaybackState::Stopped => "stopped",
            audio::PlaybackState::Buffering => "buffering",
            audio::PlaybackState::Loading => "loading",
        };

        Self {
            state: state_str.to_string(),
            position_secs: state.position.as_secs_f64(),
            duration_secs: state.duration.as_secs_f64(),
            volume: state.volume,
            current_track: state.current_track.map(|t| t.into()),
            buffered_ranges: state
                .buffered_ranges
                .into_iter()
                .map(BufferedRangeDto::from)
                .collect(),
            waveforms: state.waveforms.into_iter().map(WaveformDto::from).collect(),
            fft: state.fft,
        }
    }
}
