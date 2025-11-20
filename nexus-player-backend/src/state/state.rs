use crate::audio::engine::AudioEngine;
use crate::playlist::playlist::Playlist;
use crate::library::LibraryService;
use arc_swap::ArcSwap;
use std::sync::Arc;
use std::path::Path;

pub struct AppState {
    // Audio playback
    audio_engine: AudioEngine,

    // In-memory playback queue (lightweight audio::Track)
    playlist: ArcSwap<Playlist>,
    current_position: usize,

    // Library service (manages database, repositories, and library operations)
    library: LibraryService,
}

impl AppState {
    /// Create new AppState with initialized library and audio engine
    pub async fn new(db_path: &Path) -> anyhow::Result<Self> {
        // Initialize library service (database + repositories)
        let library = LibraryService::new(db_path).await?;

        // Initialize audio engine
        let audio_engine = AudioEngine::new()?;

        Ok(Self {
            audio_engine,
            current_position: 0,
            playlist: ArcSwap::new(Arc::new(Playlist::new())),
            library,
        })
    }

    // ========================================================================
    // Component Access
    // ========================================================================

    pub fn audio_engine(&self) -> &AudioEngine {
        &self.audio_engine
    }

    pub fn playlist(&self) -> Arc<Playlist> {
        self.playlist.load_full()
    }

    /// Get library service for track/playlist operations
    pub fn library(&self) -> &LibraryService {
        &self.library
    }

    // ========================================================================
    // Playback Queue Management
    // ========================================================================

    pub fn update_playlist<F>(&self, f: F)
    where
        F: FnOnce(&mut Playlist),
    {
        let current = self.playlist.load_full();

        let mut new_playlist = (*current).clone();

        f(&mut new_playlist);

        self.playlist.store(Arc::new(new_playlist));
    }

    pub fn set_playlist(&self, playlist: Playlist) {
        self.playlist.store(Arc::new(playlist));
    }

    pub fn play_next(&self) {
        let previous_index = if (self.current_position == self.playlist().len() - 1) {
            0
        } else {
            self.current_position + 1
        };

        let playlist = self.playlist();
        if let Some(track) = playlist.get_track(previous_index) {
            let mut track = track.clone();

            tracing::info!("Воспроизведение следующего трека: {}", &track.id);
            self.audio_engine.play(track);
        }
    }

    pub fn play_previous(&self) {
        let previous_index = if (self.current_position == 0) {
            0
        } else {
            self.current_position - 1
        };

        let playlist = self.playlist();
        if let Some(track) = playlist.get_track(previous_index) {
            let mut track = track.clone();

            tracing::info!("Воспроизведение предыдущего трека: {}", &track.id);
            self.audio_engine.play(track);
        }
    }

    pub fn handle_track_finished(&self, track_id: usize) {
        tracing::info!("Обработка завершения трека ID: {}", track_id);
    }
}
