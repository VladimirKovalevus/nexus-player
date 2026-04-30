use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, path::PathBuf, sync::Arc};

use crate::{
    buffers::{chunked::ChunkedBuffer, hls::HlsBuffer, Buffer},
    detection::StreamType,
    error::{InitializationError, StreamingError},
    multi_downloader::{SeekPos, TaskDownloader, TrackDownloader},
};

pub type TrackId = u64;

pub const DEFAULT_MEM_LIMIT_BYTES: u64 = 100 * 1024 * 1024; // 100 MB
pub const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36";

#[derive(Debug, Clone)]
pub struct ManagerConfig {
    pub waveform_enabled: bool,
    pub mem_limit_bytes: u64,
    /// Fraction (0.0–1.0) of each active track to pre-download before sequentially completing any.
    pub prefetch_limit: f64,
    /// Size of each download chunk in bytes.
    pub chunk_size: u64,
    pub default_callbacks: DefaultCallbacks,
}

impl Default for ManagerConfig {
    fn default() -> Self {
        Self {
            waveform_enabled: false,
            mem_limit_bytes: DEFAULT_MEM_LIMIT_BYTES,
            prefetch_limit: 0.2,
            chunk_size: crate::buffers::chunked::CHUNK_SIZE * 3,
            default_callbacks: DefaultCallbacks::default(),
        }
    }
}
pub type DefaultCallback =
    dyn Fn(TrackId, Result<BufferType, StreamingError>, Option<StreamMetadata>) + Send + Sync + 'static;

pub type TaskCallback =
    dyn Fn(Result<BufferType, StreamingError>, Option<StreamMetadata>) + Send + Sync + 'static;

#[derive(Clone)]
pub struct DefaultCallbacks {
    pub on_start: Option<Arc<DefaultCallback>>,
    pub on_progress: Option<Arc<DefaultCallback>>,
    pub on_finish: Option<Arc<DefaultCallback>>,
}

impl Default for DefaultCallbacks {
    fn default() -> Self {
        Self {
            on_start: None,
            on_progress: None,
            on_finish: None,
        }
    }
}

impl std::fmt::Debug for DefaultCallbacks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultCallbacks")
            .field("on_start", &self.on_start.is_some())
            .field("on_progress", &self.on_progress.is_some())
            .field("on_finish", &self.on_finish.is_some())
            .finish()
    }
}

#[derive(Debug, Clone, Default)]
pub struct Waveform {
    pub data: Vec<f32>,
    pub start_time: f64,
    pub end_time: f64,
    pub byte_start: Option<u64>,
    pub byte_end: Option<u64>,
}

#[derive(Debug, Clone)]
pub enum BufferType {
    Chunked(Arc<ChunkedBuffer>),
    Hls(Arc<HlsBuffer>),
}

impl BufferType {
    pub fn progress(&self) -> f64 {
        match self {
            BufferType::Chunked(b) => b.progress(),
            BufferType::Hls(b) => b.progress(),
        }
    }

    pub fn profile(&self) {
        match self {
            BufferType::Chunked(b) => b.profile(),
            BufferType::Hls(b) => b.profile(),
        }
    }
    pub fn container(&self) -> String {
        match self {
            BufferType::Chunked(b) => "mp3".to_string(),
            BufferType::Hls(b) => b.container().unwrap_or("m4a".to_string()),
        }
    }

    pub fn byte_size(&self) -> u64 {
        match self {
            BufferType::Chunked(b) => b.byte_size(),
            BufferType::Hls(b) => b.byte_size(),
        }
    }

    pub fn save_to_file(&self, path: PathBuf) -> std::io::Result<()> {
        match self {
            BufferType::Chunked(b) => b.save_to_file(path),
            BufferType::Hls(b) => b.save_to_file(path),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInfo {
    pub track_id: TrackId,
    pub url: String,
    pub state: String,
    pub size_human: String,
    pub byte_size: u64,
    pub progress: f64,
    pub meta: Option<StreamMetadata>,
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

pub struct DownloadTask {
    pub track_id: TrackId,
    pub priority: Priority,
    pub downloader: Box<dyn TrackDownloader>,
    pub options: Option<DownloadOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<std::time::Duration>,
    pub artwork_url: Option<String>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
}

pub struct DownloadOptions {
    pub user_agent: Option<String>,
    pub meta: Option<StreamMetadata>,
    pub on_start: Option<Arc<TaskCallback>>,
    pub on_progress: Option<Arc<TaskCallback>>,
    pub on_finish: Option<Arc<TaskCallback>>,
}

impl PartialEq for DownloadTask {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority && self.track_id == other.track_id
    }
}
impl Eq for DownloadTask {}

impl PartialOrd for DownloadTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DownloadTask {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority
            .cmp(&other.priority)
            .then_with(|| self.track_id.cmp(&other.track_id))
    }
}

#[derive(Debug)]
pub enum TrackState {
    Pending {
        url: String,
        priority: Priority,
        buffer: Arc<BufferType>,
        meta: Option<StreamMetadata>,
    },
    Downloading {
        url: String,
        buffer: Arc<BufferType>,
        meta: Option<StreamMetadata>,
    },
    Complete {
        url: String,
        buffer: Arc<BufferType>,
        meta: Option<StreamMetadata>,
    },
    Failed {
        url: String,
        error: String,
        meta: Option<StreamMetadata>,
    },
    Cancelled {
        url: String,
        meta: Option<StreamMetadata>,
    },
}

impl Clone for TrackState {
    fn clone(&self) -> Self {
        match self {
            TrackState::Pending {
                url,
                priority,
                buffer,
                meta,
            } => TrackState::Pending {
                url: url.clone(),
                priority: priority.clone(),
                buffer: Arc::clone(buffer),
                meta: meta.clone(),
            },
            TrackState::Downloading { url, buffer, meta } => TrackState::Downloading {
                url: url.clone(),
                buffer: Arc::clone(buffer),
                meta: meta.clone(),
            },
            TrackState::Complete { url, buffer, meta } => TrackState::Complete {
                url: url.clone(),
                buffer: Arc::clone(buffer),
                meta: meta.clone(),
            },
            TrackState::Failed { url, error, meta } => TrackState::Failed {
                url: url.clone(),
                error: error.clone(),
                meta: meta.clone(),
            },
            TrackState::Cancelled { url, meta } => TrackState::Cancelled {
                url: url.clone(),
                meta: meta.clone(),
            },
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low = 0,
    High = 1,
    Critical = 2,
}

pub enum ManagerCommand {
    Cancel(TrackId),
    Seek(TrackId, SeekPos),
    SetPlayback(Option<TrackId>, Vec<TrackId>),
    Wake,
    Shutdown(),
}

pub enum TaskCommand {
    Seek(TrackId, SeekPos),
    Pause(TrackId),
    Resume(TrackId),
    Cancel(TrackId),
}
