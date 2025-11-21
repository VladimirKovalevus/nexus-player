use crate::audio::engine::AudioEngine;
use crate::audio::types::Track as AudioTrack;
use crate::library::models::{Metadata, Playlist as LibraryPlaylist, Track as LibraryTrack};
use crate::library::{LibraryService, PlaylistId, TrackId};
use crate::playlist::playlist::Playlist;
use crate::playlist::playback_queue::{PlaybackQueue, RepeatMode};
use crate::streaming::StreamingService;
use anyhow::{Context, Result};
use arc_swap::ArcSwap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

pub struct AppState {
    audio_engine: AudioEngine,

    queue: ArcSwap<PlaybackQueue>,

    library: LibraryService,

    streaming: StreamingService,
}

impl AppState {
    pub async fn new(db_path: &Path) -> anyhow::Result<Self> {
        let library = LibraryService::new(db_path).await?;

        let audio_engine = AudioEngine::new()?;

        let cache_dir = db_path.parent().unwrap().join("cache");
        let streaming = StreamingService::new(cache_dir, 1024)
            .map_err(|e| anyhow::anyhow!("Failed to create streaming service: {}", e))?;

        Ok(Self {
            audio_engine,
            queue: ArcSwap::new(Arc::new(PlaybackQueue::new())),
            library,
            streaming,
        })
    }

    pub fn audio_engine(&self) -> &AudioEngine {
        &self.audio_engine
    }

    pub fn queue(&self) -> Arc<PlaybackQueue> {
        self.queue.load_full()
    }

    pub fn library(&self) -> &LibraryService {
        &self.library
    }

    pub fn streaming(&self) -> &StreamingService {
        &self.streaming
    }

    pub fn update_queue<F>(&self, f: F)
    where
        F: FnOnce(&mut PlaybackQueue),
    {
        let current = self.queue.load_full();
        let mut new_queue = (*current).clone();
        f(&mut new_queue);
        self.queue.store(Arc::new(new_queue));
    }

