use crate::audio::engine::AudioEngine;
use crate::audio::eq::{builtin_presets, EqSettings};
use crate::audio::types::Track;
use crate::audio::types::Track as AudioTrack;
use crate::config::{AppConfig, ConfigManager};
use crate::library::models::{
    Metadata, PlayHistoryEntry, Playlist as LibraryPlaylist, Track as LibraryTrack,
};
use crate::library::types::TrackOrder;
use crate::library::{LibraryService, PlaylistId, TrackId};
use crate::playlist::cursor::{AppCursorResolver, ContainerContext, PlayerCursor, RadioMode};
use crate::playlist::playback_queue::{
    ActiveQueue, ContainerKind, ContainerMeta, CursorQueue, NextResult, RepeatMode, SearchQueue,
    SearchQueueItem,
};
use crate::playlist::playlist::Playlist;
use crate::playlist::radio;
use crate::streaming::artwork::{ArtworkCache, ArtworkLocation};
use crate::streaming::types::ContentType;
use crate::streaming::StreamingService;
use anyhow::{Context, Result};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use streaming::images::{ImageVariant, OnReady};
use streaming::state::types::DefaultCallbacks;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

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

    // ========================================================================
    // Artwork cache
    // ========================================================================

    /// Resolve a batch of cover URLs to local files, queueing background fetches
    /// for the ones that aren't cached yet.
    ///
    /// Batched because the UI resolves a whole viewport at once: a virtualized
    /// list re-resolves every visible row on each scroll pass, and one call per
    /// row would trade a rendering stall for an IPC one.
    /// Only covers belonging to something saved in the library are fetched.
    /// Transient listings — search results above all — resolve to `None` and pull
    /// no traffic; the UI keeps showing the remote URL for those.
    pub async fn resolve_artwork(
        &self,
        urls: &[String],
        variant: ImageVariant,
    ) -> Vec<Option<String>> {
        let mut out = Vec::with_capacity(urls.len());
        let mut uncached = Vec::new();

        for url in urls {
            match self.artwork.lookup(url, variant) {
                ArtworkLocation::Local(path) => out.push(Some(path.to_string_lossy().into_owned())),
                ArtworkLocation::Passthrough(url) => out.push(Some(url)),
                ArtworkLocation::Pending => {
                    out.push(None);
                    uncached.push(url.clone());
                }
            }
        }

        if uncached.is_empty() {
            return out;
        }

        // Already-cached covers are served above without consulting the library:
        // the file exists, and re-checking would only cost a query.
        match self.library.known_artwork_urls(&uncached).await {
            Ok(known) => {
                for url in uncached.iter().filter(|url| known.contains(*url)) {
                    self.artwork.request(url, variant);
                }
            }
            Err(e) => {
                // Fetching nothing is the safe failure here — the remote URL still renders.
                tracing::warn!("Artwork library check failed, skipping prefetch: {}", e);
            }
        }

        out
    }

    /// Hold artwork fetching while audio is downloading, so covers never compete
    /// with playback for bandwidth.
    pub fn set_artwork_paused(&self, paused: bool) {
        self.artwork.set_paused(paused);
    }

    pub fn set_artwork_on_ready(&self, callback: Arc<OnReady>) {
        self.artwork.set_on_ready(callback);
    }

    pub async fn prune_artwork_cache(&self) -> anyhow::Result<()> {
        self.artwork.prune().await
    }

    pub async fn clear_artwork_cache(&self) -> anyhow::Result<()> {
        self.artwork.clear().await
    }

    pub fn config(&self) -> AppConfig {
        self.config_manager.get()
    }

    pub async fn update_config(&self, config: AppConfig) -> anyhow::Result<()> {
        self.config_manager.update(config.clone())?;
        let fade_in_ms = if config.audio.fade_in_enabled {
            config.audio.fade_in_ms
        } else {
            0
        };
        let fade_out_ms = if config.audio.fade_out_enabled {
            config.audio.fade_out_ms
        } else {
            0
        };
        self.audio_engine.set_fade(fade_in_ms, fade_out_ms);
        self.streaming
            .update_config(config.providers, config.audio, config.network)
            .await;
        Ok(())
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

    pub async fn handle_track_finished(&self, track_id: usize) {
        tracing::info!("Track finished: {}", track_id);
        self.play_next().await;
    }

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

        let mut queue = self.queue.lock().await;
        if let ActiveQueue::Search(sq) = &mut *queue {
            sq.append_results(items, has_more);
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

    // --- Audio Control Proxies ---

    pub fn pause(&self) {
        self.audio_engine.pause();
    }
    pub fn resume(&self) {
        self.audio_engine.resume();
    }
    pub fn stop(&self) {
        self.audio_engine.stop();
    }
    pub fn seek(&self, position: Duration) {
        self.audio_engine.seek(position);
    }
    pub fn set_volume(&self, volume: f32) {
        self.audio_engine.set_volume(volume.clamp(0.0, 1.0));
    }
    pub fn audio_state(&self) -> crate::audio::types::AudioState {
        self.audio_engine.state()
    }
    pub fn poll_audio_events(&self) -> Vec<crate::audio::types::AudioEvent> {
        self.audio_engine.poll_all_events()
    }

    pub fn set_eq_settings(&self, settings: EqSettings) -> anyhow::Result<()> {
        self.audio_engine.set_eq(settings.clone());
        let mut config = self.config_manager.get();
        config.eq = settings;
        self.config_manager.update(config)?;
        Ok(())
    }

    pub fn get_eq_settings(&self) -> EqSettings {
        self.config_manager.get().eq
    }

    pub fn get_eq_presets(&self) -> Vec<(String, [f32; 10])> {
        builtin_presets()
            .into_iter()
            .map(|(name, gains)| (name.to_string(), gains))
            .collect()
    }

    // --- Library Proxies (unchanged mostly) ---

    pub async fn add_track_to_library(&self, track: LibraryTrack) -> Result<TrackId> {
        self.library.add_track(track).await
    }

    pub async fn get_all_library_tracks(&self, order: TrackOrder) -> Result<Vec<LibraryTrack>> {
        self.library.get_all_tracks(order).await
    }

    pub async fn get_cached_library_tracks(&self, order: TrackOrder) -> Result<Vec<LibraryTrack>> {
        self.library.get_cached_tracks(order).await
    }

    pub async fn get_cached_library_albums(
        &self,
    ) -> Result<Vec<crate::library::models::CachedAlbumSummary>> {
        self.library.get_cached_albums().await
    }

    pub async fn search_library_tracks(&self, query: &str) -> Result<Vec<LibraryTrack>> {
        self.library.search_tracks(query).await
    }

    pub async fn delete_track_from_library(&self, track_id: TrackId) -> Result<()> {
        self.library.delete_track(track_id).await
    }

    pub async fn get_library_track(&self, track_id: TrackId) -> Result<Option<LibraryTrack>> {
        self.library.get_track(track_id).await
    }

    pub async fn update_track(&self, track: LibraryTrack) -> Result<()> {
        self.library.update_track(track).await
    }

    pub async fn create_playlist(
        &self,
        title: String,
        description: Option<String>,
        creator: Option<String>,
        artwork_url: Option<String>,
    ) -> Result<PlaylistId> {
        let artwork = artwork_url.map(|url| crate::library::types::Artwork {
            id: crate::library::types::ArtworkId(0),
            source: crate::library::types::ArtworkSource::Url { url },
            width: None,
            height: None,
            created_at: chrono::Utc::now(),
        });

        let playlist = LibraryPlaylist {
            id: PlaylistId(0),
            metadata: Metadata {
                title,
                description,
                artwork,
            },
            creator,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        self.library.create_playlist(playlist).await
    }

    pub async fn get_all_playlists(&self) -> Result<Vec<LibraryPlaylist>> {
        self.library.get_all_playlists().await
    }

    pub async fn get_playlist(
        &self,
        playlist_id: PlaylistId,
    ) -> Result<Option<crate::library::models::Playlist>> {
        self.library.get_playlist(playlist_id).await
    }

    pub async fn get_playlist_with_items(
        &self,
        playlist_id: PlaylistId,
    ) -> Result<Option<crate::library::models::PlaylistWithItems>> {
        self.library.get_playlist_with_items(playlist_id).await
    }

    pub async fn add_playlist_to_playlist(
        &self,
        parent_id: PlaylistId,
        child_id: PlaylistId,
    ) -> Result<()> {
        self.library
            .add_playlist_to_playlist(parent_id, child_id)
            .await
    }

    pub async fn remove_playlist_from_playlist(
        &self,
        parent_id: PlaylistId,
        child_id: PlaylistId,
    ) -> Result<()> {
        self.library
            .remove_playlist_from_playlist(parent_id, child_id)
            .await
    }

    pub async fn add_track_to_playlist(
        &self,
        playlist_id: PlaylistId,
        track: AudioTrack,
    ) -> Result<()> {
        let library_track: LibraryTrack = track.into();
        let track_id = self.library.ensure_track(library_track).await?;
        tracing::info!("ENSURE TRACK {track_id:?}");

        self.library
            .add_track_to_playlist(playlist_id, track_id)
            .await
    }

    pub async fn remove_track_from_playlist(
        &self,
        playlist_id: PlaylistId,
        track_id: TrackId,
    ) -> Result<()> {
        self.library
            .remove_track_from_playlist(playlist_id, track_id)
            .await
    }

    pub async fn delete_playlist(&self, playlist_id: PlaylistId) -> Result<()> {
        self.library.delete_playlist(playlist_id).await?;
        // The playlist's tracks are now orphaned — reclaim the ones that aren't liked.
        // Tracks still referenced by another playlist are kept, otherwise deleting one
        // playlist would silently gut the others.
        self.library.cleanup_library().await?;
        Ok(())
    }

    /// Manual library cleanup. Returns (tracks_deleted, artworks_deleted).
    pub async fn cleanup_library(&self) -> Result<(u64, u64)> {
        self.library.cleanup_library().await
    }

    pub async fn create_playlist_with_tracks(
        &self,
        title: String,
        description: Option<String>,
        artwork_url: Option<String>,
        tracks: Vec<AudioTrack>,
    ) -> Result<PlaylistId> {
        let mut playlist = Playlist::new(title);
        playlist.description = description;
        playlist.artwork = artwork_url;
        playlist.add_tracks(tracks);
        let result = self.library.create_playlist_with_tracks(playlist).await?;
        Ok(result.playlist.id)
    }

    pub async fn create_playlist_from_container(
        &self,
        provider: String,
        content_type: ContentType,
        id: String,
        title: String,
        description: Option<String>,
        artwork_url: Option<String>,
    ) -> Result<PlaylistId> {
        let track_infos = self
            .streaming
            .expand_collection(&provider, content_type, &id)
            .await
            .context("Failed to expand container")?;

        let audio_tracks: Vec<AudioTrack> = track_infos.into_iter().map(|t| t.into()).collect();

        self.create_playlist_with_tracks(title, description, artwork_url, audio_tracks)
            .await
    }

    // --- Liked Tracks ---

    pub async fn toggle_liked(&self, track: AudioTrack) -> Result<()> {
        let library_track: LibraryTrack = track.into();
        let track_id = self.library.ensure_track(library_track).await?;
        self.library.toggle_liked(track_id).await
    }

    pub async fn reorder_liked_tracks(&self, track_ids: Vec<TrackId>) -> Result<()> {
        self.library.reorder_liked_tracks(track_ids).await
    }

    pub async fn reorder_playlist_items(
        &self,
        playlist_id: PlaylistId,
        item_ids: Vec<i64>,
    ) -> Result<()> {
        self.library
            .reorder_playlist_items(playlist_id, item_ids)
            .await
    }

    // --- Play History ---

    pub async fn get_play_history(
        &self,
        limit: usize,
        offset: usize,
    ) -> anyhow::Result<Vec<PlayHistoryEntry>> {
        self.library.get_play_history(limit, offset).await
    }

    pub async fn clear_play_history(&self) -> anyhow::Result<()> {
        self.library.clear_play_history().await
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
