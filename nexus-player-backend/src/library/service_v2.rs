use anyhow::{Context, Result};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue, DatabaseConnection, TransactionTrait};
use std::path::Path;
use std::sync::Arc;

use super::{
    db_v2::{establish_connection_with_migrations, SeaOrmPlaylistRepository, SeaOrmTrackRepository},
    entities_v2::{playlist, playlist_track, track},
    models, PlaylistId, PlaylistRepositoryV2, TrackId, TrackRepositoryV2,
};

pub struct LibraryServiceV2 {
    db: DatabaseConnection,
    track_repo: Arc<dyn TrackRepositoryV2>,
    playlist_repo: Arc<dyn PlaylistRepositoryV2>,
}

impl LibraryServiceV2 {
    pub async fn new(db_path: &Path) -> Result<Self> {
        let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
        let db = establish_connection_with_migrations(&db_url).await?;

        let track_repo: Arc<dyn TrackRepositoryV2> =
            Arc::new(SeaOrmTrackRepository::new(db.clone()));
        let playlist_repo: Arc<dyn PlaylistRepositoryV2> =
            Arc::new(SeaOrmPlaylistRepository::new(db.clone()));

        Ok(Self {
            db,
            track_repo,
            playlist_repo,
        })
    }

    pub async fn add_track(&self, track: models::Track) -> Result<TrackId> {
        self.track_repo.insert(track).await
    }

    pub async fn get_track(&self, id: TrackId) -> Result<Option<models::Track>> {
        self.track_repo.get(id).await
    }

    pub async fn get_all_tracks(&self) -> Result<Vec<models::Track>> {
        self.track_repo.list_all().await
    }

    pub async fn search_tracks(&self, query: &str) -> Result<Vec<models::Track>> {
        self.track_repo.search(query).await
    }

    pub async fn find_by_artist(&self, artist: &str) -> Result<Vec<models::Track>> {
        self.track_repo.find_by_artist(artist).await
    }

    pub async fn find_by_album(&self, album: &str) -> Result<Vec<models::Track>> {
        self.track_repo.find_by_album(album).await
    }

    pub async fn update_track(&self, track: models::Track) -> Result<()> {
        self.track_repo.update(track).await
    }

    pub async fn delete_track(&self, id: TrackId) -> Result<()> {
        self.track_repo.delete(id).await
    }

    pub async fn increment_play_count(&self, id: TrackId) -> Result<()> {
        self.track_repo.increment_play_count(id).await
    }

    pub async fn create_playlist(&self, playlist: models::Playlist) -> Result<PlaylistId> {
        self.playlist_repo.create(playlist).await
    }

    pub async fn get_playlist(&self, id: PlaylistId) -> Result<Option<models::Playlist>> {
        self.playlist_repo.get(id).await
    }

    pub async fn get_playlist_with_tracks(
        &self,
        id: PlaylistId,
    ) -> Result<Option<models::PlaylistWithTracks>> {
        self.playlist_repo.get_with_tracks(id).await
    }

    pub async fn get_all_playlists(&self) -> Result<Vec<models::Playlist>> {
        self.playlist_repo.list_all().await
    }

    pub async fn update_playlist(&self, playlist: models::Playlist) -> Result<()> {
        self.playlist_repo.update(playlist).await
    }

    pub async fn delete_playlist(&self, id: PlaylistId) -> Result<()> {
        self.playlist_repo.delete(id).await
    }

    pub async fn add_track_to_playlist(
        &self,
        playlist_id: PlaylistId,
        track_id: TrackId,
    ) -> Result<()> {
        let playlist = self
            .playlist_repo
            .get_with_tracks(playlist_id)
            .await?
            .context("Playlist not found")?;

        let position = playlist.tracks.len() as u32;

        self.playlist_repo
            .add_track(playlist_id, track_id, position)
            .await
    }

    pub async fn remove_track_from_playlist(
        &self,
        playlist_id: PlaylistId,
        track_id: TrackId,
    ) -> Result<()> {
        self.playlist_repo
            .remove_track(playlist_id, track_id)
            .await
    }

    pub async fn reorder_playlist_tracks(
        &self,
        playlist_id: PlaylistId,
        track_ids: Vec<TrackId>,
    ) -> Result<()> {
        self.playlist_repo
            .reorder_tracks(playlist_id, track_ids)
            .await
    }

