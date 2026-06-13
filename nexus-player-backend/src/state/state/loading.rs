use super::AppState;
use crate::audio::types::Track as AudioTrack;
use crate::library::types::TrackOrder;
use crate::library::{PlaylistId, TrackId};
use crate::playlist::cursor::ContainerContext;
use crate::playlist::playback_queue::{ActiveQueue, CursorQueue};
use anyhow::Result;

impl AppState {
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

    pub async fn add_track_and_play(&self, track: AudioTrack) -> Result<()> {
        let cancel = self.begin_exclusive_op().await;
        {
            let mut queue = self.queue.lock().await;
            let mut cq = CursorQueue::new();
            cq.add_track(track.clone());
            cq.jump_to(0);
            *queue = ActiveQueue::Cursor(Box::new(cq));
        }
        if cancel.is_cancelled() {
            return Ok(());
        }
        self.play_cancellable(Some(track), cancel).await;
        Ok(())
    }

    pub async fn play_library_track(&self, track_id: TrackId) -> Result<()> {
        let cancel = self.begin_exclusive_op().await;
        let lib_track = self
            .library
            .get_track(track_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Track not found"))?;
        let track: AudioTrack = lib_track.into();
        {
            let mut queue = self.queue.lock().await;
            let mut cq = CursorQueue::new();
            cq.add_track(track.clone());
            cq.jump_to(0);
            *queue = ActiveQueue::Cursor(Box::new(cq));
        }
        if cancel.is_cancelled() {
            return Ok(());
        }
        self.play_cancellable(Some(track), cancel).await;
        Ok(())
    }

    pub async fn load_and_play_local_tracks(
        &self,
        track_ids: Vec<TrackId>,
        selected_id: TrackId,
    ) -> Result<()> {
        let cancel = self.begin_exclusive_op().await;
        let tracks = self.library.local_tracks_by_ids(&track_ids).await?;
        let audio_tracks: Vec<AudioTrack> = tracks.into_iter().map(Into::into).collect();
        let selected_index = audio_tracks
            .iter()
            .position(|track| i64::try_from(track.id).ok() == Some(selected_id.0))
            .ok_or_else(|| anyhow::anyhow!("Selected local track is unavailable"))?;

        self.fill_cursor_queue(audio_tracks).await;
        let track = {
            let mut queue = self.queue.lock().await;
            match &mut *queue {
                ActiveQueue::Cursor(cq) => {
                    cq.jump_to(selected_index);
                    cq.current_track.clone()
                }
                _ => None,
            }
        };
        if !cancel.is_cancelled() {
            self.play_cancellable(track, cancel).await;
        }
        Ok(())
    }

    /// Plays an arbitrary, caller-ordered set of library tracks as a queue, jumping to
    /// `selected_id`. Unlike `load_and_play_local_tracks`, tracks need not be indexed as
    /// local files — this covers liked/cached/streaming-sourced library tracks too.
    pub async fn load_and_play_library_tracks(
        &self,
        track_ids: Vec<TrackId>,
        selected_id: TrackId,
    ) -> Result<()> {
        let cancel = self.begin_exclusive_op().await;
        let mut audio_tracks = Vec::with_capacity(track_ids.len());
        for id in &track_ids {
            if let Some(track) = self.library.get_track(*id).await? {
                audio_tracks.push(AudioTrack::from(track));
            }
        }
        let selected_index = audio_tracks
            .iter()
            .position(|track| i64::try_from(track.id).ok() == Some(selected_id.0))
            .ok_or_else(|| anyhow::anyhow!("Selected track is unavailable"))?;

        self.fill_cursor_queue(audio_tracks).await;
        let track = {
            let mut queue = self.queue.lock().await;
            match &mut *queue {
                ActiveQueue::Cursor(cq) => {
                    cq.jump_to(selected_index);
                    cq.current_track.clone()
                }
                _ => None,
            }
        };
        if !cancel.is_cancelled() {
            self.play_cancellable(track, cancel).await;
        }
        Ok(())
    }
}
