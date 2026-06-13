use super::AppState;
use crate::audio::types::Track as AudioTrack;
use crate::library::types::TrackOrder;
use crate::library::TrackId;
use crate::playlist::cursor::{ContainerContext, RadioMode};
use crate::playlist::playback_queue::{ActiveQueue, CursorQueue, NextResult, RepeatMode};
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
        match &mut *queue {
            ActiveQueue::Cursor(cq) => cq.add_track(audio_track),
            ActiveQueue::Search(_) => {
                // Switch to cursor queue when manually adding
                let mut cq = CursorQueue::new();
                cq.add_track(audio_track);
                *queue = ActiveQueue::Cursor(Box::new(cq));
            }
        }

        Ok(())
    }

    pub async fn clear_queue(&self) {
        let mut queue = self.queue.lock().await;
        let (repeat, shuffle) = match &*queue {
            ActiveQueue::Cursor(cq) => (cq.repeat_mode, cq.shuffle_enabled),
            ActiveQueue::Search(sq) => (sq.repeat_mode, sq.shuffle_enabled),
        };
        match &mut *queue {
            ActiveQueue::Cursor(cq) => cq.clear(),
            ActiveQueue::Search(_) => {
                *queue = ActiveQueue::new_cursor_with_settings(repeat, shuffle);
            }
        }
    }

    pub async fn load_library_page_to_queue(
        &self,
        track_id: TrackId,
        page_size: usize,
        order: TrackOrder,
    ) -> Result<(usize, usize, usize)> {
        let all_tracks = self.get_all_library_tracks(order).await?;
        let total = all_tracks.len();

        let clicked_pos = all_tracks
            .iter()
            .position(|t| t.id == track_id)
            .unwrap_or(0);
        let page = clicked_pos / page_size;
        let page_start = page * page_size;
        let page_end = (page_start + page_size).min(total);
        let track_index_in_page = clicked_pos - page_start;

        let audio_tracks: Vec<AudioTrack> = all_tracks[page_start..page_end]
            .iter()
            .map(|t| t.clone().into())
            .collect();

        self.fill_cursor_queue(audio_tracks).await;

        let total_pages = total.div_ceil(page_size);
        Ok((page, total_pages, track_index_in_page))
    }

    pub async fn load_library_page(
        &self,
        page: usize,
        page_size: usize,
        order: TrackOrder,
    ) -> Result<usize> {
        let all_tracks = self.get_all_library_tracks(order).await?;
        let total = all_tracks.len();

        let page_start = (page * page_size).min(total);
        let page_end = (page_start + page_size).min(total);

        let audio_tracks: Vec<AudioTrack> = all_tracks[page_start..page_end]
            .iter()
            .map(|t| t.clone().into())
            .collect();

        self.fill_cursor_queue(audio_tracks).await;

        let total_pages = total.div_ceil(page_size);
        Ok(total_pages)
    }

    // --- Legacy / Missing Methods Stub ---
    // These might need to be re-implemented in CursorQueue or removed if not used by new frontend

    pub async fn remove_from_queue(&self, index: usize) -> Option<AudioTrack> {
        let mut queue = self.queue.lock().await;
        match &mut *queue {
            ActiveQueue::Cursor(cq) => {
                let removed = cq.remove_track(index);
                if removed.is_some() {
                    tracing::info!("Removed track at index {}", index);
                }
                removed
            }
            ActiveQueue::Search(_) => None, // Can't remove from search queue
        }
    }

    pub async fn set_repeat_mode(&self, mode: RepeatMode) {
        let mut queue = self.queue.lock().await;
        match &mut *queue {
            ActiveQueue::Cursor(cq) => cq.repeat_mode = mode,
            ActiveQueue::Search(sq) => sq.repeat_mode = mode,
        }
        tracing::info!("Repeat mode set to {:?}", mode);
    }

    pub async fn get_repeat_mode(&self) -> RepeatMode {
        let queue = self.queue.lock().await;
        match &*queue {
            ActiveQueue::Cursor(cq) => cq.repeat_mode,
            ActiveQueue::Search(sq) => sq.repeat_mode,
        }
    }

    pub async fn toggle_shuffle(&self) {
        let mut queue = self.queue.lock().await;
        queue.toggle_shuffle();
        tracing::info!("Shuffle toggled");
    }

    pub async fn is_shuffle_enabled(&self) -> bool {
        let queue = self.queue.lock().await;
        match &*queue {
            ActiveQueue::Cursor(cq) => cq.shuffle_enabled,
            ActiveQueue::Search(sq) => sq.shuffle_enabled,
        }
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
            match &mut *queue {
                ActiveQueue::Cursor(cq) => cq.shuffle_enabled = false,
                ActiveQueue::Search(sq) => sq.shuffle_enabled = false,
            }
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
        match &*queue {
            ActiveQueue::Cursor(cq) => {
                let (_, index) = cq.snapshot();
                index
            }
            ActiveQueue::Search(sq) => {
                let (_, position) = sq.snapshot();
                position.map(|p| p.item_index)
            }
        }
    }

    pub async fn play_at_index(&self, index: usize) {
        let cancel = self.begin_exclusive_op().await;
        let track = {
            let mut queue = self.queue.lock().await;
            match &mut *queue {
                ActiveQueue::Cursor(cq) => {
                    cq.jump_to(index);
                    cq.current_track.clone()
                }
                ActiveQueue::Search(sq) => {
                    sq.jump_to(index);
                    sq.current_track()
                }
            }
        };
        self.play_cancellable(track, cancel).await;
    }

    /// Play a specific item in the search queue, optionally with a track index within a container
    pub async fn play_queue_item(&self, item_index: usize, track_index: Option<usize>) {
        let result = {
            let mut queue = self.queue.lock().await;
            match &mut *queue {
                ActiveQueue::Search(sq) => {
                    if let Some(track_idx) = track_index {
                        sq.jump_to_with_track_index(item_index, track_idx);
                    } else {
                        sq.jump_to(item_index);
                    }
                    match sq.current_track() {
                        Some(track) => NextResult::Track(track),
                        None => {
                            // Might need expansion
                            let (entries, _) = sq.snapshot();
                            match entries.get(item_index) {
                                Some(crate::playlist::playback_queue::QueueEntry::Container {
                                    tracks: None,
                                    ..
                                }) => NextResult::NeedExpand(item_index),
                                _ => NextResult::End,
                            }
                        }
                    }
                }
                ActiveQueue::Cursor(cq) => {
                    cq.jump_to(item_index);
                    match cq.current_track.clone() {
                        Some(track) => NextResult::Track(track),
                        None => NextResult::End,
                    }
                }
            }
        };

        match result {
            NextResult::Track(track) => {
                self.play(Some(track)).await;
            }
            NextResult::NeedExpand(idx) => {
                if let Err(e) = self.expand_search_container(idx).await {
                    tracing::error!("Failed to expand: {}", e);
                    return;
                }
                // After expanding, the jump_to is already set, just get current track
                let track = {
                    let queue = self.queue.lock().await;
                    match &*queue {
                        ActiveQueue::Search(sq) => sq.current_track(),
                        _ => None,
                    }
                };
                self.play(track).await;
            }
            _ => (),
        }
    }
}
