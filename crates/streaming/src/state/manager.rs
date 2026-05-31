use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use crate::{
    buffers::chunked::ChunkedBuffer,
    detection::detect_stream_type,
    error::StreamingError,
    multi_downloader::{source::Source, TaskDownloader},
    state::types::{
        format_bytes, DownloadOptions, DownloadTask, ManagerCommand, ManagerConfig, Priority,
        TaskInfo, TrackId, TrackState,
    },
};

use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::{Mutex, RwLock};

pub struct BandwidthMonitor {
    bps: AtomicU64,
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

pub struct PlaybackContext {
    pub current: Option<TrackId>,
    pub active_ids: Vec<TrackId>,
    pub low_priority_queue: Vec<TrackId>,
}
impl Default for PlaybackContext {
    fn default() -> Self {
        Self {
            current: None,
            active_ids: Vec::new(),
            low_priority_queue: Vec::new(),
        }
    }
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

    async fn handle_command(
        cmd: ManagerCommand,
        queue: &Arc<Mutex<HashMap<TrackId, DownloadTask>>>,
        states: &Arc<Mutex<HashMap<TrackId, TrackState>>>,
        ctx: &Arc<Mutex<PlaybackContext>>,
        config: &Arc<RwLock<ManagerConfig>>,
    ) {
        match cmd {
            ManagerCommand::Cancel(id) => {
                let mut trigger_finish = false;
                {
                    let mut s = states.lock().await;
                    if let Some(e) = s.get_mut(&id) {
                        match e {
                            TrackState::Pending { url, meta, .. } => {
                                *e = TrackState::Cancelled {
                                    url: std::mem::take(url),
                                    meta: meta.clone(),
                                };
                                trigger_finish = true;
                            }
                            TrackState::Downloading { url, meta, .. } => {
                                *e = TrackState::Cancelled {
                                    url: std::mem::take(url),
                                    meta: meta.clone(),
                                };
                                trigger_finish = true;
                            }
                            _ => {}
                        }
                    }
                }

                if trigger_finish {
                    let q = queue.lock().await;
                    if let Some(task) = q.get(&id) {
                        let mut called = false;
                        let meta = task.options.as_ref().and_then(|o| o.meta.clone());
                        if let Some(options) = &task.options {
                            if let Some(on_finish) = &options.on_finish {
                                on_finish(Ok(task.downloader.buffer()), meta.clone());
                                called = true;
                            }
                        }
                        if !called {
                            let config = config.read().await;
                            if let Some(on_finish) = &config.default_callbacks.on_finish {
                                on_finish(id, Ok(task.downloader.buffer()), meta);
                            }
                        }
                    }
                }
            }
            ManagerCommand::SetPlayback(current, active_ids) => {
                let mut playback = ctx.lock().await;
                playback.current = current;
                playback.active_ids = active_ids;
            }
            ManagerCommand::Seek(id, pos) => {
                println!("SEEK RECEIVED : {id} {pos:?}");
                let mut q = queue.lock().await;
                if let Some(task) = q.get_mut(&id) {
                    task.priority = Priority::Critical;
                    task.downloader.seek(pos);
                }
                for (track_id, task) in q.iter_mut() {
                    if *track_id != id {
                        task.priority = Priority::High;
                    }
                }
            }
            ManagerCommand::Wake => {}
            ManagerCommand::Shutdown() => {
                // handled in caller
            }
        }
    }

