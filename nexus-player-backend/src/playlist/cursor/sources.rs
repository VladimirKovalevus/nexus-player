use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

use super::{ContainerContext, CursorItem, CursorSource};
use crate::audio::types::Track;
use crate::library::types::TrackId as LibTrackId;
use crate::library::LibraryService;
use crate::streaming::types::{ContentType, SearchResult};
use crate::streaming::StreamingService;

/// A simple cursor that iterates over a pre-loaded list of tracks.
pub struct StaticTrackCursor {
    tracks: Vec<Track>,
    index: usize,
}

impl StaticTrackCursor {
    pub fn new(tracks: Vec<Track>) -> Self {
        Self { tracks, index: 0 }
    }
}

#[async_trait]
impl CursorSource for StaticTrackCursor {
    async fn next_batch(&mut self, limit: usize) -> Result<Vec<CursorItem>> {
        if self.index >= self.tracks.len() {
            return Ok(Vec::new());
        }

        let end = (self.index + limit).min(self.tracks.len());
        let batch = self.tracks[self.index..end]
            .iter()
            .cloned()
            .map(CursorItem::Track)
            .collect();

        self.index = end;
        Ok(batch)
    }

    fn has_more(&self) -> bool {
        self.index < self.tracks.len()
    }
}

/// A cursor that fetches tracks from the local database by their IDs.
pub struct DbTrackCursor {
    library: Arc<LibraryService>,
    track_ids: Vec<LibTrackId>,
    index: usize,
}

impl DbTrackCursor {
    pub fn new(library: Arc<LibraryService>, track_ids: Vec<LibTrackId>) -> Self {
        Self {
            library,
            track_ids,
            index: 0,
        }
    }
}

#[async_trait]
impl CursorSource for DbTrackCursor {
    async fn next_batch(&mut self, limit: usize) -> Result<Vec<CursorItem>> {
        if self.index >= self.track_ids.len() {
            return Ok(Vec::new());
        }

        // Keep pulling until we have `limit` tracks or run out of ids. Ids can dangle
        // (a playlist_item pointing at a deleted track), and the engine treats an empty
        // batch as end-of-source — so returning early on a miss would truncate the queue.
        let mut batch = Vec::new();
        while batch.len() < limit && self.index < self.track_ids.len() {
            let id = self.track_ids[self.index];
            self.index += 1;
            match self.library.get_track(id).await? {
                Some(lib_track) => batch.push(CursorItem::Track(lib_track.into())),
                None => {
                    tracing::debug!("DbTrackCursor: skipping missing track id {}", id.0);
                }
            }
        }

        Ok(batch)
    }

    fn has_more(&self) -> bool {
        self.index < self.track_ids.len()
    }
}

/// A cursor for external provider content (Playlists/Albums).
pub struct ProviderCursor {
    streaming: Arc<StreamingService>,
    system: String,
    id: String,
    content_type: ContentType,
    items: Option<Vec<CursorItem>>,
    index: usize,
}

impl ProviderCursor {
    pub fn new(
        streaming: Arc<StreamingService>,
        system: String,
        id: String,
        content_type: ContentType,
    ) -> Self {
        Self {
            streaming,
            system,
            id,
            content_type,
            items: None,
            index: 0,
        }
    }

    async fn ensure_loaded(&mut self) -> Result<()> {
        if self.items.is_some() {
            return Ok(());
        }

        let tracks = self
            .streaming
            .expand_collection(&self.system, self.content_type, &self.id)
            .await?;

        let items = tracks
            .into_iter()
            .map(|info| CursorItem::Track(info.into()))
            .collect();

        self.items = Some(items);
        Ok(())
    }
}

#[async_trait]
impl CursorSource for ProviderCursor {
    async fn next_batch(&mut self, limit: usize) -> Result<Vec<CursorItem>> {
        self.ensure_loaded().await?;

        let Some(items) = self.items.as_ref() else {
            return Ok(Vec::new());
        };
        if self.index >= items.len() {
            return Ok(Vec::new());
        }

        let end = (self.index + limit).min(items.len());
        let batch = items[self.index..end].iter().cloned().collect();

        self.index = end;
        Ok(batch)
    }

    fn has_more(&self) -> bool {
        match &self.items {
            Some(items) => self.index < items.len(),
            None => true, // Not loaded yet
        }
    }
}

/// A cursor that wraps streaming search results.
pub struct StreamingSearchCursor {
    streaming: Arc<StreamingService>,
    system: String,
    query: String,
    content_type: ContentType,
    items: Vec<CursorItem>,
    index: usize,
    offset: usize,
    limit: usize,
    has_more: bool,
}

impl StreamingSearchCursor {
    pub fn new(
        streaming: Arc<StreamingService>,
        system: String,
        query: String,
        content_type: ContentType,
        initial_items: Vec<CursorItem>,
    ) -> Self {
        let count = initial_items.len();
        Self {
            streaming,
            system,
            query,
            content_type,
            items: initial_items,
            index: 0,
            offset: count,
            limit: 50,
            has_more: count > 0,
        }
    }

    async fn fetch_next_page(&mut self) -> Result<()> {
        if !self.has_more {
            return Ok(());
        }

        tracing::info!(
            "Fetching next page for search: {} (offset: {})",
            self.query,
            self.offset
        );

        let results = self
            .streaming
            .search(
                &self.system,
                Some(self.content_type),
                &self.query,
                self.limit,
                self.offset,
            )
            .await?;

        if results.is_empty() {
            self.has_more = false;
            return Ok(());
        }

        let content_type = self.content_type;
        let new_items: Vec<CursorItem> = results
            .into_iter()
            .filter_map(|res| match res {
                SearchResult::Track(info) => Some(CursorItem::Track(info.into())),
                SearchResult::Playlist(info) => {
                    Some(CursorItem::Container(ContainerContext::ExternalPlaylist {
                        system: self.system.clone(),
                        id: info.id,
                    }))
                }
                SearchResult::Album(info) => {
                    Some(CursorItem::Container(ContainerContext::ExternalAlbum {
                        system: self.system.clone(),
                        id: info.id,
                    }))
                }
                _ => Some(CursorItem::Container(ContainerContext::SearchQuery {
                    system: self.system.clone(),
                    query: self.query.clone(),
                    content_type,
                })),
            })
            .collect();

        self.offset += new_items.len();
        self.items.extend(new_items);

        Ok(())
    }
}

#[async_trait]
impl CursorSource for StreamingSearchCursor {
    async fn next_batch(&mut self, limit: usize) -> Result<Vec<CursorItem>> {
        if self.index >= self.items.len() && self.has_more {
            self.fetch_next_page().await?;
        }

        if self.index >= self.items.len() {
            return Ok(Vec::new());
        }

        let end = (self.index + limit).min(self.items.len());
        let batch = self.items[self.index..end].iter().cloned().collect();

        self.index = end;
        Ok(batch)
    }

    fn has_more(&self) -> bool {
        self.has_more || self.index < self.items.len()
    }
}
