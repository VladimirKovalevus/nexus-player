use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use symphonia::core::io::MediaSource as SymphoniaSource;

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

    PlaySource(Box<dyn MediaSource>),

    Pause,

    Resume,

    Stop,

    Seek(Duration),

    SetVolume(f32),

    Shutdown,
}
#[derive(Debug, Clone)]
pub enum AudioEvent {
    PlaybackStarted,

    PlaybackPaused,

    PlaybackResumed,

    PlaybackStopped,

    TrackFinished,

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

    pub position: Duration,

    pub duration: Duration,

    pub volume: f32,
}

impl Default for AudioState {
    fn default() -> Self {
        Self {
            playback_state: PlaybackState::Stopped,
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

    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<Duration>,
    pub artwork_url: Option<String>,

    pub format: Option<AudioFormat>,

    pub source: TrackSource,
}

#[derive(Debug, Clone)]
pub struct EngineTrack {
    pub track: Track,
    pub audio_source: AudioSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum TrackSource {
    Local(PathBuf),

    Http(String),

    External {
        system: String,
        id: String,
    },

    Cached {
        cache_path: PathBuf,
        original_url: String,
    },
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

    pub fn cached(cache_path: impl Into<PathBuf>, original_url: impl Into<String>) -> Self {
        Self::Cached {
            cache_path: cache_path.into(),
            original_url: original_url.into(),
        }
    }

    pub fn as_str(&self) -> String {
        match self {
            Self::Local(path) => path.display().to_string(),
            Self::Http(url) => url.clone(),
            Self::External { system, id } => format!("{}:{}", system, id),
            Self::Cached { cache_path, .. } => cache_path.display().to_string(),
        }
    }
}

impl From<crate::library::models::Track> for Track {
    fn from(track: crate::library::models::Track) -> Self {
        Self {
            id: track.id.0 as usize,
            title: track.metadata.title,
            artist: track.artist,
            album: track.album,
            duration: track
                .audio_info
                .as_ref()
                .and_then(|info| info.duration_secs.map(|secs| Duration::from_secs_f64(secs))),
            artwork_url: None,
            format: track.audio_info.as_ref().and_then(|info| {
                info.sample_rate
                    .zip(info.channels)
                    .map(|(sr, ch)| AudioFormat {
                        sample_rate: SampleRate(sr),
                        channels: ch as u16,
                    })
            }),
            source: track.source,
        }
    }
}

use std::fs::File;
use std::io::{Read, Seek};

pub trait MediaSource: SymphoniaSource + Read + Seek + Send + std::fmt::Debug {
    fn clone_box(&self) -> Box<dyn MediaSource>;
}

impl Clone for Box<dyn MediaSource> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

impl MediaSource for File {
    fn clone_box(&self) -> Box<dyn MediaSource> {
        Box::new(self.try_clone().expect("Failed to clone file descriptor"))
    }
}

#[derive(Debug, Clone)]
pub struct AudioSource {
    pub reader: Box<dyn MediaSource>,
    pub format: Option<AudioFormat>,
    pub duration: Option<Duration>,
}

impl AudioSource {
    pub fn new(reader: Box<dyn MediaSource>) -> Self {
        Self {
            reader,
            format: None,
            duration: None,
        }
    }

    pub fn with_format(mut self, format: AudioFormat) -> Self {
        self.format = Some(format);
        self
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    pub fn into_reader(self) -> Box<dyn MediaSource> {
        self.reader
    }

    pub fn from_track(track: Track) -> Result<Self, std::io::Error> {
        let reader: Box<dyn MediaSource> = match track.source {
            TrackSource::Local(path) => Box::new(File::open(path)?),
            TrackSource::Cached { cache_path, .. } => Box::new(File::open(cache_path)?),
            TrackSource::Http(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "HTTP sources not yet implemented",
                ))
            }
            TrackSource::External { .. } => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "External sources not yet implemented",
                ))
            }
        };

        Ok(Self {
            reader,
            format: track.format,
            duration: track.duration,
        })
    }
}
