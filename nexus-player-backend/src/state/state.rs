use crate::audio::engine::AudioEngine;
use crate::audio::types::Track;
use crate::audio::types::Track as AudioTrack;
use crate::config::ConfigManager;
use crate::library::models::PlayHistoryEntry;
use crate::library::{LibraryService, TrackId};
use crate::playlist::cursor::{AppCursorResolver, ContainerContext, PlayerCursor};
use crate::playlist::playback_queue::{ActiveQueue, ContainerKind, ContainerMeta, SearchQueueItem};
use crate::streaming::artwork::ArtworkCache;
use crate::streaming::types::ContentType;
use crate::streaming::StreamingService;
use anyhow::{Context, Result};
use std::path::Path;
use std::sync::Arc;
use streaming::state::types::DefaultCallbacks;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

mod artwork_config;
mod audio;
mod library_proxy;
mod loading;
mod playback;
mod queue;
mod search;

pub struct AppState {
    audio_engine: Arc<AudioEngine>,

    pub queue: Arc<Mutex<ActiveQueue>>,

    // NEW: Resolver for creating cursors
    pub cursor_resolver: Arc<AppCursorResolver>,

    library: Arc<LibraryService>,

    streaming: Arc<StreamingService>, // Made Arc to share with resolver

    artwork: Arc<ArtworkCache>,

    config_manager: ConfigManager,

    op_cancel: Mutex<CancellationToken>,
}

impl AppState {
    pub async fn new(db_path: &Path, default_callbacks: DefaultCallbacks) -> anyhow::Result<Self> {
        let library = LibraryService::new(db_path).await?;
        let library = Arc::new(library);

        let config_path = db_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join("config.json");
        let config_manager = ConfigManager::new(config_path)
            .map_err(|e| anyhow::anyhow!("Failed to load configuration: {}", e))?;

        let config = config_manager.get();

        let audio_engine = Arc::new(AudioEngine::new()?);
        // Apply saved EQ and fade settings on startup.
        audio_engine.set_eq(config.eq.clone());
        {
            let a = &config.audio;
            let fin = if a.fade_in_enabled { a.fade_in_ms } else { 0 };
            let fout = if a.fade_out_enabled { a.fade_out_ms } else { 0 };
            audio_engine.set_fade(fin, fout);
        }

        let cache_dir = db_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join("cache");
        let artwork = Arc::new(ArtworkCache::new(cache_dir.join("artwork")));

        let streaming = StreamingService::new(
            config.providers.clone(),
            config.audio.clone(),
            config.network.clone(),
            library.clone(),
            default_callbacks,
        )
        .map_err(|e| anyhow::anyhow!("Failed to create streaming service: {}", e))?;
        let streaming = Arc::new(streaming);

        let cursor_resolver = Arc::new(AppCursorResolver::new(library.clone(), streaming.clone()));

        Ok(Self {
            audio_engine,
            queue: Arc::new(Mutex::new(ActiveQueue::new_cursor())),
            cursor_resolver,
            library,
            streaming,
            artwork,
            config_manager,
            op_cancel: Mutex::new(CancellationToken::new()),
        })
    }

    pub fn audio_engine(&self) -> &AudioEngine {
        &self.audio_engine
    }