    async fn event_loop(
        mut rx: tokio::sync::mpsc::UnboundedReceiver<ManagerCommand>,
        queue: Arc<Mutex<HashMap<TrackId, DownloadTask>>>,
        states: Arc<Mutex<HashMap<TrackId, TrackState>>>,
        ctx: Arc<Mutex<PlaybackContext>>,
        bandwidth_monitor: Arc<BandwidthMonitor>,
        lru: Arc<Mutex<VecDeque<TrackId>>>,
        config: Arc<RwLock<ManagerConfig>>,
    ) {
        let mut buffered_cmd = None;

        loop {
            // Process command if we have one buffered, or try to receive one
            let mut current_cmd = buffered_cmd.take();
            if current_cmd.is_none() {
                current_cmd = rx.try_recv().ok();
            }

            if let Some(cmd) = current_cmd {
                if matches!(cmd, ManagerCommand::Shutdown()) {
                    return;
                }
                Self::handle_command(cmd, &queue, &states, &ctx, &config).await;
                continue; // Process all pending commands first
            }

            let (current_id, active_ids, prefetch_limit, low_priority_id) = {
                let playback = ctx.lock().await;
                let limit = config.read().await.prefetch_limit;
                let low_id = playback
                    .low_priority_queue
                    .iter()
                    .find(|&&id| Some(id) != playback.current && !playback.active_ids.contains(&id))
                    .copied();
                (playback.current, playback.active_ids.clone(), limit, low_id)
            };

            let mut target_id = None;
            let mut tasks_to_remove = Vec::new();
            let mut lru_completed = Vec::new();

            // Step 1: current track
            let mut q = queue.lock().await;
            if let Some(id) = current_id {
                if let Some(task) = q.get(&id) {
                    if !task.downloader.is_complete() {
                        target_id = Some(id);
                    } else {
                        tasks_to_remove.push(id);
                        lru_completed.push(id);
                    }
                }
            }

            // Step 2 & 3: active tracks below prefetch_limit
            if target_id.is_none() {
                let non_current: Vec<TrackId> = active_ids
                    .iter()
                    .filter(|&&id| Some(id) != current_id)
                    .copied()
                    .collect();

                let mut all_at_limit = true;
                let mut first_below_limit = None;
                let mut first_incomplete = None;

                let s = states.lock().await;
                for &id in &non_current {
                    let (is_complete, is_below) = match s.get(&id) {
                        Some(TrackState::Complete { .. }) => (true, false),
                        Some(TrackState::Downloading { buffer, .. }) => {
                            (false, buffer.progress() < prefetch_limit)
                        }
                        Some(TrackState::Pending { buffer, .. }) => {
                            (false, buffer.progress() < prefetch_limit)
                        }
                        _ => (false, false),
                    };
                    if !is_complete && first_incomplete.is_none() {
                        first_incomplete = Some(id);
                    }
                    if is_below {
                        all_at_limit = false;
                        if first_below_limit.is_none() {
                            first_below_limit = Some(id);
                        }
                    }
                }
                drop(s);

                if !all_at_limit {
                    if let Some(id) = first_below_limit {
                        if q.contains_key(&id) {
                            target_id = Some(id);
                        }
                    }
                } else {
                    if let Some(id) = first_incomplete {
                        if let Some(task) = q.get(&id) {
                            if !task.downloader.is_complete() {
                                target_id = Some(id);
                            } else {
                                tasks_to_remove.push(id);
                                lru_completed.push(id);
                            }
                        }
                    }
                }
            }

            // Step 4: Removed from here. Low priority only in idle.

            for id in &tasks_to_remove {
                q.remove(id);
            }
            let task_to_process = target_id.and_then(|id| q.remove(&id).map(|task| (id, task)));
            drop(q);

            if !lru_completed.is_empty() {
                let mut lru_guard = lru.lock().await;
                for id in lru_completed {
                    lru_guard.retain(|&x| x != id);
                    lru_guard.push_back(id);
                }
            }

            if let Some((id, mut task)) = task_to_process {
                let mut finished = false;

                tokio::select! {
                    cmd_opt = rx.recv() => {
                        if let Some(cmd) = cmd_opt {
                            buffered_cmd = Some(cmd);
                        } else {
                            return; // Channel closed
                        }
                    }
                    res = Self::process_task(&mut task, states.clone(), bandwidth_monitor.clone(), config.clone()) => {
                        finished = res;
                    }
                }

                let mut q = queue.lock().await;
                let is_cancelled = {
                    let s = states.lock().await;
                    matches!(s.get(&id), Some(TrackState::Cancelled { .. }))
                };

                if !is_cancelled {
                    if finished {
                        let mut lru_guard = lru.lock().await;
                        lru_guard.retain(|&x| x != id);
                        lru_guard.push_back(id);
                    } else {
                        q.insert(id, task);
                    }
                }
                drop(q);
            } else {
                // IDLE STATE: No high priority tasks.
                // Check if we can do one chunk of a low priority task.
                let mut low_task_to_process = None;
                if let Some(id) = low_priority_id {
                    let mut q = queue.lock().await;
                    if let Some(task) = q.get(&id) {
                        if !task.downloader.is_complete() {
                            low_task_to_process = q.remove(&id).map(|t| (id, t));
                        } else {
                            // Already complete, remove from low priority
                            let mut playback = ctx.lock().await;
                            playback.low_priority_queue.retain(|&x| x != id);
                            q.remove(&id);

                            let mut lru_guard = lru.lock().await;
                            lru_guard.retain(|&x| x != id);
                            lru_guard.push_back(id);
                        }
                    } else {
                        // Task not in queue anymore
                        let mut playback = ctx.lock().await;
                        playback.low_priority_queue.retain(|&x| x != id);
                    }
                }

                if let Some((id, mut task)) = low_task_to_process {
                    // Before starting low priority, wait a bit for potential commands (Throttling/Grace period)
                    tokio::select! {
                        cmd_opt = rx.recv() => {
                            // Put task back and handle command
                            let mut q = queue.lock().await;
                            q.insert(id, task);
                            if let Some(cmd) = cmd_opt {
                                if matches!(cmd, ManagerCommand::Shutdown()) { return; }
                                Self::handle_command(cmd, &queue, &states, &ctx, &config).await;
                            } else { return; }
                        }
                        _ = tokio::time::sleep(Duration::from_millis(500)) => {
                            // Grace period passed, download one chunk
                            let mut finished = false;
                            tokio::select! {
                                cmd_opt = rx.recv() => {
                                    if let Some(cmd) = cmd_opt { buffered_cmd = Some(cmd); }
                                    else { return; }
                                }
                                res = Self::process_task(&mut task, states.clone(), bandwidth_monitor.clone(), config.clone()) => {
                                    finished = res;
                                }
                            }

                            let mut q = queue.lock().await;
                            let is_cancelled = {
                                let s = states.lock().await;
                                matches!(s.get(&id), Some(TrackState::Cancelled { .. }))
                            };

                            if !is_cancelled {
                                if finished {
                                    let mut playback = ctx.lock().await;
                                    playback.low_priority_queue.retain(|&x| x != id);
                                    let mut lru_guard = lru.lock().await;
                                    lru_guard.retain(|&x| x != id);
                                    lru_guard.push_back(id);
                                } else {
                                    q.insert(id, task);
                                }
                            }
                        }
                    }
                } else {
                    // Truly idle, nothing even in low priority
                    if let Some(cmd) = rx.recv().await {
                        if matches!(cmd, ManagerCommand::Shutdown()) {
                            return;
                        }
                        Self::handle_command(cmd, &queue, &states, &ctx, &config).await;
                    } else {
                        return;
                    }
                }
            }
        }
    }
    async fn process_task(
        task: &mut DownloadTask,
        states: Arc<Mutex<HashMap<TrackId, TrackState>>>,
        bandwidth_monitor: Arc<BandwidthMonitor>,
        config: Arc<RwLock<ManagerConfig>>,
    ) -> bool {
        let mut s = states.lock().await;
        let state = s.get_mut(&task.track_id);
        if let Some(state) = state {
            match state {
                TrackState::Pending {
                    url,
                    priority: _,
                    buffer: _,
                    meta,
                } => {
                    let init = task.downloader.init().await;
                    match init {
                        Ok(()) => {
                            let mut called = false;
                            let meta = meta.clone();
                            if let Some(options) = &task.options {
                                if let Some(on_start) = &options.on_start {
                                    on_start(Ok(task.downloader.buffer()), meta.clone());
                                    called = true;
                                }
                            }
                            if !called {
                                let config = config.read().await;
                                if let Some(on_start) = &config.default_callbacks.on_start {
                                    on_start(
                                        task.track_id,
                                        Ok(task.downloader.buffer()),
                                        meta.clone(),
                                    );
                                }
                            }
                            *state = TrackState::Downloading {
                                url: std::mem::take(url),
                                buffer: task.downloader.buffer().into(),
                                meta,
                            };
                        }
                        Err(e) => {
                            let error_str = e.to_string();
                            let mut called = false;
                            let meta = meta.clone();
                            if let Some(options) = &task.options {
                                if let Some(on_finish) = &options.on_finish {
                                    on_finish(
                                        Err(StreamingError::DownloadFailed(error_str.clone())),
                                        meta.clone(),
                                    );
                                    called = true;
                                }
                            }
                            if !called {
                                let config = config.read().await;
                                if let Some(on_finish) = &config.default_callbacks.on_finish {
                                    on_finish(
                                        task.track_id,
                                        Err(StreamingError::DownloadFailed(error_str.clone())),
                                        meta.clone(),
                                    );
                                }
                            }
                            *state = TrackState::Failed {
                                url: std::mem::take(url),
                                error: error_str,
                                meta,
                            };
                        }
                    }
                    false
                }
                TrackState::Downloading { url, buffer, meta } => {
                    let start = std::time::Instant::now();
                    let result = task.downloader.download().await;
                    // let result: Result<usize, StreamingError> = Ok(0);
                    // task.downloader.buffer().profile();
                    let duration = start.elapsed();

                    match result {
                        Ok(bytes) => {
                            if bytes > 0 {
                                bandwidth_monitor.update(bytes, duration);
                                let mut called = false;
                                let meta = meta.clone();
                                if let Some(options) = &task.options {
                                    if let Some(on_progress) = &options.on_progress {
                                        on_progress(Ok(task.downloader.buffer()), meta.clone());
                                        called = true;
                                    }
                                }
                                if !called {
                                    let config = config.read().await;
                                    if let Some(on_progress) = &config.default_callbacks.on_progress
                                    {
                                        on_progress(
                                            task.track_id,
                                            Ok(task.downloader.buffer()),
                                            meta,
                                        );
                                    }
                                }
                            }

                            if task.downloader.is_complete() {
                                task.downloader.buffer().profile();
                                let mut called = false;
                                let meta = meta.clone();
                                if let Some(options) = &task.options {
                                    if let Some(on_finish) = &options.on_finish {
                                        on_finish(Ok(task.downloader.buffer()), meta.clone());
                                        called = true;
                                    }
                                }
                                if !called {
                                    let config = config.read().await;
                                    if let Some(on_finish) = &config.default_callbacks.on_finish {
                                        on_finish(
                                            task.track_id,
                                            Ok(task.downloader.buffer()),
                                            meta.clone(),
                                        );
                                    }
                                }
                                *state = TrackState::Complete {
                                    url: url.clone(),
                                    buffer: buffer.clone(),
                                    meta,
                                };
                                true
                            } else {
                                false
                            }
                        }
                        Err(e) => {
                            let mut called = false;
                            let meta = meta.clone();
                            if let Some(options) = &task.options {
                                if let Some(on_finish) = &options.on_finish {
                                    on_finish(Err(e.clone()), meta.clone());
                                    called = true;
                                }
                            }
                            if !called {
                                let config = config.read().await;
                                if let Some(on_finish) = &config.default_callbacks.on_finish {
                                    on_finish(task.track_id, Err(e.clone()), meta.clone());
                                }
                            }
                            *state = TrackState::Failed {
                                url: url.clone(),
                                error: e.to_string(),
                                meta,
                            };
                            true
                        }
                    }
                }
                TrackState::Failed { .. } => true,
                TrackState::Complete { .. } => true,
                TrackState::Cancelled { .. } => true,
            }
        } else {
            false
        }
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
            TaskDownloader::new(
                stream_type,
                bandwidth_bps,
                user_agent,
                cfg.waveform_enabled,
                cfg.chunk_size,
            )
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
