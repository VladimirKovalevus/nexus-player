use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, QueryTrait, TransactionTrait, sea_query,
};

use crate::library::entities_v2::{artwork, playlist_item, track};
use crate::library::models::Track;
use crate::library::repository_v2::TrackRepositoryV2;
use crate::library::types::{SortDirection, TrackId, TrackOrder};

pub struct SeaOrmTrackRepository {
    db: DatabaseConnection,
}

impl SeaOrmTrackRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn get_next_liked_position(&self) -> Result<i64> {
        let max_position = track::Entity::find()
            .filter(track::Column::Liked.eq(1))
            .select_only()
            .column(track::Column::LikedPosition)
            .order_by_desc(track::Column::LikedPosition)
            .into_tuple::<(Option<i64>,)>()
            .one(&self.db)
            .await
            .context("Failed to find max liked_position")?
            .and_then(|t| t.0)
            .unwrap_or(-1);

        Ok(max_position + 1)
    }
}

#[async_trait]
impl TrackRepositoryV2 for SeaOrmTrackRepository {
    async fn insert(&self, mut domain_track: Track) -> Result<TrackId> {
        let now = Utc::now().timestamp();
        domain_track.updated_at =
            chrono::DateTime::from_timestamp(now, 0).context("Invalid timestamp")?;

        if domain_track.liked && domain_track.liked_position.is_none() {
            domain_track.liked_position = Some(self.get_next_liked_position().await?);
        }

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
            liked: ActiveValue::Set(entity_model.liked),
            liked_position: ActiveValue::Set(entity_model.liked_position),
            unavailable: ActiveValue::Set(entity_model.unavailable),
        };

        let result = active_model
            .insert(&self.db)
            .await
            .context("Failed to insert track")?;

