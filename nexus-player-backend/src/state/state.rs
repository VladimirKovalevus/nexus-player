use crate::audio::engine::AudioEngine;
use crate::playlist::playlist::Playlist;
use crate::library::{TrackRepository, PlaylistRepository, SqliteTrackRepository, SqlitePlaylistRepository};
use crate::db;
use arc_swap::ArcSwap;
use std::sync::Arc;
use std::path::Path;
use sqlx::SqlitePool;

pub struct AppState {
    
    audio_engine: AudioEngine,

    
    playlist: ArcSwap<Playlist>,
    current_position: usize,

    
    db_pool: SqlitePool,
 
    track_repo: Arc<dyn TrackRepository>,
    playlist_repo: Arc<dyn PlaylistRepository>,
}

impl AppState {
    
    pub async fn new(db_path: &Path) -> anyhow::Result<Self> {
        
        let db_pool = db::create_pool(db_path).await?;

        
        let track_repo: Arc<dyn TrackRepository> = Arc::new(SqliteTrackRepository::new(db_pool.clone()));
        let playlist_repo: Arc<dyn PlaylistRepository> = Arc::new(SqlitePlaylistRepository::new(db_pool.clone()));

        
        let audio_engine = AudioEngine::new()?;

        Ok(Self {
            audio_engine,
            current_position: 0,
            playlist: ArcSwap::new(Arc::new(Playlist::new())),
            db_pool,
            track_repo,
            playlist_repo,
        })
    }

    pub fn audio_engine(&self) -> &AudioEngine {
        &self.audio_engine
    }

    pub fn playlist(&self) -> Arc<Playlist> {
        self.playlist.load_full()
    }

    
    pub fn db_pool(&self) -> &SqlitePool {
        &self.db_pool
    }

    pub fn track_repo(&self) -> &Arc<dyn TrackRepository> {
        &self.track_repo
    }

    pub fn playlist_repo(&self) -> &Arc<dyn PlaylistRepository> {
        &self.playlist_repo
    }

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
