use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::Duration,
};

use crate::{
    error::StreamingError,
    state::types::{DownloadTask, ManagerCommand, ManagerConfig, TrackId, TrackState},
};

use tokio::sync::{Mutex, RwLock};

use super::{BandwidthMonitor, DownloadManager, PlaybackContext};

impl DownloadManager {
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
                tracing::debug!(%id, ?pos, "seek received");
                let mut q = queue.lock().await;
                if let Some(task) = q.get_mut(&id) {
                    task.downloader.seek(pos);
                }
            }
            ManagerCommand::Wake => {}
            ManagerCommand::Shutdown() => {
                // handled in caller
            }
        }
    }

    pub(super) async fn event_loop(
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
}
