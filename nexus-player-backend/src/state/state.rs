use crate::audio::engine::AudioEngine;
use crate::audio::types::Track;
use crate::audio::types::Track as AudioTrack;
use crate::config::{AppConfig, ConfigManager};
use crate::library::models::{
    Metadata, PlayHistoryEntry, Playlist as LibraryPlaylist, Track as LibraryTrack,
};
use crate::library::types::TrackOrder;
use crate::library::{LibraryService, PlaylistId, TrackId};
use crate::playlist::cursor::{AppCursorResolver, ContainerContext, PlayerCursor};
use crate::playlist::playback_queue::{
    ActiveQueue, ContainerKind, ContainerMeta, CursorQueue, NextResult, RepeatMode, SearchQueue,
    SearchQueueItem,
};
use crate::playlist::playlist::Playlist;
use crate::streaming::types::ContentType;
use crate::streaming::StreamingService;
use anyhow::{Context, Result};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use streaming::state::types::DefaultCallbacks;
use tokio::sync::Mutex;

pub struct AppState {
    audio_engine: Arc<AudioEngine>,

    pub queue: Arc<Mutex<ActiveQueue>>,

    // NEW: Resolver for creating cursors
    pub cursor_resolver: Arc<AppCursorResolver>,

    library: Arc<LibraryService>,

    streaming: Arc<StreamingService>, // Made Arc to share with resolver

    config_manager: ConfigManager,
}

