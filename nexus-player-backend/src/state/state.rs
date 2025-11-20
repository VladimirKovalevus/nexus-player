use crate::audio::engine::AudioEngine;
use crate::audio::types::Track as AudioTrack;
use crate::library::models::{Metadata, Playlist as LibraryPlaylist, Track as LibraryTrack};
use crate::library::{LibraryService, PlaylistId, TrackId};
use crate::playlist::playlist::Playlist;
use anyhow::{Context, Result};
use arc_swap::ArcSwap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

pub struct AppState {
    audio_engine: AudioEngine,

    playlist: ArcSwap<Playlist>,
    current_position: usize,

    library: LibraryService,
}

impl AppState {
    pub async fn new(db_path: &Path) -> anyhow::Result<Self> {
        let library = LibraryService::new(db_path).await?;

        let audio_engine = AudioEngine::new()?;

        Ok(Self {
            audio_engine,
            current_position: 0,
            playlist: ArcSwap::new(Arc::new(Playlist::new())),
            library,
        })
    }

    pub fn audio_engine(&self) -> &AudioEngine {
        &self.audio_engine
    }

    pub fn playlist(&self) -> Arc<Playlist> {
        self.playlist.load_full()
    }

    pub fn library(&self) -> &LibraryService {
        &self.library
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
        let previous_index = if (self.current_position + 1 == self.playlist().len()) {
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

    pub async fn add_to_queue(&self, track_id: TrackId) -> Result<usize> {
        let library_track = self
            .library
            .get_track(track_id)
            .await?
            .context("Track not found in library")?;

        let audio_track: AudioTrack = library_track.into();

        let index = self.update_playlist_with_return(|playlist| playlist.add_track(audio_track));

        tracing::info!(
            "Трек {} добавлен в очередь на позицию {}",
            track_id.0,
            index
        );
        Ok(index)
    }

    pub fn remove_from_queue(&self, index: usize) -> Option<AudioTrack> {
        let mut result = None;
        self.update_playlist(|playlist| {
            result = playlist.remove_track(index);
        });

        if result.is_some() {
            tracing::info!("Трек удален из очереди с позиции {}", index);
        }

        result
    }

    pub fn clear_queue(&self) {
        self.update_playlist(|playlist| {
            playlist.clear();
        });
        tracing::info!("Очередь воспроизведения очищена");
    }

    pub fn get_queue(&self) -> Vec<AudioTrack> {
        self.playlist().tracks().to_vec()
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

        self.set_playlist(Playlist::new());
        self.update_playlist(|playlist| {
            playlist.add_tracks(audio_tracks);
        });

        tracing::info!(
            "Плейлист {} загружен в очередь ({} треков)",
            playlist_id.0,
            playlist_with_tracks.tracks.len()
        );

        Ok(())
    }

    pub fn play_at_index(&self, index: usize) {
        let playlist = self.playlist();
        if let Some(track) = playlist.get_track(index) {
            tracing::info!(
                "Воспроизведение трека на позиции {}: ID {}",
                index,
                track.id
            );
            self.audio_engine.play(track.clone());
        } else {
            tracing::warn!("Трек на позиции {} не найден", index);
        }
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

    fn update_playlist_with_return<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut Playlist) -> R,
    {
        let current = self.playlist.load_full();
        let mut new_playlist = (*current).clone();
        let result = f(&mut new_playlist);
        self.playlist.store(Arc::new(new_playlist));
        result
    }
}