        Ok(TrackId(result.id))
    }

    async fn get(&self, id: TrackId) -> Result<Option<Track>> {
        let result = track::Entity::find_by_id(id.0)
            .find_also_related(crate::library::entities_v2::artwork::Entity)
            .one(&self.db)
            .await
            .context("Failed to fetch track with artwork")?;

        match result {
            Some((entity, artwork_entity)) => {
                let mut domain_track: Track = entity.try_into()?;
                if let Some(art_entity) = artwork_entity {
                    domain_track.metadata.artwork = Some(art_entity.try_into()?);
                }
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
            liked: ActiveValue::Set(entity_model.liked),
            liked_position: ActiveValue::Set(entity_model.liked_position),
            unavailable: ActiveValue::Set(entity_model.unavailable),
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
        let subquery = sea_query::Query::select()
            .column(playlist_item::Column::ItemId)
            .from(playlist_item::Entity)
            .and_where(sea_query::Expr::col(playlist_item::Column::ItemType).eq("track"))
            .to_owned();

        let results = track::Entity::find()
            .filter(track::Column::Id.not_in_subquery(subquery))
            .find_also_related(crate::library::entities_v2::artwork::Entity)
            .order_by_desc(track::Column::AddedAt)
            .all(&self.db)
            .await
            .context("Failed to list all tracks with artwork")?;

        let mut domain_tracks = Vec::new();
        for (entity, artwork_entity) in results {
            let mut domain_track: Track = entity.try_into()?;
            if let Some(art_entity) = artwork_entity {
                domain_track.metadata.artwork = Some(art_entity.try_into()?);
            }
            domain_tracks.push(domain_track);
        }
        Ok(domain_tracks)
    }

    async fn list_all_flat(&self) -> Result<Vec<Track>> {
        let results = track::Entity::find()
            .find_also_related(crate::library::entities_v2::artwork::Entity)
            .order_by_desc(track::Column::AddedAt)
            .all(&self.db)
            .await
            .context("Failed to list all tracks flat")?;

        let mut domain_tracks = Vec::new();
        for (entity, artwork_entity) in results {
            let mut domain_track: Track = entity.try_into()?;
            if let Some(art_entity) = artwork_entity {
                domain_track.metadata.artwork = Some(art_entity.try_into()?);
            }
            domain_tracks.push(domain_track);
        }
        Ok(domain_tracks)
    }

    async fn search(&self, query: &str) -> Result<Vec<Track>> {
        use sea_orm::Condition;

        let results = track::Entity::find()
            .filter(
                Condition::any()
                    .add(track::Column::Title.contains(query))
                    .add(track::Column::Artist.contains(query))
                    .add(track::Column::Album.contains(query)),
            )
            .find_also_related(crate::library::entities_v2::artwork::Entity)
            .order_by_desc(track::Column::AddedAt)
            .all(&self.db)
            .await
            .context("Failed to search tracks with artwork")?;

        let mut domain_tracks = Vec::new();
        for (entity, artwork_entity) in results {
            let mut domain_track: Track = entity.try_into()?;
            if let Some(art_entity) = artwork_entity {
                domain_track.metadata.artwork = Some(art_entity.try_into()?);
            }
            domain_tracks.push(domain_track);
        }
        Ok(domain_tracks)
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

    async fn find_by_source(&self, source: &crate::library::types::TrackSource) -> Result<Option<Track>> {
        use serde::{Deserialize, Serialize};
        #[derive(Debug, Serialize, Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum TrackSourceData {
            Local { path: String },
            Http { url: String },
            External { system: String, id: String },
        }

        let (source_type, source_data) = match source {
            crate::library::types::TrackSource::Local(path) => {
                let data = TrackSourceData::Local {
                    path: path.display().to_string(),
                };
                ("local".to_string(), serde_json::to_string(&data).ok())
            }
            crate::library::types::TrackSource::Http(url) => {
                let data = TrackSourceData::Http { url: url.clone() };
                ("http".to_string(), serde_json::to_string(&data).ok())
            }
            crate::library::types::TrackSource::External { system, id } => {
                let data = TrackSourceData::External {
                    system: system.clone(),
                    id: id.clone(),
                };
                ("external".to_string(), serde_json::to_string(&data).ok())
            }
            _ => ("unknown".to_string(), None),
        };

        let result = track::Entity::find()
            .filter(track::Column::SourceType.eq(source_type))
            .filter(track::Column::SourceData.eq(source_data))
            .find_also_related(crate::library::entities_v2::artwork::Entity)
            .one(&self.db)
            .await
            .context("Failed to find track by source")?;

        match result {
            Some((entity, artwork_entity)) => {
                let mut domain_track: Track = entity.try_into()?;
                if let Some(art_entity) = artwork_entity {
                    domain_track.metadata.artwork = Some(art_entity.try_into()?);
                }
                Ok(Some(domain_track))
            }
            None => Ok(None),
        }
    }

    async fn list_liked(&self, order: TrackOrder) -> Result<Vec<Track>> {
        let mut query = track::Entity::find()
            .filter(track::Column::Liked.eq(1))
            .find_also_related(crate::library::entities_v2::artwork::Entity);

        let (col, direction) = match order {
            TrackOrder::Default(d) => (track::Column::LikedPosition, d),
            TrackOrder::Title(d) => (track::Column::Title, d),
            TrackOrder::Artist(d) => (track::Column::Artist, d),
            TrackOrder::AddedAt(d) => (track::Column::AddedAt, d),
        };

        query = match direction {
            SortDirection::Asc => query.order_by_asc(col),
            SortDirection::Desc => query.order_by_desc(col),
        };

        let results = query
            .all(&self.db)
            .await
            .context("Failed to list liked tracks with artwork and sorting")?;

        let mut domain_tracks = Vec::new();
        for (entity, artwork_entity) in results {
            let mut domain_track: Track = entity.try_into()?;
            if let Some(art_entity) = artwork_entity {
                domain_track.metadata.artwork = Some(art_entity.try_into()?);
            }
            domain_tracks.push(domain_track);
        }
        Ok(domain_tracks)
    }

    async fn list_cached(&self, order: TrackOrder) -> Result<Vec<Track>> {
        let mut query = track::Entity::find()
            .filter(track::Column::CacheType.eq("cached"))
            .find_also_related(artwork::Entity);

        let (col, direction) = match order {
            TrackOrder::Default(d) => (track::Column::AddedAt, d),
            TrackOrder::Title(d) => (track::Column::Title, d),
            TrackOrder::Artist(d) => (track::Column::Artist, d),
            TrackOrder::AddedAt(d) => (track::Column::AddedAt, d),
        };

        query = match direction {
            SortDirection::Asc => query.order_by_asc(col),
            SortDirection::Desc => query.order_by_desc(col),
        };

        let results = query
            .all(&self.db)
            .await
            .context("Failed to list cached tracks with artwork and sorting")?;

        let mut domain_tracks = Vec::new();
        for (entity, artwork_entity) in results {
            let mut domain_track: Track = entity.try_into()?;
            if let Some(art_entity) = artwork_entity {
                domain_track.metadata.artwork = Some(art_entity.try_into()?);
            }
            domain_tracks.push(domain_track);
        }
        Ok(domain_tracks)
    }

    async fn set_liked(&self, id: TrackId, liked: bool) -> Result<()> {
        if liked {
            let position = self.get_next_liked_position().await?;

            let active_model = track::ActiveModel {
                id: ActiveValue::Unchanged(id.0),
                liked: ActiveValue::Set(1),
                liked_position: ActiveValue::Set(Some(position)),
                ..Default::default()
            };
            active_model
                .update(&self.db)
                .await
                .context("Failed to set liked=true")?;
        } else {
            let active_model = track::ActiveModel {
                id: ActiveValue::Unchanged(id.0),
                liked: ActiveValue::Set(0),
                liked_position: ActiveValue::Set(None),
                ..Default::default()
            };
            active_model
                .update(&self.db)
                .await
                .context("Failed to set liked=false")?;
        }
        Ok(())
    }

    async fn set_unavailable(&self, id: TrackId, unavailable: bool) -> Result<()> {
        let active_model = track::ActiveModel {
            id: ActiveValue::Unchanged(id.0),
            unavailable: ActiveValue::Set(unavailable as i32),
            ..Default::default()
        };
        active_model
            .update(&self.db)
            .await
            .context("Failed to set unavailable")?;
        Ok(())
    }

    async fn reorder_liked(&self, track_ids: Vec<TrackId>) -> Result<()> {
        let txn = self.db.begin().await?;

        for (idx, track_id) in track_ids.iter().enumerate() {
            let active_model = track::ActiveModel {
                id: ActiveValue::Unchanged(track_id.0),
                liked_position: ActiveValue::Set(Some(idx as i64)),
                ..Default::default()
            };
            active_model
                .update(&txn)
                .await
                .context("Failed to update liked_position")?;
        }

        txn.commit().await?;
        Ok(())
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

    async fn find_liked_by_sources(
        &self,
        sources: &[crate::library::types::TrackSource],
    ) -> Result<std::collections::HashSet<crate::library::types::TrackSource>> {
        use sea_orm::Condition;
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Serialize, Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum TrackSourceData {
            Local { path: String },
            Http { url: String },
            External { system: String, id: String },
        }

        if sources.is_empty() {
            return Ok(std::collections::HashSet::new());
        }

        let mut condition = Condition::any();

        for source in sources {
            let (source_type, source_data) = match source {
                crate::library::types::TrackSource::Local(path) => {
                    let data = TrackSourceData::Local {
                        path: path.display().to_string(),
                    };
                    ("local".to_string(), serde_json::to_string(&data).ok())
                }
                crate::library::types::TrackSource::Http(url) => {
                    let data = TrackSourceData::Http { url: url.clone() };
                    ("http".to_string(), serde_json::to_string(&data).ok())
                }
                crate::library::types::TrackSource::External { system, id } => {
                    let data = TrackSourceData::External {
                        system: system.clone(),
                        id: id.clone(),
                    };
                    ("external".to_string(), serde_json::to_string(&data).ok())
                }
                _ => continue,
            };

            condition = condition.add(
                Condition::all()
                    .add(track::Column::SourceType.eq(source_type))
                    .add(track::Column::SourceData.eq(source_data)),
            );
        }

        let results = track::Entity::find()
            .filter(condition)
            .filter(track::Column::Liked.eq(1))
            .all(&self.db)
            .await
            .context("Failed to find liked tracks by sources")?;

        let liked_sources: std::collections::HashSet<_> = results
            .into_iter()
            .map(|entity| {
                let track_domain: Track = entity.try_into()?;
                Ok(track_domain.source)
            })
            .collect::<Result<std::collections::HashSet<_>>>()?;

        Ok(liked_sources)
    }

    async fn delete_orphan_unliked(&self) -> Result<u64> {
        let in_playlists = sea_query::Query::select()
            .column(playlist_item::Column::ItemId)
            .from(playlist_item::Entity)
            .and_where(sea_query::Expr::col(playlist_item::Column::ItemType).eq("track"))
            .to_owned();

        let result = track::Entity::delete_many()
            .filter(track::Column::Liked.eq(0))
            .filter(track::Column::Id.not_in_subquery(in_playlists))
            .exec(&self.db)
            .await
            .context("Failed to delete orphan unliked tracks")?;

        Ok(result.rows_affected)
    }

    async fn delete_orphan_artworks(&self) -> Result<u64> {
        use crate::library::entities_v2::playlist;

        let used_by_tracks = sea_query::Query::select()
            .column(track::Column::ArtworkId)
            .from(track::Entity)
            .and_where(sea_query::Expr::col(track::Column::ArtworkId).is_not_null())
            .to_owned();

        let used_by_playlists = sea_query::Query::select()
            .column(playlist::Column::ArtworkId)
            .from(playlist::Entity)
            .and_where(sea_query::Expr::col(playlist::Column::ArtworkId).is_not_null())
            .to_owned();

        let result = artwork::Entity::delete_many()
            .filter(artwork::Column::Id.not_in_subquery(used_by_tracks))
            .filter(artwork::Column::Id.not_in_subquery(used_by_playlists))
            .exec(&self.db)
            .await
            .context("Failed to delete orphan artworks")?;

        Ok(result.rows_affected)
    }
}
