use super::AppState;
use crate::playlist::playback_queue::{ActiveQueue, NextResult, RepeatMode};

impl AppState {
    // --- Playback Control ---

    pub async fn play_next(&self) {
        let cancel = self.begin_exclusive_op().await;
        loop {
            let result = {
                let mut queue = self.queue.lock().await;
                match &mut *queue {
                    ActiveQueue::Cursor(cq) => match cq.next().await {
                        Ok(Some(track)) => NextResult::Track(track),
                        Ok(None) => NextResult::End,
                        Err(e) => {
                            tracing::error!("Error fetching next track: {}", e);
                            NextResult::End
                        }
                    },
                    ActiveQueue::Search(sq) => sq.advance(),
                }
            };

            match result {
                NextResult::Track(track) => {
                    tracing::info!("Next track: {}", track.id);
                    self.play_cancellable(Some(track), cancel).await;
                    return;
                }
                NextResult::NeedExpand(idx) => {
                    if let Err(e) = self.expand_search_container(idx).await {
                        tracing::error!("Failed to expand container {}: {}", idx, e);
                        self.play_cancellable(None, cancel).await;
                        return;
                    }
                    let track = {
                        let queue = self.queue.lock().await;
                        match &*queue {
                            ActiveQueue::Search(sq) => sq.current_track(),
                            _ => None,
                        }
                    };
                    self.play_cancellable(track, cancel).await;
                    return;
                }
                NextResult::NeedMorePages => {
                    if let Err(e) = self.fetch_more_search_results().await {
                        tracing::error!("Failed to fetch more pages: {}", e);
                        self.play_cancellable(None, cancel).await;
                        return;
                    }
                    // Loop again — sq.advance() will now find the newly loaded item at current+1
                }
                NextResult::End => {
                    let (repeat_mode, context, is_search) = {
                        let queue = self.queue.lock().await;
                        match &*queue {
                            ActiveQueue::Cursor(cq) => (cq.repeat_mode, cq.context.clone(), false),
                            ActiveQueue::Search(sq) => (sq.repeat_mode, None, true),
                        }
                    };

                    tracing::info!(
                        "End of queue reached. Repeat mode: {:?}, is_search: {}",
                        repeat_mode,
                        is_search
                    );

                    if repeat_mode == RepeatMode::All {
                        if is_search {
                            // SearchQueue handles it internally in advance(),
                            // but if it returned End, maybe something is wrong or items are empty
                            tracing::warn!("SearchQueue returned End even with Repeat::All");
                        } else if let Some(ctx) = context {
                            tracing::info!("Restarting context queue: {:?}", ctx);
                            if let Err(e) = self.fill_context_queue(ctx, &cancel).await {
                                tracing::error!("Failed to restart queue for Repeat::All: {}", e);
                            } else {
                                tracing::info!("Queue restarted successfully. Continuing loop...");
                                continue; // Loop again to get the first track of the restarted queue
                            }
                        } else {
                            tracing::warn!(
                                "End reached with Repeat::All but no context available to restart"
                            );
                        }
                    }

                    tracing::info!("Playback truly ended.");
                    self.play_cancellable(None, cancel).await;
                    return;
                }
            }
        }
    }

    pub async fn play_current(&self) -> anyhow::Result<()> {
        let cancel = self.begin_exclusive_op().await;
        let track = {
            let queue = self.queue.lock().await;
            queue.current_track()
        };

        if let Some(track) = track {
            self.play_cancellable(Some(track), cancel).await;
            Ok(())
        } else {
            Err(anyhow::anyhow!("No current track to play"))
        }
    }

    pub async fn preload_next_track(&self) {
        let next_track = {
            let queue = self.queue.lock().await;
            match &*queue {
                ActiveQueue::Cursor(cq) => cq.peek_next(),
                ActiveQueue::Search(sq) => sq.peek_next(),
            }
        };

        if let Some(track) = next_track {
            tracing::info!("Preloading next track: {}", track.id);
            match self
                .streaming()
                .resolve(track.source.clone(), track.id as u64, None)
                .await
            {
                Ok(wrapper) => {
                    if let Err(e) = wrapper.prepare().await {
                        tracing::warn!("Preload prepare failed: {}", e);
                        return;
                    }

                    wrapper.preload(self.audio_engine(), track);
                }
                Err(e) => {
                    tracing::warn!("Error resolving track for preload: {}", e);
                }
            }
        }
    }

    pub async fn play_previous(&self) {
        let cancel = self.begin_exclusive_op().await;
        let result = {
            let mut queue = self.queue.lock().await;
            match &mut *queue {
                ActiveQueue::Cursor(cq) => match cq.previous() {
                    Some(track) => NextResult::Track(track),
                    None => NextResult::End,
                },
                ActiveQueue::Search(sq) => sq.previous(),
            }
        };

        match result {
            NextResult::Track(track) => {
                tracing::info!("Previous track: {}", track.id);
                self.play_cancellable(Some(track), cancel).await;
            }
            NextResult::NeedExpand(idx) => {
                if let Err(e) = self.expand_search_container(idx).await {
                    tracing::error!("Failed to expand container {}: {}", idx, e);
                    self.play_cancellable(None, cancel).await;
                    return;
                }
                let track = {
                    let queue = self.queue.lock().await;
                    match &*queue {
                        ActiveQueue::Search(sq) => sq.current_track(),
                        _ => None,
                    }
                };
                self.play_cancellable(track, cancel).await;
            }
            _ => {
                self.play_cancellable(None, cancel).await;
            }
        }
    }

    pub async fn handle_track_finished(&self, track_id: usize) {
        tracing::info!("Track finished: {}", track_id);
        self.play_next().await;
    }
}