    pub async fn create_playlist_with_tracks(
        &self,
        runtime_playlist: crate::playlist::playlist::Playlist,
    ) -> Result<models::PlaylistWithTracks> {
        let txn = self.db.begin().await?;

        let mut saved_track_ids = Vec::new();

        for audio_track in runtime_playlist.tracks() {
            let library_track: models::Track = audio_track.clone().into();
            let entity_track: track::Model = library_track.into();

            let active_track = track::ActiveModel {
                id: ActiveValue::NotSet,
                title: ActiveValue::Set(entity_track.title),
                artist: ActiveValue::Set(entity_track.artist),
                description: ActiveValue::Set(entity_track.description),
                artwork_id: ActiveValue::Set(entity_track.artwork_id),
                album: ActiveValue::Set(entity_track.album),
                album_artist: ActiveValue::Set(entity_track.album_artist),
                year: ActiveValue::Set(entity_track.year),
                genre: ActiveValue::Set(entity_track.genre),
                track_number: ActiveValue::Set(entity_track.track_number),
                source_type: ActiveValue::Set(entity_track.source_type),
                source_data: ActiveValue::Set(entity_track.source_data),
                cache_type: ActiveValue::Set(entity_track.cache_type),
                cache_data: ActiveValue::Set(entity_track.cache_data),
                file_format: ActiveValue::Set(entity_track.file_format),
                duration_secs: ActiveValue::Set(entity_track.duration_secs),
                sample_rate: ActiveValue::Set(entity_track.sample_rate),
                channels: ActiveValue::Set(entity_track.channels),
                bitrate: ActiveValue::Set(entity_track.bitrate),
                play_count: ActiveValue::Set(entity_track.play_count),
                last_played: ActiveValue::Set(entity_track.last_played),
                added_at: ActiveValue::Set(entity_track.added_at),
                updated_at: ActiveValue::Set(entity_track.updated_at),
            };

            let result = active_track
                .insert(&txn)
                .await
                .context("Failed to insert track")?;

            saved_track_ids.push(TrackId(result.id));
        }

        let now = Utc::now();
        let playlist_library = models::Playlist {
            id: PlaylistId(0),
            metadata: models::Metadata {
                title: runtime_playlist.title.clone(),
                description: runtime_playlist.description.clone(),
                artwork: None,
            },
            creator: None,
            created_at: now,
            updated_at: now,
        };

        let entity_playlist: playlist::Model = playlist_library.into();
        let active_playlist = playlist::ActiveModel {
            id: ActiveValue::NotSet,
            title: ActiveValue::Set(entity_playlist.title),
            description: ActiveValue::Set(entity_playlist.description),
            creator: ActiveValue::Set(entity_playlist.creator),
            artwork_id: ActiveValue::Set(entity_playlist.artwork_id),
            created_at: ActiveValue::Set(entity_playlist.created_at),
            updated_at: ActiveValue::Set(entity_playlist.updated_at),
        };

        let playlist_result = active_playlist
            .insert(&txn)
            .await
            .context("Failed to create playlist")?;

        let playlist_id = PlaylistId(playlist_result.id);

        let mut playlist_tracks = Vec::new();

        for (position, track_id) in saved_track_ids.iter().enumerate() {
            let now_ts = Utc::now().timestamp();

            let active_playlist_track = playlist_track::ActiveModel {
                playlist_id: ActiveValue::Set(playlist_id.0),
                track_id: ActiveValue::Set(track_id.0),
                position: ActiveValue::Set(position as i64),
                added_at: ActiveValue::Set(now_ts),
            };

            active_playlist_track
                .insert(&txn)
                .await
                .context("Failed to add track to playlist")?;

            playlist_tracks.push(models::PlaylistTrack {
                track_id: *track_id,
                position: position as u32,
                added_at: Utc::now(),
            });
        }

        txn.commit().await.context("Failed to commit transaction")?;

        let mut playlist = models::Playlist {
            id: playlist_id,
            metadata: models::Metadata {
                title: runtime_playlist.title,
                description: runtime_playlist.description,
                artwork: None,
            },
            creator: None,
            created_at: now,
            updated_at: now,
        };

        Ok(models::PlaylistWithTracks {
            playlist,
            tracks: playlist_tracks,
        })
    }

    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }
}
