use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, TransactionTrait,
};
use std::collections::{HashSet, VecDeque};

use crate::library::entities_v2::{playlist, playlist_item};
use crate::library::models::{Playlist, PlaylistItem, PlaylistItemKind, PlaylistWithItems};
use crate::library::repository_v2::PlaylistRepositoryV2;
use crate::library::types::PlaylistId;

pub struct SeaOrmPlaylistRepository {
    db: DatabaseConnection,
}

impl SeaOrmPlaylistRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// BFS cycle detection: checks if adding child_playlist_id under playlist_id would create a cycle.
    async fn has_cycle<C: sea_orm::ConnectionTrait>(
        conn: &C,
        playlist_id: i64,
        child_playlist_id: i64,
    ) -> Result<bool> {
        if child_playlist_id == playlist_id {
            return Ok(true);
        }

        // BFS: collect all descendant playlist IDs starting from child_playlist_id
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(child_playlist_id);

        while let Some(current) = queue.pop_front() {
            if !visited.insert(current) {
                continue;
            }

            let children = playlist_item::Entity::find()
                .filter(playlist_item::Column::PlaylistId.eq(current))
                .filter(playlist_item::Column::ItemType.eq("playlist"))
                .all(conn)
                .await
                .context("Failed to query descendant playlists")?;

            for child in children {
                if child.item_id == playlist_id {
                    return Ok(true);
                }
                queue.push_back(child.item_id);
            }
        }

        Ok(false)
    }
}

#[async_trait]
impl PlaylistRepositoryV2 for SeaOrmPlaylistRepository {
    async fn create(&self, mut domain_playlist: Playlist) -> Result<PlaylistId> {
        let now = Utc::now().timestamp();
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
        let result = playlist::Entity::find_by_id(id.0)
            .find_also_related(crate::library::entities_v2::artwork::Entity)
            .one(&self.db)
            .await
            .context("Failed to fetch playlist with artwork")?;

        match result {
            Some((entity, artwork_entity)) => {
                let mut domain_playlist: Playlist = entity.try_into()?;
                if let Some(art_entity) = artwork_entity {
                    domain_playlist.metadata.artwork = Some(art_entity.try_into()?);
                }
                Ok(Some(domain_playlist))
            }
            None => Ok(None),
        }
    }

    async fn get_with_items(&self, id: PlaylistId) -> Result<Option<PlaylistWithItems>> {
        let result = playlist::Entity::find_by_id(id.0)
            .find_also_related(crate::library::entities_v2::artwork::Entity)
            .one(&self.db)
            .await
            .context("Failed to fetch playlist with artwork")?;

        let Some((playlist_entity, artwork_entity)) = result else {
            return Ok(None);
        };

        let mut domain_playlist: Playlist = playlist_entity.try_into()?;
        if let Some(art_entity) = artwork_entity {
            domain_playlist.metadata.artwork = Some(art_entity.try_into()?);
        }

        let item_entities = playlist_item::Entity::find()
            .filter(playlist_item::Column::PlaylistId.eq(id.0))
            .order_by_asc(playlist_item::Column::Position)
            .all(&self.db)
            .await
            .context("Failed to fetch playlist items")?;

        let items: Vec<PlaylistItem> = item_entities
            .into_iter()
            .map(|entity| entity.try_into())
            .collect::<Result<Vec<_>>>()?;

        Ok(Some(PlaylistWithItems {
            playlist: domain_playlist,
            items,
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
        let results = playlist::Entity::find()
            .find_also_related(crate::library::entities_v2::artwork::Entity)
            .order_by_desc(playlist::Column::CreatedAt)
            .all(&self.db)
            .await
            .context("Failed to list playlists with artwork")?;

        let mut domain_playlists = Vec::new();
        for (entity, artwork_entity) in results {
            let mut domain_playlist: Playlist = entity.try_into()?;
            if let Some(art_entity) = artwork_entity {
                domain_playlist.metadata.artwork = Some(art_entity.try_into()?);
            }
            domain_playlists.push(domain_playlist);
        }
        Ok(domain_playlists)
    }

    async fn add_item(
        &self,
        playlist_id: PlaylistId,
        item_kind: PlaylistItemKind,
        item_id: i64,
        position: u32,
    ) -> Result<()> {
        let item_type_str = match item_kind {
            PlaylistItemKind::Track => "track",
            PlaylistItemKind::Playlist => "playlist",
        };

        // Cycle detection for sub-playlists
        if item_kind == PlaylistItemKind::Playlist {
            if Self::has_cycle(&self.db, playlist_id.0, item_id).await? {
                bail!("Cannot add playlist: would create a cycle");
            }
        }

        let now = Utc::now().timestamp();

        let item_active = playlist_item::ActiveModel {
            id: ActiveValue::NotSet,
            playlist_id: ActiveValue::Set(playlist_id.0),
            item_type: ActiveValue::Set(item_type_str.to_string()),
            item_id: ActiveValue::Set(item_id),
            position: ActiveValue::Set(position as i64),
            added_at: ActiveValue::Set(now),
        };

        item_active.insert(&self.db).await?;

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

    async fn remove_item(
        &self,
        playlist_id: PlaylistId,
        item_kind: PlaylistItemKind,
        item_id: i64,
    ) -> Result<()> {
        let item_type_str = match item_kind {
            PlaylistItemKind::Track => "track",
            PlaylistItemKind::Playlist => "playlist",
        };

        playlist_item::Entity::delete_many()
            .filter(playlist_item::Column::PlaylistId.eq(playlist_id.0))
            .filter(playlist_item::Column::ItemType.eq(item_type_str))
            .filter(playlist_item::Column::ItemId.eq(item_id))
            .exec(&self.db)
            .await
            .context("Failed to remove item from playlist")?;

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

    async fn reorder_items(&self, playlist_id: PlaylistId, item_ids: Vec<i64>) -> Result<()> {
        let txn = self.db.begin().await?;

        for (idx, item_id) in item_ids.iter().enumerate() {
            let pos = idx as i64;

            let entity = playlist_item::Entity::find_by_id(*item_id)
                .filter(playlist_item::Column::PlaylistId.eq(playlist_id.0))
                .one(&txn)
                .await
                .context("Failed to find playlist item")?
                .context("Playlist item not found")?;

            let active = playlist_item::ActiveModel {
                id: ActiveValue::Unchanged(entity.id),
                playlist_id: ActiveValue::Unchanged(entity.playlist_id),
                item_type: ActiveValue::Unchanged(entity.item_type),
                item_id: ActiveValue::Unchanged(entity.item_id),
                position: ActiveValue::Set(pos),
                added_at: ActiveValue::Unchanged(entity.added_at),
            };

            active
                .update(&txn)
                .await
                .context("Failed to update item position")?;
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

    async fn is_track_in_any_playlist(
        &self,
        track_id: crate::library::types::TrackId,
    ) -> Result<bool> {
        let count = playlist_item::Entity::find()
            .filter(playlist_item::Column::ItemType.eq("track"))
            .filter(playlist_item::Column::ItemId.eq(track_id.0))
            .count(&self.db)
            .await
            .context("Failed to count track occurrences in playlists")?;

        Ok(count > 0)
    }
}
