use super::AppState;
use crate::playlist::playback_queue::{ActiveQueue, CursorQueue};
use crate::streaming::types::ContentType;
use anyhow::{Context, Result};

impl AppState {
    pub async fn load_search_to_queue(
        &self,
        system: String,
        query: String,
        content_type: ContentType,
        selected_track_id: usize,
    ) -> Result<()> {
        tracing::info!("Loading search context: {} from {}", query, system);
        let cancel = self.begin_exclusive_op().await;
        self.fill_search_queue(&system, &query, content_type, selected_track_id, &cancel)
            .await
    }

    pub async fn load_search_and_play(
        &self,
        system: String,
        query: String,
        content_type: ContentType,
        selected_track_id: usize,
    ) -> Result<()> {
        let cancel = self.begin_exclusive_op().await;
        self.fill_search_queue(&system, &query, content_type, selected_track_id, &cancel)
            .await?;
        if cancel.is_cancelled() {
            return Ok(());
        }
        let track = {
            let queue = self.queue.lock().await;
            let ActiveQueue::Cursor(cq) = &*queue;
            cq.current_track.clone()
        };
        self.play_cancellable(track, cancel).await;
        Ok(())
    }

    /// Atomic search + play: cancels any in-flight operation, starts a `CursorQueue` on the
    /// search at the chosen result (`offset + selected_index`), and plays it.
    ///
    /// `result_id` guards against results that changed since the UI listed them: when the
    /// result at the position has another id the call fails instead of playing something else.
    #[allow(clippy::too_many_arguments)]
    pub async fn search_and_play(
        &self,
        system: String,
        query: String,
        content_type: Option<ContentType>,
        _limit: usize,
        offset: usize,
        selected_index: usize,
        result_id: Option<String>,
    ) -> Result<()> {
        let cancel = self.begin_exclusive_op().await;
        let position = offset + selected_index;
        let content_type = content_type.unwrap_or(ContentType::Track);

        // The new queue is built off to the side; the lock is only taken to swap it in.
        let (repeat_mode, shuffle_enabled) = {
            let queue = self.queue.lock().await;
            let ActiveQueue::Cursor(cq) = &*queue;
            (cq.repeat_mode, cq.shuffle_enabled)
        };
        let mut fresh = CursorQueue::new();
        fresh.repeat_mode = repeat_mode;
        fresh.shuffle_enabled = shuffle_enabled;

        let started = tokio::select! {
            r = async {
                let track = fresh
                    .start_search_at_checked(
                        self.streaming.clone(),
                        self.cursor_resolver.clone(),
                        &system,
                        &query,
                        content_type,
                        position,
                        result_id.as_deref(),
                    )
                    .await?;
                fresh.prefetch(50).await?;
                Ok::<_, anyhow::Error>(track)
            } => r.context("Failed to start search playback")?,
            _ = cancel.cancelled() => return Ok(()),
        };

        {
            let mut queue = self.queue.lock().await;
            *queue = ActiveQueue::Cursor(Box::new(fresh));
        }

        self.play_cancellable(Some(started), cancel).await;
        Ok(())
    }
}
