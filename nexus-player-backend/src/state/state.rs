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

    library: LibraryService,

    streaming: Arc<StreamingService>, // Made Arc to share with resolver

    config_manager: ConfigManager,
}

impl AppState {
    pub async fn new(db_path: &Path) -> anyhow::Result<Self> {
        let library = LibraryService::new(db_path).await?;
        let library_arc = Arc::new(library); // Create an Arc wrapper for shared use

        // We need to clone it back to keep ownership in struct (or change struct to hold Arc)
        // For now, let's keep LibraryService as owned in struct, but we need Arc for resolver.
        // Actually, LibraryService methods are async & use internal pool, but struct itself might be heavy?
        // Let's assume LibraryService is cheap to clone or we wrap it.
        // Looking at previous code, LibraryService has SqlitePool inside, so it's cheap to clone.
        let library_clone = library_arc.as_ref().clone(); 
        
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
        let streaming_arc = Arc::new(streaming);

        let cursor_resolver = Arc::new(AppCursorResolver::new(library_arc, streaming_arc.clone()));

        Ok(Self {
            audio_engine,
            queue: Arc::new(Mutex::new(CursorQueue::new())),
            cursor_resolver,
            library: library_clone,
            streaming: streaming_arc,
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
        // Create a new cursor from the context
        let cursor = PlayerCursor::from_context(context, self.cursor_resolver.clone()).await?;
        
        // Update queue
        let mut queue = self.queue.lock().await;
        queue.clear();
        queue.set_cursor(cursor);
        
        // Optionally start playing immediately? 
        // For now, the frontend usually calls play_next() after loading.
        Ok(())
    }

    // Legacy method support (load playlist to manual queue)
    pub async fn load_playlist_to_queue(&self, playlist_id: PlaylistId) -> Result<()> {
        let context = ContainerContext::LocalPlaylist(playlist_id.0);
        self.load_context_to_queue(context).await
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