impl AppState {
    pub async fn new(db_path: &Path, default_callbacks: DefaultCallbacks) -> anyhow::Result<Self> {
        let library = LibraryService::new(db_path).await?;
        let library = Arc::new(library);

        let config_path = db_path.parent().unwrap().join("config.json");
        let config_manager = ConfigManager::new(config_path)
            .map_err(|e| anyhow::anyhow!("Failed to load configuration: {}", e))?;

        let config = config_manager.get();

        let audio_engine = Arc::new(AudioEngine::new()?);

        let _cache_dir = db_path.parent().unwrap().join("cache");
        let streaming = StreamingService::new(
            config.providers.clone(),
            config.audio.clone(),
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
            config_manager,
        })
    }

    pub fn config(&self) -> AppConfig {
        self.config_manager.get()
    }

    pub async fn update_config(&self, config: AppConfig) -> anyhow::Result<()> {
        self.config_manager.update(config.clone())?;
        self.streaming
            .update_config(config.providers, config.audio)
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
            if liked_sources.contains(&track.source) {
                track.liked = true;
            } else {
                track.liked = false;
            }
        }
        Ok(tracks)
    }

    // --- Playback Control ---

    pub async fn play_next(&self) {
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
                    ActiveQueue::Search(sq) => sq.next(),
                }
            };

            match result {
                NextResult::Track(track) => {
                    tracing::info!("Next track: {}", track.id);
                    self.play(Some(track)).await;
                    return;
                }
                NextResult::NeedExpand(idx) => {
                    if let Err(e) = self.expand_search_container(idx).await {
                        tracing::error!("Failed to expand container {}: {}", idx, e);
                        self.play(None).await;
                        return;
                    }
                    // Retry after expansion
                    continue;
                }
                NextResult::NeedMorePages => {
                    if let Err(e) = self.fetch_more_search_results().await {
                        tracing::error!("Failed to fetch more pages: {}", e);
                        self.play(None).await;
                        return;
                    }
                    // Retry after fetching
                    continue;
                }
                NextResult::End => {
                    tracing::info!("End of queue");
                    self.play(None).await;
                    return;
                }
            }
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
        loop {
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
                    self.play(Some(track)).await;
                    return;
                }
                NextResult::NeedExpand(idx) => {
                    if let Err(e) = self.expand_search_container(idx).await {
                        tracing::error!("Failed to expand container {}: {}", idx, e);
                        self.play(None).await;
                        return;
                    }
                    continue;
                }
                _ => {
                    self.play(None).await;
                    return;
                }
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

            // Get next track for preloading context
            let next_track = {
                let queue = self.queue.lock().await;
                queue.peek_next()
            };
            let next_track_id = next_track.as_ref().map(|t| t.id as u64);

            // Trigger resolution for next track to start its download/init process
            if let Some(next) = next_track {
                let streaming = self.streaming.clone();
                let nx = next.clone();
                let engine = self.audio_engine.clone();
                tokio::spawn(async move {
                    match streaming
                        .resolve(nx.source.clone(), nx.id.clone() as u64, None)
                        .await
                    {
                        Ok(wrapper) => wrapper.preload(&engine, nx),
                        Err(_) => {}
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
                        .set_context(track.id as u64, next_track_id)
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
                *queue = ActiveQueue::Cursor(cq);
            }
        }

        Ok(())
    }

    pub async fn clear_queue(&self) {
        let mut queue = self.queue.lock().await;
        match &mut *queue {
            ActiveQueue::Cursor(cq) => cq.clear(),
            ActiveQueue::Search(_) => {
                *queue = ActiveQueue::new_cursor();
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

        let total_pages = (total + page_size - 1) / page_size;
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

        let total_pages = (total + page_size - 1) / page_size;
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

    pub async fn load_context_to_queue(&self, context: ContainerContext) -> Result<()> {
        tracing::info!("Loading context to queue: {:?}", context);
        let cursor = PlayerCursor::from_context(context, self.cursor_resolver.clone()).await?;

        {
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

            if let Ok(Some(first)) = cq.next().await {
                tracing::info!("First track loaded: {}", first.id);
            }

            let prefetched = cq.prefetch(50).await?;
            tracing::info!("Prefetched {} tracks from cursor", prefetched);
        }

        tracing::info!("Queue updated and pre-filled");
        Ok(())
    }

    // Legacy method support (load playlist to manual queue)
    pub async fn load_playlist_to_queue(&self, playlist_id: PlaylistId) -> Result<()> {
        let context = ContainerContext::LocalPlaylist(playlist_id.0);
        self.load_context_to_queue(context).await
    }

    pub async fn load_search_to_queue(
        &self,
        system: String,
        query: String,
        content_type: ContentType,
        selected_track_id: usize,
    ) -> Result<()> {
        tracing::info!("Loading search context: {} from {}", query, system);
        let context = ContainerContext::SearchQuery {
            system: system.clone(),
            query: query.clone(),
            content_type,
        };
        let cursor = PlayerCursor::from_context(context, self.cursor_resolver.clone()).await?;

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
        use crate::streaming::types::{ContentType, SearchResult as SR};

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
                    expanded_tracks: if playlist.tracks.is_empty() {
                        None
                    } else {
                        None
                    },
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

    // --- Library Proxies (unchanged mostly) ---

    pub async fn add_track_to_library(&self, track: LibraryTrack) -> Result<TrackId> {
        self.library.add_track(track).await
    }

    pub async fn get_all_library_tracks(&self, order: TrackOrder) -> Result<Vec<LibraryTrack>> {
        self.library.get_all_tracks(order).await
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
        self.library.delete_playlist(playlist_id).await
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
        if let ActiveQueue::Cursor(cq) = &mut *queue {
            cq.repeat_mode = mode;
        }
        tracing::info!("Repeat mode set to {:?}", mode);
    }

    pub async fn get_repeat_mode(&self) -> RepeatMode {
        let queue = self.queue.lock().await;
        match &*queue {
            ActiveQueue::Cursor(cq) => cq.repeat_mode,
            ActiveQueue::Search(_) => RepeatMode::Off,
        }
    }

    pub async fn toggle_shuffle(&self) {
        let mut queue = self.queue.lock().await;
        if let ActiveQueue::Cursor(cq) = &mut *queue {
            cq.shuffle_enabled = !cq.shuffle_enabled;
            tracing::info!("Shuffle toggled to {}", cq.shuffle_enabled);
        }
    }

    pub async fn is_shuffle_enabled(&self) -> bool {
        let queue = self.queue.lock().await;
        match &*queue {
            ActiveQueue::Cursor(cq) => cq.shuffle_enabled,
            ActiveQueue::Search(_) => false,
        }
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

        if let Some(track) = track {
            self.play(Some(track)).await;
        }
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
                return;
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
                return;
            }
            _ => return,
        }
    }
}
