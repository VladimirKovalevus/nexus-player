use anyhow::{Context, Result};
use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ActiveValue, DatabaseConnection, EntityTrait, QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};

use crate::library::entities_v2::play_history;
use crate::library::models::PlayHistoryEntry;
use crate::library::repository_v2::PlayHistoryRepository;
use crate::library::types::TrackSource;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum TrackSourceData {
    Local { path: String },
    Http { url: String },
    External { system: String, id: String },
}

pub struct SeaOrmHistoryRepository {
    db: DatabaseConnection,
}

impl SeaOrmHistoryRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl PlayHistoryRepository for SeaOrmHistoryRepository {
    async fn add(&self, entry: PlayHistoryEntry) -> Result<()> {
        let (source_type, source_data) = match &entry.source {
            TrackSource::Local(path) => {
                let data = TrackSourceData::Local {
                    path: path.display().to_string(),
                };
                ("local".to_string(), serde_json::to_string(&data)?)
            }
            TrackSource::Http(url) => {
                let data = TrackSourceData::Http { url: url.clone() };
                ("http".to_string(), serde_json::to_string(&data)?)
            }
            TrackSource::External { system, id } => {
                let data = TrackSourceData::External {
                    system: system.clone(),
                    id: id.clone(),
                };
                ("external".to_string(), serde_json::to_string(&data)?)
            }
            TrackSource::Cached { original_url, .. } => {
                let data = TrackSourceData::Http {
                    url: original_url.clone(),
                };
                ("cached".to_string(), serde_json::to_string(&data)?)
            }
        };

        let active_model = play_history::ActiveModel {
            id: ActiveValue::NotSet,
            title: ActiveValue::Set(entry.title),
            artist: ActiveValue::Set(entry.artist),
            source_type: ActiveValue::Set(source_type),
            source_data: ActiveValue::Set(source_data),
            artwork_url: ActiveValue::Set(entry.artwork_url),
            played_at: ActiveValue::Set(entry.played_at),
        };

        active_model
            .insert(&self.db)
            .await
            .context("Failed to insert play history entry")?;

        Ok(())
    }

    async fn list_recent(&self, limit: usize, offset: usize) -> Result<Vec<PlayHistoryEntry>> {
        let entities = play_history::Entity::find()
            .order_by_desc(play_history::Column::PlayedAt)
            .limit(limit as u64)
            .offset(offset as u64)
            .all(&self.db)
            .await
            .context("Failed to list play history")?;

        entities
            .into_iter()
            .map(|e| e.try_into())
            .collect::<Result<Vec<_>>>()
    }

    async fn clear(&self) -> Result<()> {
        play_history::Entity::delete_many()
            .exec(&self.db)
            .await
            .context("Failed to clear play history")?;
        Ok(())
    }
}