    pub async fn lock_queue(&self) -> tokio::sync::MutexGuard<'_, ActiveQueue> {
        self.queue.lock().await
    }

    pub fn library(&self) -> &LibraryService {
        &self.library
    }

    pub fn streaming(&self) -> &StreamingService {
        &self.streaming
    }

    /// Enrich tracks with 'liked' status from library using batch lookup
    pub async fn enrich_tracks(&self, mut tracks: Vec<Track>) -> Result<Vec<Track>> {
        let sources: Vec<_> = tracks.iter().map(|t| t.source.clone()).collect();
        let liked_sources = self.library.find_liked_by_sources(&sources).await?;

        for track in &mut tracks {
            track.liked = liked_sources.contains(&track.source);
        }
        Ok(tracks)
    }

    /// Search streaming providers, cancelling any in-flight exclusive operation first.
    pub async fn search_streaming_cancellable(
        &self,
        system: &str,
        content_type: Option<crate::streaming::types::ContentType>,
        query: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<crate::streaming::types::SearchResult>> {
        let cancel = self.begin_exclusive_op().await;
        tokio::select! {
            r = self.streaming.search(system, content_type, query, limit, offset) => Ok(r?),
            _ = cancel.cancelled() => Ok(vec![]),
        }
    }

    pub async fn direct_link_resolve(
        &self,
        provider: &str,
        url: &str,
    ) -> Result<crate::streaming::types::SearchResult> {
        Ok(self.streaming.direct_link_resolve(provider, url).await?)
    }

    async fn play(&self, track: Option<Track>) {
        if let Some(track) = track {
            self.audio_engine().set_loading(track.clone());

            // Record play history without blocking the playback path
            let lib = self.library.clone();
            let track_for_history = track.clone();
            tokio::spawn(async move {
                let entry = PlayHistoryEntry {
                    id: 0,
                    title: track_for_history.title.clone(),
                    artist: track_for_history.artist.clone(),
                    source: track_for_history.source.clone(),
                    artwork_url: track_for_history.artwork_url.clone(),
                    played_at: chrono::Utc::now().timestamp(),
                };
                let _ = lib.add_play_history(entry).await;
            });

            // Get upcoming tracks for audio preload and download manager
            let upcoming = {
                let queue = self.queue.lock().await;
                queue.peek_upcoming_tracks(9)
            };
            let active_ids: Vec<u64> = upcoming.iter().map(|t| t.id as u64).collect();

            // Preload next track in audio engine for gapless playback
            if let Some(next) = upcoming.first().cloned() {
                let streaming = self.streaming.clone();
                let engine = self.audio_engine.clone();
                tokio::spawn(async move {
                    if let Ok(wrapper) = streaming
                        .resolve(next.source.clone(), next.id as u64, None)
                        .await
                    {
                        if let Err(e) = wrapper.prepare().await {
                            tracing::warn!("Preload prepare failed for track {}: {}", next.id, e);
                            return;
                        }
                        wrapper.preload(&engine, next);
                    }
                });
            }

            // Resolve remaining upcoming tracks so they enter the download queue
            if upcoming.len() > 1 {
                let streaming = self.streaming.clone();
                let rest = upcoming[1..].to_vec();
                tokio::spawn(async move {
                    for t in rest {
                        let _ = streaming.resolve(t.source.clone(), t.id as u64, None).await;
                    }
                });
            }

            // Prepare source for current track
            match self
                .streaming()
                .resolve(track.source.clone(), track.id as u64, None)
                .await
            {
                Ok(wrapper) => {
                    self.streaming()
                        .set_context(Some(track.id as u64), active_ids)
                        .await;

                    if let Err(e) = wrapper.prepare().await {
                        tracing::warn!("Prepare failed: {}", e);
                        self.audio_engine().stop();
                        return;
                    }

                    wrapper.play(self.audio_engine(), track);
                }
                Err(e) => {
                    tracing::warn!("Error resolving track source: {}", e);
                    if let crate::streaming::error::StreamError::ChallengeRequired {
                        provider,
                        url,
                    } = e
                    {
                        self.audio_engine().push_event(
                            crate::audio::types::AudioEvent::ProviderChallenge { provider, url },
                        );
                    } else if matches!(e, crate::streaming::error::StreamError::Unavailable(_))
                        && track.state == crate::audio::types::TrackState::Saved
                    {
                        let id = track.id as i64;
                        self.audio_engine().push_event(
                            crate::audio::types::AudioEvent::TrackUnavailable { track_id: id },
                        );
                        let lib = self.library.clone();
                        tokio::spawn(async move {
                            let _ = lib.set_track_unavailable(TrackId(id), true).await;
                        });
                    }
                    self.audio_engine().stop();
                }
            }
        } else {
            // Stop if no track
            self.stop();
        }
    }

    async fn fill_cursor_queue(&self, tracks: Vec<AudioTrack>) {
        let mut queue = self.queue.lock().await;
        match &mut *queue {
            ActiveQueue::Cursor(cq) => {
                cq.clear();
                for track in tracks {
                    cq.add_track(track);
                }
            }
            _ => {
                *queue = ActiveQueue::new_cursor();
                if let ActiveQueue::Cursor(cq) = &mut *queue {
                    for track in tracks {
                        cq.add_track(track);
                    }
                }
            }
        }
    }

    // --- Cancellation mechanism ---

    /// Cancel any in-flight exclusive operation and return a fresh token for the new one.
    async fn begin_exclusive_op(&self) -> CancellationToken {
        let mut guard = self.op_cancel.lock().await;
        guard.cancel();
        let token = CancellationToken::new();
        *guard = token.clone();
        token
    }

    /// Like `play()` but stops silently if the cancel token is triggered before reaching the
    /// audio engine. Prevents stale play commands from a superseded operation reaching the driver.
    async fn play_cancellable(&self, track: Option<Track>, cancel: CancellationToken) {
        tracing::info!(
            "play_cancellable: start, track={:?}",
            track.as_ref().map(|t| (t.id, &t.title))
        );
        if let Some(track) = track {
            tracing::info!("play_cancellable: set_loading track_id={}", track.id);
            self.audio_engine().set_loading(track.clone());

            let upcoming = {
                let queue = self.queue.lock().await;
                queue.peek_upcoming_tracks(9)
            };
            tracing::info!("play_cancellable: upcoming tracks count={}", upcoming.len());
            let active_ids: Vec<u64> = upcoming.iter().map(|t| t.id as u64).collect();

            // Set context early so the download manager knows this track is high priority
            self.streaming()
                .set_context(Some(track.id as u64), active_ids.clone())
                .await;

            let lib = self.library.clone();
            let track_for_history = track.clone();
            tokio::spawn(async move {
                tracing::info!(
                    "play_cancellable[history]: saving play history for track_id={}",
                    track_for_history.id
                );
                let entry = PlayHistoryEntry {
                    id: 0,
                    title: track_for_history.title.clone(),
                    artist: track_for_history.artist.clone(),
                    source: track_for_history.source.clone(),
                    artwork_url: track_for_history.artwork_url.clone(),
                    played_at: chrono::Utc::now().timestamp(),
                };
                match lib.add_play_history(entry).await {
                    Ok(_) => tracing::info!(
                        "play_cancellable[history]: saved ok for track_id={}",
                        track_for_history.id
                    ),
                    Err(e) => tracing::warn!(
                        "play_cancellable[history]: failed for track_id={}: {}",
                        track_for_history.id,
                        e
                    ),
                }
            });

            // Preload next track into audio engine for gapless playback
            if let Some(next) = upcoming.first().cloned() {
                let streaming = self.streaming.clone();
                let engine = self.audio_engine.clone();
                let preload_cancel = cancel.clone();
                tokio::spawn(async move {
                    if preload_cancel.is_cancelled() {
                        return;
                    }
                    if let Ok(wrapper) = streaming
                        .resolve(next.source.clone(), next.id as u64, None)
                        .await
                    {
                        if let Err(e) = wrapper.prepare().await {
                            tracing::warn!(
                                "play_cancellable[preload]: prepare failed for track {}: {}",
                                next.id,
                                e
                            );
                            return;
                        }
                        if preload_cancel.is_cancelled() {
                            return;
                        }
                        wrapper.preload(&engine, next);
                    }
                });
            }

            // Resolve remaining upcoming tracks so they enter the download queue
            if upcoming.len() > 1 {
                tracing::info!(
                    "play_cancellable: spawning background resolve for {} remaining tracks",
                    upcoming.len() - 1
                );
                let streaming = self.streaming.clone();
                let rest = upcoming[1..].to_vec();
                let rest_cancel = cancel.clone();
                tokio::spawn(async move {
                    for t in rest {
                        if rest_cancel.is_cancelled() {
                            tracing::info!(
                                "play_cancellable[bg_resolve]: cancelled, stopping batch resolve"
                            );
                            return;
                        }
                        tracing::info!("play_cancellable[bg_resolve]: resolving track_id={}", t.id);
                        match streaming.resolve(t.source.clone(), t.id as u64, None).await {
                            Ok(_) => tracing::info!(
                                "play_cancellable[bg_resolve]: resolve ok track_id={}",
                                t.id
                            ),
                            Err(e) => tracing::warn!(
                                "play_cancellable[bg_resolve]: resolve failed track_id={}: {}",
                                t.id,
                                e
                            ),
                        }
                    }
                });
            }

            tracing::info!(
                "play_cancellable: resolving source for track_id={}",
                track.id
            );
            let resolve_result = tokio::select! {
                r = self.streaming().resolve(track.source.clone(), track.id as u64, None) => r,
                _ = cancel.cancelled() => {
                    tracing::info!("play_cancellable: cancelled during resolve for track_id={}", track.id);
                    return;
                },
            };

            match resolve_result {
                Ok(wrapper) => {
                    tracing::info!("play_cancellable: resolve ok for track_id={}", track.id);
                    if cancel.is_cancelled() {
                        tracing::info!(
                            "play_cancellable: cancelled after resolve for track_id={}",
                            track.id
                        );
                        return;
                    }

                    tracing::info!("play_cancellable: preparing track_id={}", track.id);
                    let prepare_result = tokio::select! {
                        r = wrapper.prepare() => r,
                        _ = cancel.cancelled() => {
                            tracing::info!("play_cancellable: cancelled during prepare for track_id={}", track.id);
                            return;
                        },
                    };

                    if let Err(e) = prepare_result {
                        tracing::warn!("Prepare failed: {}", e);
                        self.audio_engine().stop();
                        return;
                    }

                    if cancel.is_cancelled() {
                        tracing::info!(
                            "play_cancellable: cancelled after prepare for track_id={}",
                            track.id
                        );
                        return;
                    }
                    tracing::info!(
                        "play_cancellable: calling wrapper.play for track_id={}",
                        track.id
                    );
                    wrapper.play(self.audio_engine(), track);
                }
                Err(e) => {
                    tracing::warn!("Error resolving track source: {}", e);
                    if let crate::streaming::error::StreamError::ChallengeRequired {
                        provider,
                        url,
                    } = e
                    {
                        self.audio_engine().push_event(
                            crate::audio::types::AudioEvent::ProviderChallenge { provider, url },
                        );
                    } else if matches!(e, crate::streaming::error::StreamError::Unavailable(_))
                        && track.state == crate::audio::types::TrackState::Saved
                    {
                        let id = track.id as i64;
                        self.audio_engine().push_event(
                            crate::audio::types::AudioEvent::TrackUnavailable { track_id: id },
                        );
                        let lib = self.library.clone();
                        tokio::spawn(async move {
                            tracing::info!(
                                "play_cancellable[unavailable]: marking track_id={} as unavailable",
                                id
                            );
                            match lib.set_track_unavailable(TrackId(id), true).await {
                                Ok(_) => tracing::info!(
                                    "play_cancellable[unavailable]: marked ok track_id={}",
                                    id
                                ),
                                Err(e) => tracing::warn!(
                                    "play_cancellable[unavailable]: failed track_id={}: {}",
                                    id,
                                    e
                                ),
                            }
                        });
                    }
                    self.audio_engine().stop();
                }
            }
        } else {
            tracing::info!("play_cancellable: no track provided, stopping");
            self.stop();
        }
    }

    async fn fill_context_queue(
        &self,
        context: ContainerContext,
        cancel: &CancellationToken,
    ) -> Result<()> {
        let cursor = tokio::select! {
            r = PlayerCursor::from_context(context.clone(), self.cursor_resolver.clone()) => r?,
            _ = cancel.cancelled() => return Ok(()),
        };

        {
            let mut queue = self.queue.lock().await;
            let (repeat_mode, shuffle_enabled) = match &*queue {
                ActiveQueue::Cursor(cq) => (cq.repeat_mode, cq.shuffle_enabled),
                ActiveQueue::Search(sq) => (sq.repeat_mode, sq.shuffle_enabled),
            };

            let cq = match &mut *queue {
                ActiveQueue::Cursor(cq) => {
                    cq.clear();
                    cq
                }
                _ => {
                    *queue = ActiveQueue::new_cursor_with_settings(repeat_mode, shuffle_enabled);
                    match &mut *queue {
                        ActiveQueue::Cursor(cq) => cq,
                        _ => unreachable!(),
                    }
                }
            };
            cq.context = Some(context);
            cq.set_cursor(cursor);
            let prefetched = cq.prefetch(50).await?;
            tracing::info!("Prefetched {} tracks from cursor", prefetched);
        }

        tracing::info!("Queue updated and pre-filled");
        Ok(())
    }

    async fn fill_search_queue(
        &self,
        system: &str,
        query: &str,
        content_type: ContentType,
        selected_track_id: usize,
        cancel: &CancellationToken,
    ) -> Result<()> {
        let context = ContainerContext::SearchQuery {
            system: system.to_string(),
            query: query.to_string(),
            content_type,
        };
        let cursor = tokio::select! {
            r = PlayerCursor::from_context(context, self.cursor_resolver.clone()) => r?,
            _ = cancel.cancelled() => return Ok(()),
        };

        let mut queue = self.queue.lock().await;
        let cq = match &mut *queue {
            ActiveQueue::Cursor(cq) => {
                cq.clear();
                cq
            }
            _ => {
                *queue = ActiveQueue::new_cursor();
                match &mut *queue {
                    ActiveQueue::Cursor(cq) => cq,
                    _ => unreachable!(),
                }
            }
        };
        cq.set_cursor(cursor);

        let mut found = false;
        while let Ok(Some(track)) = cq.next().await {
            if track.id == selected_track_id {
                found = true;
                break;
            }
        }

        if !found {
            tracing::warn!(
                "Selected track {} not found in search results initial batch",
                selected_track_id
            );
        }

        cq.prefetch(50).await?;
        Ok(())
    }

    /// Fetch more search results for pagination
    async fn fetch_more_search_results(&self) -> Result<()> {
        use crate::streaming::types::SearchResult as SR;

        let (system, query, offset) = {
            let queue = self.queue.lock().await;
            match &*queue {
                ActiveQueue::Search(sq) => (sq.system.clone(), sq.query.clone(), sq.search_offset),
                _ => return Err(anyhow::anyhow!("Not in search queue mode")),
            }
        };

        let results = self
            .streaming
            .search(&system, None, &query, 50, offset)
            .await
            .context("Search pagination failed")?;

        let has_more = results.len() >= 50;
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
                    expanded_tracks: if album.tracks.is_empty() {
                        None
                    } else {
                        Some(album.tracks.into_iter().map(|t| t.into()).collect())
                    },
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
                    expanded_tracks: if playlist.tracks.is_empty() {
                        None
                    } else {
                        Some(playlist.tracks.into_iter().map(|t| t.into()).collect())
                    },
                    play_index: 0,
                },
                _ => SearchQueueItem::Track(Track {
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

        let mut queue = self.queue.lock().await;
        if let ActiveQueue::Search(sq) = &mut *queue {
            sq.append_results(items, has_more);
        }

        Ok(())
    }
}
