use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use crate::{
    detection::detect_stream_type,
    error::StreamingError,
    multi_downloader::{source::Source, TaskDownloader},
    state::types::{
        format_bytes, DownloadOptions, DownloadTask, ManagerCommand, ManagerConfig, Priority,
        TaskInfo, TrackId, TrackState,
    },
};

use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::{Mutex, RwLock};

mod background;

pub struct BandwidthMonitor {
    bps: AtomicU64,
}

impl Default for BandwidthMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl BandwidthMonitor {
    pub fn new() -> Self {
        Self {
            bps: AtomicU64::new(0),
        }
    }

    pub fn update(&self, bytes: usize, duration: Duration) {
        if duration.as_secs_f64() > 0.0 {
            let current_bps = (bytes as f64 * 8.0 / duration.as_secs_f64()) as u64;

            // Simple moving average (smoothing)
            let old_bps = self.bps.load(Ordering::Relaxed);
            if old_bps == 0 {
                self.bps.store(current_bps, Ordering::Relaxed);
            } else {
                let new_bps = (old_bps as f64 * 0.7 + current_bps as f64 * 0.3) as u64;
                self.bps.store(new_bps, Ordering::Relaxed);
            }
        }
    }

    pub fn get_bps(&self) -> u64 {
        self.bps.load(Ordering::Relaxed)
    }
}

#[derive(Default)]
pub struct PlaybackContext {
    pub current: Option<TrackId>,
    pub active_ids: Vec<TrackId>,
    pub low_priority_queue: Vec<TrackId>,
}

