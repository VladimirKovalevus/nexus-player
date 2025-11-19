use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SampleRate(pub u32);

impl SampleRate {
    pub const LOW: Self = Self(44100);

    pub const AVG: Self = Self(96000);

    pub const HIRES: Self = Self(192000);

    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone)]
pub enum AudioCommand {
    Play(Track),

    Pause,

    Resume,

    Stop,

    Seek(Duration),

    SetVolume(f32),

    Shutdown,
}
#[derive(Debug, Clone)]
pub enum AudioEvent {
    PlaybackStarted { track: Track },

    PlaybackPaused,

    PlaybackResumed,

    PlaybackStopped,

    TrackFinished { track_id: usize },

    PositionUpdated { position: Duration },

    DurationChanged { duration: Duration },

    VolumeChanged { volume: f32 },

    Error { message: String },

    BufferingStarted,

    BufferingFinished,
}

#[derive(Debug, Clone)]
pub struct AudioState {
    pub playback_state: PlaybackState,

    pub current_track: Option<Track>,

    pub position: Duration,

    pub duration: Duration,

    pub volume: f32,
}

impl Default for AudioState {
    fn default() -> Self {
        Self {
            playback_state: PlaybackState::Stopped,
            current_track: None,
            position: Duration::ZERO,
            duration: Duration::ZERO,
            volume: 1.0,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,

    Playing,

    Paused,

    Buffering,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self::Stopped
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioFormat {
    pub sample_rate: SampleRate,

    pub channels: u16,
}
impl AudioFormat {
    pub fn new(sample_rate: SampleRate, channels: u16) -> Self {
        Self {
            sample_rate,
            channels,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub id: usize,

    pub format: Option<AudioFormat>,

    pub source: TrackSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum TrackSource {
    Local(PathBuf),

    Http(String),

    External { system: String, id: String },
}

impl TrackSource {
    pub fn local(path: impl Into<PathBuf>) -> Self {
        Self::Local(path.into())
    }

    pub fn http(url: impl Into<String>) -> Self {
        Self::Http(url.into())
    }

    pub fn external(system: impl Into<String>, id: impl Into<String>) -> Self {
        Self::External {
            system: system.into(),
            id: id.into(),
        }
    }

    pub fn as_str(&self) -> String {
        match self {
            Self::Local(path) => path.display().to_string(),
            Self::Http(url) => url.clone(),
            Self::External { system, id } => format!("{}:{}", system, id),
        }
    }
}