    pub fn update_queue_with_return<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut PlaybackQueue) -> R,
    {
        let current = self.queue.load_full();
        let mut new_queue = (*current).clone();
        let result = f(&mut new_queue);
        self.queue.store(Arc::new(new_queue));
        result
    }

    pub fn play_next(&self) {
        self.update_queue(|queue| {
            if let Some(track) = queue.next() {
                tracing::info!("Воспроизведение следующего трека: {}", track.id);
                self.audio_engine.play(track.clone());
            } else {
                tracing::info!("Достигнут конец очереди");
            }
        });
    }

    pub fn play_previous(&self) {
        self.update_queue(|queue| {
            if let Some(track) = queue.previous() {
                tracing::info!("Воспроизведение предыдущего трека: {}", track.id);
                self.audio_engine.play(track.clone());
            }
        });
    }

    pub fn handle_track_finished(&self, track_id: usize) {
        tracing::info!("Обработка завершения трека ID: {}", track_id);
    }

    pub async fn add_to_queue(&self, track_id: TrackId) -> Result<usize> {
        let library_track = self
            .library
            .get_track(track_id)
            .await?
            .context("Track not found in library")?;

        let audio_track: AudioTrack = library_track.into();

        let index = self.update_queue_with_return(|queue| queue.add_track(audio_track));

        tracing::info!(
            "Трек {} добавлен в очередь на позицию {}",
            track_id.0,
            index
        );
        Ok(index)
    }

    pub fn remove_from_queue(&self, index: usize) -> Option<AudioTrack> {
        let result = self.update_queue_with_return(|queue| queue.remove_track(index));

        if result.is_some() {
            tracing::info!("Трек удален из очереди с позиции {}", index);
        }

        result
    }

    pub fn clear_queue(&self) {
        self.update_queue(|queue| {
            queue.clear();
        });
        tracing::info!("Очередь воспроизведения очищена");
    }

    pub fn get_queue(&self) -> Vec<AudioTrack> {
        self.queue().tracks().to_vec()
    }

    pub async fn load_playlist_to_queue(&self, playlist_id: PlaylistId) -> Result<()> {
        let playlist_with_tracks = self
            .library
            .get_playlist_with_tracks(playlist_id)
            .await?
            .context("Playlist not found")?;

        let mut audio_tracks = Vec::new();
        for pt in &playlist_with_tracks.tracks {
            if let Some(library_track) = self.library.get_track(pt.track_id).await? {
                let audio_track: AudioTrack = library_track.into();
                audio_tracks.push(audio_track);
            }
        }

        let mut playlist = Playlist::new();
        playlist.add_tracks(audio_tracks);

        self.update_queue(|queue| {
            queue.load_from_playlist(playlist, None);
        });

        tracing::info!(
            "Плейлист {} загружен в очередь ({} треков)",
            playlist_id.0,
            playlist_with_tracks.tracks.len()
        );

        Ok(())
    }

    pub fn play_at_index(&self, index: usize) {
        self.update_queue(|queue| {
            if let Some(track) = queue.jump_to(index) {
                tracing::info!(
                    "Воспроизведение трека на позиции {}: ID {}",
                    index,
                    track.id
                );
                self.audio_engine.play(track.clone());
            } else {
                tracing::warn!("Трек на позиции {} не найден", index);
            }
        });
    }

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
        let clamped = volume.clamp(0.0, 1.0);
        self.audio_engine.set_volume(clamped);
    }

    pub fn audio_state(&self) -> crate::audio::types::AudioState {
        self.audio_engine.state()
    }

    pub fn poll_audio_events(&self) -> Vec<crate::audio::types::AudioEvent> {
        self.audio_engine.poll_all_events()
    }

    pub async fn add_track_to_library(&self, track: LibraryTrack) -> Result<TrackId> {
        let track_id = self.library.add_track(track).await?;
        tracing::info!("Трек {} добавлен в библиотеку", track_id.0);
        Ok(track_id)
    }

    pub async fn get_all_library_tracks(&self) -> Result<Vec<LibraryTrack>> {
        self.library.get_all_tracks().await
    }

    pub async fn search_library_tracks(&self, query: &str) -> Result<Vec<LibraryTrack>> {
        self.library.search_tracks(query).await
    }

    pub async fn delete_track_from_library(&self, track_id: TrackId) -> Result<()> {
        self.library.delete_track(track_id).await?;
        tracing::info!("Трек {} удален из библиотеки", track_id.0);
        Ok(())
    }

    pub async fn get_library_track(&self, track_id: TrackId) -> Result<Option<LibraryTrack>> {
        self.library.get_track(track_id).await
    }

    pub async fn create_playlist(
        &self,
        title: String,
        description: Option<String>,
    ) -> Result<PlaylistId> {
        let playlist = LibraryPlaylist {
            id: PlaylistId(0),
            metadata: Metadata {
                title,
                description,
                artwork: None,
            },
            creator: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let playlist_id = self.library.create_playlist(playlist).await?;
        tracing::info!("Плейлист {} создан", playlist_id.0);
        Ok(playlist_id)
    }

    pub async fn get_all_playlists(&self) -> Result<Vec<LibraryPlaylist>> {
        self.library.get_all_playlists().await
    }

    pub async fn get_playlist_with_tracks(
        &self,
        playlist_id: PlaylistId,
    ) -> Result<Option<crate::library::models::PlaylistWithTracks>> {
        self.library.get_playlist_with_tracks(playlist_id).await
    }

    pub async fn add_track_to_playlist(
        &self,
        playlist_id: PlaylistId,
        track_id: TrackId,
    ) -> Result<()> {
        self.library
            .add_track_to_playlist(playlist_id, track_id)
            .await?;
        tracing::info!("Трек {} добавлен в плейлист {}", track_id.0, playlist_id.0);
        Ok(())
    }

    pub async fn remove_track_from_playlist(
        &self,
        playlist_id: PlaylistId,
        track_id: TrackId,
    ) -> Result<()> {
        self.library
            .remove_track_from_playlist(playlist_id, track_id)
            .await?;
        tracing::info!("Трек {} удален из плейлиста {}", track_id.0, playlist_id.0);
        Ok(())
    }

    pub async fn delete_playlist(&self, playlist_id: PlaylistId) -> Result<()> {
        self.library.delete_playlist(playlist_id).await?;
        tracing::info!("Плейлист {} удален", playlist_id.0);
        Ok(())
    }

    pub fn set_repeat_mode(&self, mode: RepeatMode) {
        self.update_queue(|queue| {
            queue.set_repeat_mode(mode);
        });
        tracing::info!("Режим повтора изменен на {:?}", mode);
    }

    pub fn get_repeat_mode(&self) -> RepeatMode {
        self.queue().repeat_mode()
    }

    pub fn toggle_shuffle(&self) {
        self.update_queue(|queue| {
            queue.toggle_shuffle();
        });
        let enabled = self.queue().shuffle_enabled();
        tracing::info!("Shuffle {}", if enabled { "включен" } else { "выключен" });
    }

    pub fn is_shuffle_enabled(&self) -> bool {
        self.queue().shuffle_enabled()
    }

    pub fn get_current_track_index(&self) -> Option<usize> {
        self.queue().current_index()
    }
}