pub struct DownloadManager {
    pub states: Arc<Mutex<HashMap<TrackId, TrackState>>>,
    pub queue: Arc<Mutex<HashMap<TrackId, DownloadTask>>>,
    pub sender: Arc<UnboundedSender<ManagerCommand>>,
    pub playback: Arc<Mutex<PlaybackContext>>,
    pub bandwidth_monitor: Arc<BandwidthMonitor>,
    pub config: Arc<RwLock<ManagerConfig>>,
    /// LRU order: front = least recently used, back = most recently used.
    /// Only Complete tracks are tracked here.
    lru: Arc<Mutex<VecDeque<TrackId>>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl DownloadManager {
    pub fn new(config: ManagerConfig) -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let queue = Arc::new(Mutex::new(HashMap::new()));
        let tasks = Arc::new(Mutex::new(HashMap::new()));
        let v1 = queue.clone();
        let v2 = tasks.clone();
        let p = Arc::new(Mutex::new(PlaybackContext::default()));
        let v3 = p.clone();
        let bandwidth_monitor = Arc::new(BandwidthMonitor::new());
        let v4 = bandwidth_monitor.clone();
        let lru = Arc::new(Mutex::new(VecDeque::new()));
        let v5 = lru.clone();
        let config = Arc::new(RwLock::new(config));
        let v6 = config.clone();
        let handle = std::thread::spawn(move || {
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    tracing::error!("Failed to create tokio runtime: {}", e);
                    return;
                }
            };
            rt.block_on(Self::event_loop(rx, v1, v2, v3, v4, v5, v6));
        });

        Self {
            handle: Some(handle),
            queue,
            states: tasks,
            playback: p,
            sender: Arc::new(tx),
            bandwidth_monitor,
            config,
            lru,
        }
    }

    pub fn new_default() -> Self {
        Self::new(ManagerConfig::default())
    }

    pub async fn update_task(&self, track_id: TrackId, options: DownloadOptions) {
        let mut q = self.queue.lock().await;
        if let Some(task) = q.get_mut(&track_id) {
            task.options = Some(options);
        }
    }

    pub async fn add_download_task(
        &self,
        url: &str,
        track_id: u64,
        options: Option<DownloadOptions>,
    ) -> Result<Source, StreamingError> {
        let source = self.add_track(url, track_id, options).await?;
        let mut playback = self.playback.lock().await;
        if !playback.low_priority_queue.contains(&track_id) {
            playback.low_priority_queue.push(track_id);
        }
        Ok(source)
    }

    pub async fn update_config(&self, config: ManagerConfig) {
        *self.config.write().await = config;
        self.evict_if_needed().await;
    }

    pub async fn set_context(&self, current: Option<TrackId>, active_ids: Vec<TrackId>) {
        let _ = self
            .sender
            .send(ManagerCommand::SetPlayback(current, active_ids));
    }

    pub async fn get_track(&self, track_id: u64) -> Option<Source> {
        let states = self.states.lock().await;
        let result = match states.get(&track_id) {
            Some(track) => match track {
                TrackState::Downloading {
                    url: _,
                    buffer,
                    meta: _,
                } => Some(Source::from_buffer_type(
                    buffer.clone(),
                    track_id,
                    self.sender.clone(),
                )),
                TrackState::Complete {
                    url: _,
                    buffer,
                    meta: _,
                } => Some(Source::from_buffer_type(
                    buffer.clone(),
                    track_id,
                    self.sender.clone(),
                )),
                _ => None,
            },
            None => None,
        };
        drop(states);
        if result.is_some() {
            let mut lru_guard = self.lru.lock().await;
            lru_guard.retain(|&x| x != track_id);
            lru_guard.push_back(track_id);
        }
        result
    }

    pub async fn get_track_with_url(&self, track_id: u64) -> Option<(Source, String)> {
        let states = self.states.lock().await;
        let result = match states.get(&track_id) {
            Some(track) => match track {
                TrackState::Downloading {
                    url,
                    buffer,
                    meta: _,
                } => Some((
                    Source::from_buffer_type(buffer.clone(), track_id, self.sender.clone()),
                    url.clone(),
                )),
                TrackState::Complete {
                    url,
                    buffer,
                    meta: _,
                } => Some((
                    Source::from_buffer_type(buffer.clone(), track_id, self.sender.clone()),
                    url.clone(),
                )),
                _ => None,
            },
            None => None,
        };
        drop(states);
        if result.is_some() {
            let mut lru_guard = self.lru.lock().await;
            lru_guard.retain(|&x| x != track_id);
            lru_guard.push_back(track_id);
        }
        result
    }
    pub async fn list_tasks(&self) -> Vec<TaskInfo> {
        let states = self.states.lock().await;
        states
            .iter()
            .map(|(id, state)| {
                let (url, state_name, bytes, progress, meta) = match state {
                    TrackState::Pending {
                        url, buffer, meta, ..
                    } => (
                        url.clone(),
                        "Pending".to_string(),
                        buffer.byte_size(),
                        buffer.progress(),
                        meta.clone(),
                    ),
                    TrackState::Downloading { url, buffer, meta } => (
                        url.clone(),
                        "Downloading".to_string(),
                        buffer.byte_size(),
                        buffer.progress(),
                        meta.clone(),
                    ),
                    TrackState::Complete { url, buffer, meta } => (
                        url.clone(),
                        "Complete".to_string(),
                        buffer.byte_size(),
                        buffer.progress(),
                        meta.clone(),
                    ),
                    TrackState::Failed { url, error, meta } => (
                        url.clone(),
                        format!("Failed: {}", error),
                        0,
                        0.0,
                        meta.clone(),
                    ),
                    TrackState::Cancelled { url, meta } => {
                        (url.clone(), "Cancelled".to_string(), 0, 0.0, meta.clone())
                    }
                };

                TaskInfo {
                    track_id: *id,
                    url,
                    state: state_name,
                    size_human: format_bytes(bytes),
                    byte_size: bytes,
                    progress,
                    meta,
                }
            })
            .collect()
    }

    pub async fn remove_track(&self, track_id: TrackId) {
        // Cancel the task if it's currently downloading
        let _ = self.sender.send(ManagerCommand::Cancel(track_id));

        // Remove from states, queue and LRU
        {
            let mut states = self.states.lock().await;
            states.remove(&track_id);
        }
        {
            let mut queue = self.queue.lock().await;
            queue.remove(&track_id);
        }
        {
            let mut lru_guard = self.lru.lock().await;
            lru_guard.retain(|&x| x != track_id);
        }
    }

    async fn evict_if_needed(&self) {
        let (protected_current, protected_active) = {
            let pb = self.playback.lock().await;
            (pb.current, pb.active_ids.clone())
        };

        loop {
            // Find least recently used candidate (not protected, Complete state)
            let candidate = {
                let lru_guard = self.lru.lock().await;
                lru_guard
                    .iter()
                    .find(|&&id| Some(id) != protected_current && !protected_active.contains(&id))
                    .copied()
            };

            let Some(candidate_id) = candidate else {
                break;
            };

            // Calculate total memory of Complete tracks
            let total_bytes: u64 = {
                let states = self.states.lock().await;
                states
                    .values()
                    .filter_map(|s| match s {
                        TrackState::Complete { buffer, .. } => Some(buffer.byte_size()),
                        _ => None,
                    })
                    .sum()
            };

            if total_bytes <= self.config.read().await.mem_limit_bytes {
                break;
            }

            // Evict
            {
                let mut states = self.states.lock().await;
                if matches!(states.get(&candidate_id), Some(TrackState::Complete { .. })) {
                    println!("LRU EVICT: track_id={candidate_id} (total_bytes={total_bytes})");
                    states.remove(&candidate_id);
                }
            }
            {
                let mut lru_guard = self.lru.lock().await;
                lru_guard.retain(|&x| x != candidate_id);
            }
        }
    }

    pub async fn add_track(
        &self,
        url: &str,
        track_id: u64,
        options: Option<DownloadOptions>,
    ) -> Result<Source, StreamingError> {
        {
            let states = self.states.lock().await;
            if let Some(state) = states.get(&track_id) {
                let buffer = match state {
                    TrackState::Pending { buffer, .. } => Some(buffer.clone()),
                    TrackState::Downloading { buffer, .. } => Some(buffer.clone()),
                    TrackState::Complete { buffer, .. } => Some(buffer.clone()),
                    _ => None,
                };
                if let Some(buffer) = buffer {
                    drop(states);
                    let mut lru_guard = self.lru.lock().await;
                    lru_guard.retain(|&x| x != track_id);
                    lru_guard.push_back(track_id);
                    return Ok(Source::from_buffer_type(
                        buffer,
                        track_id,
                        self.sender.clone(),
                    ));
                }
            }
        }

        self.evict_if_needed().await;

        let user_agent = options.as_ref().and_then(|o| o.user_agent.clone());

        let stream_type = detect_stream_type(url, user_agent.clone()).await?;
        let bandwidth_bps = self.bandwidth_monitor.get_bps();
        let (loader, buffer) = {
            let cfg = self.config.read().await;
            TaskDownloader::create(
                stream_type,
                bandwidth_bps,
                user_agent,
                cfg.waveform_enabled,
                cfg.chunk_size,
            )?
        };
        let meta = options.as_ref().and_then(|o| o.meta.clone());
        let download_task = DownloadTask {
            track_id,
            priority: Priority::High,
            downloader: loader,
            options,
        };

        self.states.lock().await.insert(
            track_id,
            TrackState::Pending {
                url: url.to_string(),
                priority: Priority::High,
                buffer: buffer.clone(),
                meta,
            },
        );
        self.queue.lock().await.insert(track_id, download_task);

        let _ = self.sender.send(ManagerCommand::Wake);

        let stream = Source::from_buffer_type(buffer, track_id, self.sender.clone());

        Ok(stream)
    }
}
impl Drop for DownloadManager {
    fn drop(&mut self) {
        let hanlde = self.handle.take();
        if let Some(thread) = hanlde {
            let _ = self.sender.send(ManagerCommand::Shutdown());
            let _ = thread.join();
        }
    }
}
