use super::AppState;
use crate::playlist::playback_queue::{ActiveQueue, RepeatMode};

impl AppState {
    // --- Playback Control ---

    pub async fn play_next(&self) {
        let cancel = self.begin_exclusive_op().await;
        loop {
            let next = {
                let mut queue = self.queue.lock().await;
                let ActiveQueue::Cursor(cq) = &mut *queue;
                match cq.next().await {
                    Ok(next) => next,
                    Err(e) => {
                        tracing::error!("Error fetching next track: {}", e);
                        None
                    }
                }
            };

            if let Some(track) = next {
                tracing::info!("Next track: {}", track.id);
                self.play_cancellable(Some(track), cancel).await;
                return;
            }

            let (repeat_mode, context) = {
                let queue = self.queue.lock().await;
                let ActiveQueue::Cursor(cq) = &*queue;
                (cq.repeat_mode, cq.context.clone())
            };

            tracing::info!("End of queue reached. Repeat mode: {:?}", repeat_mode);

            if repeat_mode == RepeatMode::All {
                if let Some(ctx) = context {
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
            queue.peek_next()
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
        let previous = {
            let mut queue = self.queue.lock().await;
            let ActiveQueue::Cursor(cq) = &mut *queue;
            cq.previous()
        };

        if let Some(track) = &previous {
            tracing::info!("Previous track: {}", track.id);
        }
        self.play_cancellable(previous, cancel).await;
    }

    pub async fn handle_track_finished(&self, track_id: usize) {
        tracing::info!("Track finished: {}", track_id);
        self.play_next().await;
    }
}
