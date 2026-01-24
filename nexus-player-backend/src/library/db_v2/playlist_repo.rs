use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, TransactionTrait,
};

use crate::library::entities_v2::{playlist, playlist_track};
use crate::library::models::{Playlist, PlaylistTrack, PlaylistWithTracks};
use crate::library::repository_v2::PlaylistRepositoryV2;
use crate::library::types::{PlaylistId, TrackId};

pub struct SeaOrmPlaylistRepository {
    db: DatabaseConnection,
}

impl SeaOrmPlaylistRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl PlaylistRepositoryV2 for SeaOrmPlaylistRepository {
    async fn create(&self, mut domain_playlist: Playlist) -> Result<PlaylistId> {
        let now = Utc::now().timestamp();
        domain_playlist.created_at =
            chrono::DateTime::from_timestamp(now, 0).context("Invalid timestamp")?;
        domain_playlist.updated_at =
            chrono::DateTime::from_timestamp(now, 0).context("Invalid timestamp")?;

        let entity_model: playlist::Model = domain_playlist.into();

        let active_model = playlist::ActiveModel {
            id: ActiveValue::NotSet,
            title: ActiveValue::Set(entity_model.title),
            description: ActiveValue::Set(entity_model.description),
            creator: ActiveValue::Set(entity_model.creator),
            artwork_id: ActiveValue::Set(entity_model.artwork_id),
            created_at: ActiveValue::Set(entity_model.created_at),
            updated_at: ActiveValue::Set(entity_model.updated_at),
        };

        let result = active_model
            .insert(&self.db)
            .await
            .context("Failed to create playlist")?;

        Ok(PlaylistId(result.id))
    }

    async fn get(&self, id: PlaylistId) -> Result<Option<Playlist>> {
        let entity = playlist::Entity::find_by_id(id.0)
            .one(&self.db)
            .await
            .context("Failed to fetch playlist")?;

        match entity {
            Some(entity) => {
                let domain_playlist = entity.try_into()?;
                Ok(Some(domain_playlist))
            }
            None => Ok(None),
        }
    }

    async fn get_with_tracks(&self, id: PlaylistId) -> Result<Option<PlaylistWithTracks>> {
        let playlist_entity = playlist::Entity::find_by_id(id.0)
            .one(&self.db)
            .await
            .context("Failed to fetch playlist")?;

        let Some(playlist_entity) = playlist_entity else {
            return Ok(None);
        };

        let domain_playlist: Playlist = playlist_entity.try_into()?;

        let track_entities = playlist_track::Entity::find()
            .filter(playlist_track::Column::PlaylistId.eq(id.0))
            .order_by_asc(playlist_track::Column::Position)
            .all(&self.db)
            .await
            .context("Failed to fetch playlist tracks")?;

        let tracks: Vec<PlaylistTrack> = track_entities
            .into_iter()
            .map(|entity| entity.try_into())
            .collect::<Result<Vec<_>>>()?;

        Ok(Some(PlaylistWithTracks {
            playlist: domain_playlist,
            tracks,
        }))
    }

    async fn find_by_title(&self, title: &str) -> Result<Option<Playlist>> {
        let entity = playlist::Entity::find()
            .filter(playlist::Column::Title.eq(title))
            .one(&self.db)
            .await
            .context("Failed to fetch playlist by title")?;

        match entity {
            Some(entity) => {
                let domain_playlist = entity.try_into()?;
                Ok(Some(domain_playlist))
            }
            None => Ok(None),
        }
    }

    async fn update(&self, domain_playlist: Playlist) -> Result<()> {
        let mut entity_model: playlist::Model = domain_playlist.into();
        entity_model.updated_at = Utc::now().timestamp();

        let active_model = playlist::ActiveModel {
            id: ActiveValue::Unchanged(entity_model.id),
            title: ActiveValue::Set(entity_model.title),
            description: ActiveValue::Set(entity_model.description),
            creator: ActiveValue::Set(entity_model.creator),
            artwork_id: ActiveValue::Set(entity_model.artwork_id),
            created_at: ActiveValue::Set(entity_model.created_at),
            updated_at: ActiveValue::Set(entity_model.updated_at),
        };

        active_model
            .update(&self.db)
            .await
            .context("Failed to update playlist")?;

        Ok(())
    }

