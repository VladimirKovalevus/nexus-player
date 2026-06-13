use super::AppState;
use crate::audio::types::Track as AudioTrack;
use crate::library::types::TrackOrder;
use crate::library::{PlaylistId, TrackId};
use crate::playlist::cursor::ContainerContext;
use crate::playlist::playback_queue::ActiveQueue;
use anyhow::Result;

impl AppState {
    /// Load a library page (determined by the clicked track's position) and play it.
    /// Cancels any in-flight exclusive operation before starting.
    pub async fn load_and_play_library_page_at_track(
        &self,
        track_id: TrackId,
        page_size: usize,
        order: TrackOrder,
    ) -> Result<(usize, usize, usize)> {
        let cancel = self.begin_exclusive_op().await;

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

        let total_pages = total.div_ceil(page_size);

        if cancel.is_cancelled() {
            return Ok((page, total_pages, track_index_in_page));
        }

        self.fill_cursor_queue(audio_tracks).await;

        let track = {
            let mut queue = self.queue.lock().await;
            match &mut *queue {
                ActiveQueue::Cursor(cq) => {
                    cq.jump_to(track_index_in_page);
                    cq.current_track.clone()
                }
                _ => None,
            }
        };

        if let Some(t) = track {
            self.play_cancellable(Some(t), cancel).await;
        }
        Ok((page, total_pages, track_index_in_page))
    }

    /// Load a specific library page into the queue and play the first track.
    /// Cancels any in-flight exclusive operation before starting.
    pub async fn load_and_play_library_page(
        &self,
        page: usize,
        page_size: usize,
        order: TrackOrder,
    ) -> Result<usize> {
        let cancel = self.begin_exclusive_op().await;

        let all_tracks = self.get_all_library_tracks(order).await?;
        let total = all_tracks.len();
        let page_start = (page * page_size).min(total);
        let page_end = (page_start + page_size).min(total);
        let audio_tracks: Vec<AudioTrack> = all_tracks[page_start..page_end]
            .iter()
            .map(|t| t.clone().into())
            .collect();

        let total_pages = total.div_ceil(page_size);

        if cancel.is_cancelled() {
            return Ok(total_pages);
        }

        self.fill_cursor_queue(audio_tracks).await;

        let track = {
            let mut queue = self.queue.lock().await;
            match &mut *queue {
                ActiveQueue::Cursor(cq) => {
                    cq.jump_to(0);
                    cq.current_track.clone()
                }
                _ => None,
            }
        };

        if let Some(t) = track {
            self.play_cancellable(Some(t), cancel).await;
        }
        Ok(total_pages)
    }

    pub async fn load_context_to_queue(&self, context: ContainerContext) -> Result<()> {
        tracing::info!("Loading context to queue: {:?}", context);
        let cancel = self.begin_exclusive_op().await;
        self.fill_context_queue(context, &cancel).await
    }

    pub async fn load_context_and_play(
        &self,
        context: ContainerContext,
        start_index: usize,
    ) -> Result<()> {
        let cancel = self.begin_exclusive_op().await;
        self.fill_context_queue(context, &cancel).await?;
        if cancel.is_cancelled() {
            return Ok(());
        }
        let track = {
            let mut queue = self.queue.lock().await;
            match &mut *queue {
                ActiveQueue::Cursor(cq) => {
                    cq.jump_to(start_index);
                    if cq.current_track.is_none() && start_index == 0 {
                        let _ = cq.next().await;
                    }
                    cq.current_track.clone()
                }
                ActiveQueue::Search(sq) => {
                    sq.jump_to(start_index);
                    sq.current_track()
                }
            }
        };
        self.play_cancellable(track, cancel).await;
        Ok(())
    }

    pub async fn load_playlist_to_queue(&self, playlist_id: PlaylistId) -> Result<()> {
        self.load_context_to_queue(ContainerContext::LocalPlaylist(playlist_id.0))
            .await
    }

    pub async fn load_playlist_and_play(
        &self,
        playlist_id: PlaylistId,
        start_index: usize,
    ) -> Result<()> {
        self.load_context_and_play(ContainerContext::LocalPlaylist(playlist_id.0), start_index)
            .await
    }
}
