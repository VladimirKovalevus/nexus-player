use super::AppState;
use crate::audio::types::Track as AudioTrack;
use crate::library::TrackId;
use crate::playlist::cursor::{ContainerContext, RadioMode};
use crate::playlist::playback_queue::{ActiveQueue, RepeatMode};
use crate::playlist::radio;
use anyhow::{Context, Result};

impl AppState {
    // --- Queue Management ---

    pub async fn add_to_queue(&self, track_id: TrackId) -> Result<()> {
        let library_track = self
            .library
            .get_track(track_id)
            .await?
            .context("Track not found")?;

        let audio_track: AudioTrack = library_track.into();

        let mut queue = self.queue.lock().await;
        let ActiveQueue::Cursor(cq) = &mut *queue;
        cq.add_track(audio_track);

        Ok(())
    }

    pub async fn clear_queue(&self) {
        let mut queue = self.queue.lock().await;
        let ActiveQueue::Cursor(cq) = &mut *queue;
        cq.clear();
    }

    pub async fn remove_from_queue(&self, index: usize) -> Option<AudioTrack> {
        let mut queue = self.queue.lock().await;
        let ActiveQueue::Cursor(cq) = &mut *queue;
        let removed = cq.remove_track(index);
        if removed.is_some() {
            tracing::info!("Removed track at index {}", index);
        }
        removed
    }

    pub async fn set_repeat_mode(&self, mode: RepeatMode) {
        let mut queue = self.queue.lock().await;
        let ActiveQueue::Cursor(cq) = &mut *queue;
        cq.repeat_mode = mode;
        tracing::info!("Repeat mode set to {:?}", mode);
    }

    pub async fn get_repeat_mode(&self) -> RepeatMode {
        let queue = self.queue.lock().await;
        let ActiveQueue::Cursor(cq) = &*queue;
        cq.repeat_mode
    }

    pub async fn toggle_shuffle(&self) {
        let mut queue = self.queue.lock().await;
        queue.toggle_shuffle();
        tracing::info!("Shuffle toggled");
    }

    pub async fn is_shuffle_enabled(&self) -> bool {
        let queue = self.queue.lock().await;
        let ActiveQueue::Cursor(cq) = &*queue;
        cq.shuffle_enabled
    }

    /// Builds a radio order for `mode`, freezes it into the queue context and starts
    /// playing from the top.
    pub async fn start_radio(&self, mode: RadioMode) -> Result<()> {
        let track_ids = radio::build_radio_order(&self.library, mode).await?;
        if track_ids.is_empty() {
            anyhow::bail!("No tracks available for radio");
        }
        tracing::info!("Starting radio {:?} with {} tracks", mode, track_ids.len());

        // Radio *is* the shuffle. Leaving `shuffle_enabled` on would make CursorQueue::next()
        // re-randomise inside the prefetch window, destroying the frozen order — and the
        // "up next" list, which always reports manual_queue in insertion order.
        {
            let mut queue = self.queue.lock().await;
            let ActiveQueue::Cursor(cq) = &mut *queue;
            cq.shuffle_enabled = false;
        }

        self.load_context_and_play(ContainerContext::Radio { mode, track_ids }, 0)
            .await
    }

    pub async fn stop_radio(&self) {
        self.stop();
        self.clear_queue().await;
        tracing::info!("Radio stopped");
    }

    pub async fn get_current_track_index(&self) -> Option<usize> {
        let queue = self.queue.lock().await;
        let ActiveQueue::Cursor(cq) = &*queue;
        let (_, index) = cq.snapshot();
        index
    }

    pub async fn play_at_index(&self, index: usize) {
        let cancel = self.begin_exclusive_op().await;
        let track = {
            let mut queue = self.queue.lock().await;
            let ActiveQueue::Cursor(cq) = &mut *queue;
            match cq.play_queue_item(index, None).await {
                Ok(track) => track,
                Err(e) => {
                    tracing::error!("Failed to play queue row {}: {}", index, e);
                    None
                }
            }
        };
        self.play_cancellable(track, cancel).await;
    }

    /// Play a specific queue row, optionally with a track index within a container row.
    pub async fn play_queue_item(&self, item_index: usize, track_index: Option<usize>) {
        let track = {
            let mut queue = self.queue.lock().await;
            let ActiveQueue::Cursor(cq) = &mut *queue;
            match cq.play_queue_item(item_index, track_index).await {
                Ok(track) => track,
                Err(e) => {
                    tracing::error!("Failed to play queue row {}: {}", item_index, e);
                    None
                }
            }
        };
        if track.is_some() {
            self.play(track).await;
        }
    }

    /// Previews a container row of the queue: loads its tracks without moving playback.
    pub async fn expand_queue_container(&self, item_index: usize) -> Result<()> {
        let mut queue = self.queue.lock().await;
        let ActiveQueue::Cursor(cq) = &mut *queue;
        cq.expand_container(item_index)
            .await
            .context("Failed to expand container")
    }
}
