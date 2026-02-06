use crate::audio;
use crate::audio::engine::AudioEngine;
use crate::audio::types::Track;
use crate::audio::types::Track as AudioTrack;
use crate::config::{AppConfig, ConfigManager};
use crate::library::models::{Metadata, Playlist as LibraryPlaylist, Track as LibraryTrack};
use crate::library::{LibraryService, PlaylistId, TrackId};
use crate::playlist::playback_queue::{PlaybackQueue, RepeatMode, CursorQueue};
use crate::playlist::playlist::Playlist;
use crate::playlist::cursor::{AppCursorResolver, ContainerContext, PlayerCursor};
use crate::streaming::external::StreamingSourceWrapper;
use crate::streaming::StreamingService;
use anyhow::{Context, Result};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

pub struct AppState {
    audio_engine: AudioEngine,

    // NEW: Async Mutex wrapping the new CursorQueue
    pub queue: Arc<Mutex<CursorQueue>>,
    
    // NEW: Resolver for creating cursors
    pub cursor_resolver: Arc<AppCursorResolver>,

    library: Arc<LibraryService>,

    streaming: Arc<StreamingService>, // Made Arc to share with resolver

    config_manager: ConfigManager,
}

impl AppState {
    pub async fn new(db_path: &Path) -> anyhow::Result<Self> {
        let library = LibraryService::new(db_path).await?;
        let library = Arc::new(library);
        
        let config_path = db_path.parent().unwrap().join("config.json");
        let config_manager = ConfigManager::new(config_path)
            .map_err(|e| anyhow::anyhow!("Failed to load configuration: {}", e))?;

        let config = config_manager.get();

        let audio_engine = AudioEngine::new()?;
        
        let cache_dir = db_path.parent().unwrap().join("cache");
        let streaming = StreamingService::new(
            cache_dir,
            config.network.chunk_size as u64,
            config.providers.clone(),
        )
        .map_err(|e| anyhow::anyhow!("Failed to create streaming service: {}", e))?;
        let streaming = Arc::new(streaming);

        let cursor_resolver = Arc::new(AppCursorResolver::new(library.clone(), streaming.clone()));

        Ok(Self {
            audio_engine,
            queue: Arc::new(Mutex::new(CursorQueue::new())),
            cursor_resolver,
            library,
            streaming,
            config_manager,
        })
    }

    pub fn config(&self) -> AppConfig {
        self.config_manager.get()
    }

    pub fn update_config(&self, config: AppConfig) -> anyhow::Result<()> {
        self.config_manager.update(config.clone())?;
        self.streaming.update_config(config.providers);
        Ok(())
    }

    pub fn audio_engine(&self) -> &AudioEngine {
        &self.audio_engine
    }

