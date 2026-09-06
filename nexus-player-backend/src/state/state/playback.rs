use super::AppState;
use crate::playlist::playback_queue::ActiveQueue;

impl AppState {
    // --- Playback Control ---

    pub async fn play_next(&self) {
        let cancel = self.begin_exclusive_op().await;
        // Repeat All restarts inside `CursorQueue::next`, so there is one restart path.
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

        match &next {
            Some(track) => tracing::info!("Next track: {}", track.id),
            None => tracing::info!("Playback truly ended."),
        }
        self.play_cancellable(next, cancel).await;
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