    async fn delete(&self, id: PlaylistId) -> Result<()> {
        playlist::Entity::delete_by_id(id.0)
            .exec(&self.db)
            .await
            .context("Failed to delete playlist")?;

        Ok(())
    }

    async fn list_all(&self) -> Result<Vec<Playlist>> {
        let entities = playlist::Entity::find()
            .order_by_desc(playlist::Column::CreatedAt)
            .all(&self.db)
            .await
            .context("Failed to list playlists")?;

        entities
            .into_iter()
            .map(|entity| entity.try_into())
            .collect::<Result<Vec<_>>>()
    }

    async fn add_track(
        &self,
        playlist_id: PlaylistId,
        track_id: TrackId,
        position: u32,
    ) -> Result<()> {
        let now = Utc::now().timestamp();

        let track_active = playlist_track::ActiveModel {
            playlist_id: ActiveValue::Set(playlist_id.0),
            track_id: ActiveValue::Set(track_id.0),
            position: ActiveValue::Set(position as i64),
            added_at: ActiveValue::Set(now),
        };

        track_active
            .insert(&self.db)
            .await
            .context("Failed to add track to playlist")?;

        let playlist_active = playlist::ActiveModel {
            id: ActiveValue::Unchanged(playlist_id.0),
            updated_at: ActiveValue::Set(now),
            ..Default::default()
        };

        playlist_active
            .update(&self.db)
            .await
            .context("Failed to update playlist timestamp")?;

        Ok(())
    }

    async fn remove_track(&self, playlist_id: PlaylistId, track_id: TrackId) -> Result<()> {
        playlist_track::Entity::delete_many()
            .filter(playlist_track::Column::PlaylistId.eq(playlist_id.0))
            .filter(playlist_track::Column::TrackId.eq(track_id.0))
            .exec(&self.db)
            .await
            .context("Failed to remove track from playlist")?;

        let now = Utc::now().timestamp();
        let playlist_active = playlist::ActiveModel {
            id: ActiveValue::Unchanged(playlist_id.0),
            updated_at: ActiveValue::Set(now),
            ..Default::default()
        };

        playlist_active
            .update(&self.db)
            .await
            .context("Failed to update playlist timestamp")?;

        Ok(())
    }

    async fn reorder_tracks(&self, playlist_id: PlaylistId, track_ids: Vec<TrackId>) -> Result<()> {
        let txn = self.db.begin().await?;

        for (idx, track_id) in track_ids.iter().enumerate() {
            let pos = idx as i64;

            let entity = playlist_track::Entity::find()
                .filter(playlist_track::Column::PlaylistId.eq(playlist_id.0))
                .filter(playlist_track::Column::TrackId.eq(track_id.0))
                .one(&txn)
                .await
                .context("Failed to find playlist track")?
                .context("Playlist track not found")?;

            let active = playlist_track::ActiveModel {
                playlist_id: ActiveValue::Unchanged(entity.playlist_id),
                track_id: ActiveValue::Unchanged(entity.track_id),
                position: ActiveValue::Set(pos),
                added_at: ActiveValue::Unchanged(entity.added_at),
            };

            active
                .update(&txn)
                .await
                .context("Failed to update track position")?;
        }

        let now = Utc::now().timestamp();
        let playlist_active = playlist::ActiveModel {
            id: ActiveValue::Unchanged(playlist_id.0),
            updated_at: ActiveValue::Set(now),
            ..Default::default()
        };

        playlist_active
            .update(&txn)
            .await
            .context("Failed to update playlist timestamp")?;

        txn.commit().await?;

        Ok(())
    }
}