    // Helper to get queue lock
    pub async fn lock_queue(&self) -> tokio::sync::MutexGuard<'_, CursorQueue> {
        self.queue.lock().await
    }

    pub fn library(&self) -> &LibraryService {
        &self.library
    }

    pub fn streaming(&self) -> &StreamingService {
        &self.streaming
    }

    // --- Playback Control ---

    pub async fn play_next(&self) {
        let track = {
            let mut queue = self.queue.lock().await;
            match queue.next().await {
                Ok(Some(track)) => {
                    tracing::info!("Next track: {}", track.id);
                    Some(track)
                }
                Ok(None) => {
                    tracing::info!("End of queue");
                    None
                }
                Err(e) => {
                    tracing::error!("Error fetching next track: {}", e);
                    None
                }
            }
        };

        self.play(track).await;
    }

    pub async fn play_previous(&self) {
        let track = {
            let mut queue = self.queue.lock().await;
            queue.previous()
        };
        
        if let Some(ref t) = track {
            tracing::info!("Previous track: {}", t.id);
        }

        self.play(track).await;
    }

    async fn play(&self, track: Option<Track>) {
        if let Some(track) = track {
            // Prepare source
            match self.streaming().resolve(track.source.clone(), track.id as u64).await {
                Ok(wrapper) => {
                    // Preload next track logic (simplified for now)
                    // In new architecture, we might need to peek the cursor. 
                    // For now, let's skip preloading to keep it simple or peek if queue supports it.
                    
                    self.streaming().set_context(track.id as u64, None).await;

                    if let Err(e) = wrapper.prepare().await {
                        tracing::warn!("Prepare failed: {}", e);
                        return;
                    }

                    match wrapper {
                        StreamingSourceWrapper::Streaming(source) => {
                            self.audio_engine().play_source(Box::new(source));
                        }
                        StreamingSourceWrapper::Hls(hls) => {
                            self.audio_engine().play_hls(hls);
                        }
                        StreamingSourceWrapper::Local(source) => {
                            self.audio_engine().play_source(source);
                        }
                    }
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
        let library_track = self.library.get_track(track_id).await?
            .context("Track not found")?;
        
        let audio_track: AudioTrack = library_track.into();
        
        let mut queue = self.queue.lock().await;
        queue.add_track(audio_track);
        
        Ok(())
    }

    pub async fn clear_queue(&self) {
        let mut queue = self.queue.lock().await;
        queue.clear();
    }

    pub async fn load_context_to_queue(&self, context: ContainerContext) -> Result<()> {
        tracing::info!("Loading context to queue: {:?}", context);
        // Create a new cursor from the context
        let cursor = PlayerCursor::from_context(context, self.cursor_resolver.clone()).await?;
        
        // Update queue
        {
            let mut queue = self.queue.lock().await;
            queue.clear();
            queue.set_cursor(cursor);
            
            // Pre-fill the current track
            if let Ok(Some(first)) = queue.next().await {
                tracing::info!("First track loaded: {}", first.id);
            }
            
            // Prefetch next 50 tracks to make them visible in UI
            let prefetched = queue.prefetch(50).await?;
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

    pub async fn load_search_to_queue(&self, system: String, query: String, selected_track_id: usize) -> Result<()> {
        tracing::info!("Loading search context: {} from {}", query, system);
        let context = ContainerContext::SearchQuery { system, query };
        let cursor = PlayerCursor::from_context(context, self.cursor_resolver.clone()).await?;
        
        let mut queue = self.queue.lock().await;
        queue.clear();
        queue.set_cursor(cursor);
        
        // Fast-forward to the selected track
        let mut found = false;
        while let Ok(Some(track)) = queue.next().await {
            if track.id == selected_track_id {
                found = true;
                break;
            }
        }

        if !found {
            tracing::warn!("Selected track {} not found in search results initial batch", selected_track_id);
        }
        
        // Prefetch next 50 tracks
        queue.prefetch(50).await?;
        
        Ok(())
    }
    
    // --- Audio Control Proxies ---

    pub fn pause(&self) { self.audio_engine.pause(); }
    pub fn resume(&self) { self.audio_engine.resume(); }
    pub fn stop(&self) { self.audio_engine.stop(); }
    pub fn seek(&self, position: Duration) { self.audio_engine.seek(position); }
    pub fn set_volume(&self, volume: f32) { self.audio_engine.set_volume(volume.clamp(0.0, 1.0)); }
    pub fn audio_state(&self) -> crate::audio::types::AudioState { self.audio_engine.state() }
    pub fn poll_audio_events(&self) -> Vec<crate::audio::types::AudioEvent> { self.audio_engine.poll_all_events() }

    // --- Library Proxies (unchanged mostly) ---
    
    pub async fn add_track_to_library(&self, track: LibraryTrack) -> Result<TrackId> {
        self.library.add_track(track).await
    }
    
    pub async fn get_all_library_tracks(&self) -> Result<Vec<LibraryTrack>> {
        self.library.get_all_tracks().await
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

    pub async fn create_playlist(&self, title: String, description: Option<String>) -> Result<PlaylistId> {
        let playlist = LibraryPlaylist {
            id: PlaylistId(0),
            metadata: Metadata { title, description, artwork: None },
            creator: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        self.library.create_playlist(playlist).await
    }

    pub async fn get_all_playlists(&self) -> Result<Vec<LibraryPlaylist>> {
        self.library.get_all_playlists().await
    }

    pub async fn get_playlist_with_tracks(&self, playlist_id: PlaylistId) -> Result<Option<crate::library::models::PlaylistWithTracks>> {
        self.library.get_playlist_with_tracks(playlist_id).await
    }

    pub async fn add_track_to_playlist(&self, playlist_id: PlaylistId, track_id: TrackId) -> Result<()> {
        self.library.add_track_to_playlist(playlist_id, track_id).await
    }

    pub async fn remove_track_from_playlist(&self, playlist_id: PlaylistId, track_id: TrackId) -> Result<()> {
        self.library.remove_track_from_playlist(playlist_id, track_id).await
    }

    pub async fn delete_playlist(&self, playlist_id: PlaylistId) -> Result<()> {
        self.library.delete_playlist(playlist_id).await
    }

    pub async fn create_playlist_with_tracks(&self, title: String, description: Option<String>, tracks: Vec<AudioTrack>) -> Result<PlaylistId> {
        let mut playlist = Playlist::new(title);
        playlist.description = description;
        playlist.add_tracks(tracks);
        let result = self.library.create_playlist_with_tracks(playlist).await?;
        Ok(result.playlist.id)
    }

    // --- Legacy / Missing Methods Stub ---
    // These might need to be re-implemented in CursorQueue or removed if not used by new frontend
    
    pub async fn remove_from_queue(&self, index: usize) -> Option<AudioTrack> {
        let mut queue = self.queue.lock().await;
        let removed = queue.remove_track(index);
        if removed.is_some() {
            tracing::info!("Removed track at index {}", index);
        }
        removed
    }

    pub async fn set_repeat_mode(&self, mode: RepeatMode) {
        let mut queue = self.queue.lock().await;
        queue.repeat_mode = mode;
        tracing::info!("Repeat mode set to {:?}", mode);
    }

    pub async fn get_repeat_mode(&self) -> RepeatMode {
        let queue = self.queue.lock().await;
        queue.repeat_mode
    }

    pub async fn toggle_shuffle(&self) {
        let mut queue = self.queue.lock().await;
        queue.shuffle_enabled = !queue.shuffle_enabled;
        tracing::info!("Shuffle toggled to {}", queue.shuffle_enabled);
    }

    pub async fn is_shuffle_enabled(&self) -> bool {
        let queue = self.queue.lock().await;
        queue.shuffle_enabled
    }

    pub async fn get_current_track_index(&self) -> Option<usize> {
        let queue = self.queue.lock().await;
        let (_, index) = queue.snapshot();
        index
    }

    pub async fn play_at_index(&self, index: usize) {
        // This is hard with CursorQueue because "index" is ambiguous (history? manual? cursor offset?)
        // For now, we only support jumping within manual_queue or history as presented in snapshot.
        // Assuming snapshot order: [history] + [current] + [manual]
        
        let (tracks, _) = {
            let queue = self.queue.lock().await;
            queue.snapshot()
        };

        if let Some(track) = tracks.get(index) {
             // Logic to "jump" is complex: we basically need to rebuild state.
             // Simplification: just play that track and clear current?
             // Or maybe just Play it directly.
             self.play(Some(track.clone())).await;
        }
    }
}