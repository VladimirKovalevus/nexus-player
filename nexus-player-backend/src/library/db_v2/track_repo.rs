use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder,
};

use crate::library::entities_v2::track;
use crate::library::models::Track;
use crate::library::repository_v2::TrackRepositoryV2;
use crate::library::types::TrackId;

pub struct SeaOrmTrackRepository {
    db: DatabaseConnection,
}

impl SeaOrmTrackRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TrackRepositoryV2 for SeaOrmTrackRepository {
    async fn insert(&self, mut domain_track: Track) -> Result<TrackId> {
        let now = Utc::now().timestamp();
        domain_track.added_at =
            chrono::DateTime::from_timestamp(now, 0).context("Invalid timestamp")?;
        domain_track.updated_at =
            chrono::DateTime::from_timestamp(now, 0).context("Invalid timestamp")?;

        let entity_model: track::Model = domain_track.into();

        let active_model = track::ActiveModel {
            id: ActiveValue::NotSet,
            title: ActiveValue::Set(entity_model.title),
            artist: ActiveValue::Set(entity_model.artist),
            description: ActiveValue::Set(entity_model.description),
            artwork_id: ActiveValue::Set(entity_model.artwork_id),
            album: ActiveValue::Set(entity_model.album),
            album_artist: ActiveValue::Set(entity_model.album_artist),
            year: ActiveValue::Set(entity_model.year),
            genre: ActiveValue::Set(entity_model.genre),
            track_number: ActiveValue::Set(entity_model.track_number),
            source_type: ActiveValue::Set(entity_model.source_type),
            source_data: ActiveValue::Set(entity_model.source_data),
            cache_type: ActiveValue::Set(entity_model.cache_type),
            cache_data: ActiveValue::Set(entity_model.cache_data),
            file_format: ActiveValue::Set(entity_model.file_format),
            duration_secs: ActiveValue::Set(entity_model.duration_secs),
            sample_rate: ActiveValue::Set(entity_model.sample_rate),
            channels: ActiveValue::Set(entity_model.channels),
            bitrate: ActiveValue::Set(entity_model.bitrate),
            play_count: ActiveValue::Set(entity_model.play_count),
            last_played: ActiveValue::Set(entity_model.last_played),
            added_at: ActiveValue::Set(entity_model.added_at),
            updated_at: ActiveValue::Set(entity_model.updated_at),
        };

        let result = active_model
            .insert(&self.db)
            .await
            .context("Failed to insert track")?;

        Ok(TrackId(result.id))
    }

    async fn get(&self, id: TrackId) -> Result<Option<Track>> {
        let entity = track::Entity::find_by_id(id.0)
            .one(&self.db)
            .await
            .context("Failed to fetch track")?;

        match entity {
            Some(entity) => {
                let domain_track = entity.try_into()?;
                Ok(Some(domain_track))
            }
            None => Ok(None),
        }
    }

    async fn update(&self, domain_track: Track) -> Result<()> {
        let mut entity_model: track::Model = domain_track.into();
        entity_model.updated_at = Utc::now().timestamp();

        let active_model = track::ActiveModel {
            id: ActiveValue::Unchanged(entity_model.id),
            title: ActiveValue::Set(entity_model.title),
            artist: ActiveValue::Set(entity_model.artist),
            description: ActiveValue::Set(entity_model.description),
            artwork_id: ActiveValue::Set(entity_model.artwork_id),
            album: ActiveValue::Set(entity_model.album),
            album_artist: ActiveValue::Set(entity_model.album_artist),
            year: ActiveValue::Set(entity_model.year),
            genre: ActiveValue::Set(entity_model.genre),
            track_number: ActiveValue::Set(entity_model.track_number),
            source_type: ActiveValue::Set(entity_model.source_type),
            source_data: ActiveValue::Set(entity_model.source_data),
            cache_type: ActiveValue::Set(entity_model.cache_type),
            cache_data: ActiveValue::Set(entity_model.cache_data),
            file_format: ActiveValue::Set(entity_model.file_format),
            duration_secs: ActiveValue::Set(entity_model.duration_secs),
            sample_rate: ActiveValue::Set(entity_model.sample_rate),
            channels: ActiveValue::Set(entity_model.channels),
            bitrate: ActiveValue::Set(entity_model.bitrate),
            play_count: ActiveValue::Set(entity_model.play_count),
            last_played: ActiveValue::Set(entity_model.last_played),
            added_at: ActiveValue::Set(entity_model.added_at),
            updated_at: ActiveValue::Set(entity_model.updated_at),
        };

        active_model
            .update(&self.db)
            .await
            .context("Failed to update track")?;

        Ok(())
    }

    async fn delete(&self, id: TrackId) -> Result<()> {
        track::Entity::delete_by_id(id.0)
            .exec(&self.db)
            .await
            .context("Failed to delete track")?;

        Ok(())
    }

    async fn list_all(&self) -> Result<Vec<Track>> {
        let entities = track::Entity::find()
            .order_by_desc(track::Column::AddedAt)
            .all(&self.db)
            .await
            .context("Failed to list all tracks")?;

        entities
            .into_iter()
            .map(|entity| entity.try_into())
            .collect::<Result<Vec<_>>>()
    }

    async fn search(&self, query: &str) -> Result<Vec<Track>> {
        use sea_orm::Condition;

        let entities = track::Entity::find()
            .filter(
                Condition::any()
                    .add(track::Column::Title.contains(query))
                    .add(track::Column::Artist.contains(query))
                    .add(track::Column::Album.contains(query)),
            )
            .order_by_desc(track::Column::AddedAt)
            .all(&self.db)
            .await
            .context("Failed to search tracks")?;

        entities
            .into_iter()
            .map(|entity| entity.try_into())
            .collect::<Result<Vec<_>>>()
    }

    async fn find_by_artist(&self, artist: &str) -> Result<Vec<Track>> {
        let entities = track::Entity::find()
            .filter(track::Column::Artist.eq(artist))
            .order_by_asc(track::Column::Year)
            .order_by_asc(track::Column::TrackNumber)
            .all(&self.db)
            .await
            .context("Failed to find tracks by artist")?;

        entities
            .into_iter()
            .map(|entity| entity.try_into())
            .collect::<Result<Vec<_>>>()
    }

    async fn find_by_album(&self, album: &str) -> Result<Vec<Track>> {
        let entities = track::Entity::find()
            .filter(track::Column::Album.eq(album))
            .order_by_asc(track::Column::TrackNumber)
            .all(&self.db)
            .await
            .context("Failed to find tracks by album")?;

        entities
            .into_iter()
            .map(|entity| entity.try_into())
            .collect::<Result<Vec<_>>>()
    }

    async fn increment_play_count(&self, id: TrackId) -> Result<()> {
        let entity = track::Entity::find_by_id(id.0)
            .one(&self.db)
            .await
            .context("Failed to fetch track for increment")?
            .context("Track not found")?;

        let now = Utc::now().timestamp();

        let active_model = track::ActiveModel {
            id: ActiveValue::Unchanged(entity.id),
            play_count: ActiveValue::Set(entity.play_count + 1),
            last_played: ActiveValue::Set(Some(now)),
            ..Default::default()
        };

        active_model
            .update(&self.db)
            .await
            .context("Failed to increment play count")?;

        Ok(())
    }
}
