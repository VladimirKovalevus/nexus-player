use super::AppState;
use crate::audio::types::Track;
use crate::audio::types::Track as AudioTrack;
use crate::library::TrackId;
use crate::playlist::playback_queue::{
    ActiveQueue, ContainerKind, ContainerMeta, CursorQueue, SearchQueue, SearchQueueItem,
};
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
            let mut queue = self.queue.lock().await;
            match &mut *queue {
                ActiveQueue::Cursor(cq) => cq.current_track.clone(),
                _ => None,
            }
        };
        self.play_cancellable(track, cancel).await;
        Ok(())
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

    /// Load search results into a SearchQueue with full context (tracks, albums, playlists)
    pub async fn load_search_queue(
        &self,
        system: String,
        query: String,
        content_type: Option<crate::streaming::types::ContentType>,
        limit: usize,
        offset: usize,
        selected_index: usize,
    ) -> Result<()> {
        use crate::streaming::types::SearchResult as SR;

        tracing::info!(
            "Loading search queue: '{}' from {} (offset: {}, limit: {})",
            query,
            system,
            offset,
            limit
        );

        let results = self
            .streaming
            .search(&system, content_type, &query, limit, offset)
            .await
            .context("Search failed")?;

        let has_more = results.len() >= limit;
        let items: Vec<SearchQueueItem> = results
            .into_iter()
            .map(|r| match r {
                SR::Track(track) => SearchQueueItem::Track(track.into()),
                SR::Album(album) => SearchQueueItem::Container {
                    info: ContainerMeta {
                        kind: ContainerKind::Album,
                        id: album.id,
                        system: album.service.to_string(),
                        title: album.title,
                        artist: album.artist,
                        track_count: album.track_count,
                        thumbnail: album.thumbnail,
                    },
                    expanded_tracks: None,
                    play_index: 0,
                },
                SR::Playlist(playlist) => SearchQueueItem::Container {
                    info: ContainerMeta {
                        kind: ContainerKind::Playlist,
                        id: playlist.id,
                        system: playlist.service.to_string(),
                        title: playlist.title,
                        artist: playlist.creator,
                        track_count: playlist.track_count,
                        thumbnail: playlist.thumbnail,
                    },
                    expanded_tracks: None,
                    play_index: 0,
                },
                SR::Channel(_) => {
                    // Skip channels, treat as empty track placeholder
                    SearchQueueItem::Track(Track {
                        id: 0,
                        title: "Channel (unsupported)".to_string(),
                        artist: None,
                        album: None,
                        duration: None,
                        artwork_url: None,
                        format: None,
                        source: crate::audio::types::TrackSource::local("".to_string()),
                        state: crate::audio::types::TrackState::Streaming,
                        liked: false,
                    })
                }
            })
            .collect();

        let mut sq = SearchQueue::new(system.clone(), query.clone(), items, has_more);
        sq.jump_to(selected_index);

        // If selected item is a container without tracks, expand it
        let needs_expand = {
            let (entries, _) = sq.snapshot();
            if let Some(entry) = entries.get(selected_index) {
                matches!(
                    entry,
                    crate::playlist::playback_queue::QueueEntry::Container { tracks: None, .. }
                )
            } else {
                false
            }
        };

        let mut queue = self.queue.lock().await;
        *queue = ActiveQueue::Search(sq);
        drop(queue);

        if needs_expand {
            self.expand_search_container(selected_index).await?;
        }

        Ok(())
    }

    /// Atomic search + play: cancels any in-flight operation, searches, builds the SearchQueue,
    /// optionally expands a container, then plays the selected item via play_cancellable.
    pub async fn search_and_play(
        &self,
        system: String,
        query: String,
        content_type: Option<crate::streaming::types::ContentType>,
        limit: usize,
        offset: usize,
        selected_index: usize,
    ) -> Result<()> {
        use crate::streaming::types::SearchResult as SR;

        let cancel = self.begin_exclusive_op().await;

        let results = tokio::select! {
            r = self.streaming.search(&system, content_type, &query, limit, offset) => {
                r.context("Search failed")?
            }
            _ = cancel.cancelled() => return Ok(()),
        };

        let has_more = results.len() >= limit;
        let items: Vec<SearchQueueItem> = results
            .into_iter()
            .map(|r| match r {
                SR::Track(track) => SearchQueueItem::Track(track.into()),
                SR::Album(album) => SearchQueueItem::Container {
                    info: ContainerMeta {
                        kind: ContainerKind::Album,
                        id: album.id,
                        system: album.service.to_string(),
                        title: album.title,
                        artist: album.artist,
                        track_count: album.track_count,
                        thumbnail: album.thumbnail,
                    },
                    expanded_tracks: None,
                    play_index: 0,
                },
                SR::Playlist(playlist) => SearchQueueItem::Container {
                    info: ContainerMeta {
                        kind: ContainerKind::Playlist,
                        id: playlist.id,
                        system: playlist.service.to_string(),
                        title: playlist.title,
                        artist: playlist.creator,
                        track_count: playlist.track_count,
                        thumbnail: playlist.thumbnail,
                    },
                    expanded_tracks: None,
                    play_index: 0,
                },
                SR::Channel(_) => SearchQueueItem::Track(Track {
                    id: 0,
                    title: "Channel (unsupported)".to_string(),
                    artist: None,
                    album: None,
                    duration: None,
                    artwork_url: None,
                    format: None,
                    source: crate::audio::types::TrackSource::local("".to_string()),
                    state: crate::audio::types::TrackState::Streaming,
                    liked: false,
                }),
            })
            .collect();

        let mut sq = SearchQueue::new(system.clone(), query.clone(), items, has_more);
        sq.jump_to(selected_index);

        let needs_expand = {
            let (entries, _) = sq.snapshot();
            if let Some(entry) = entries.get(selected_index) {
                matches!(
                    entry,
                    crate::playlist::playback_queue::QueueEntry::Container { tracks: None, .. }
                )
            } else {
                false
            }
        };

        {
            let mut queue = self.queue.lock().await;
            *queue = ActiveQueue::Search(sq);
        }

        if needs_expand {
            let expand_result = tokio::select! {
                r = self.expand_search_container(selected_index) => r,
                _ = cancel.cancelled() => return Ok(()),
            };
            expand_result?;
        }

        let track = {
            let queue = self.queue.lock().await;
            match &*queue {
                ActiveQueue::Search(sq) => sq.current_track(),
                _ => None,
            }
        };

        self.play_cancellable(track, cancel).await;
        Ok(())
    }

    /// Expand a container in the search queue by loading its tracks
    pub async fn expand_search_container(&self, item_index: usize) -> Result<()> {
        use crate::streaming::types::ContentType;

        // Get container info while holding lock briefly
        let (kind, id, system) = {
            let queue = self.queue.lock().await;
            match &*queue {
                ActiveQueue::Search(sq) => {
                    let (entries, _) = sq.snapshot();
                    match entries.get(item_index) {
                        Some(crate::playlist::playback_queue::QueueEntry::Container {
                            info,
                            ..
                        }) => (info.kind.clone(), info.id.clone(), info.system.clone()),
                        _ => {
                            return Err(anyhow::anyhow!(
                                "Item at index {} is not a container",
                                item_index
                            ))
                        }
                    }
                }
                _ => return Err(anyhow::anyhow!("Not in search queue mode")),
            }
        };

        let content_type = match kind {
            ContainerKind::Album => ContentType::Album,
            ContainerKind::Playlist => ContentType::Playlist,
        };

        let track_infos = self
            .streaming
            .expand_collection(&system, content_type, &id)
            .await
            .context("Failed to expand container")?;

        let tracks: Vec<Track> = track_infos.into_iter().map(|t| t.into()).collect();
        tracing::info!(
            "Expanded container {} with {} tracks",
            item_index,
            tracks.len()
        );

        let mut queue = self.queue.lock().await;
        if let ActiveQueue::Search(sq) = &mut *queue {
            sq.set_container_tracks(item_index, tracks);
        }

        Ok(())
    }

    /// Play the current track from the search queue
    pub async fn play_current_search(&self) {
        let track = {
            let queue = self.queue.lock().await;
            match &*queue {
                ActiveQueue::Search(sq) => sq.current_track(),
                _ => None,
            }
        };
        self.play(track).await;
    }
}
