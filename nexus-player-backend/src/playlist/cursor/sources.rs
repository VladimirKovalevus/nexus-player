use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

use crate::audio::types::Track;
use crate::library::LibraryService;
use crate::library::types::TrackId as LibTrackId;
use crate::streaming::StreamingService;
use crate::streaming::types::{ContentType, SearchResult};
use super::{CursorSource, CursorItem, ContainerContext};

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
        Self { library, track_ids, index: 0 }
    }
}

#[async_trait]
impl CursorSource for DbTrackCursor {
    async fn next_batch(&mut self, limit: usize) -> Result<Vec<CursorItem>> {
        if self.index >= self.track_ids.len() {
            return Ok(Vec::new());
        }

        let end = (self.index + limit).min(self.track_ids.len());
        let ids_to_fetch = &self.track_ids[self.index..end];
        
        let mut batch = Vec::new();
        for id in ids_to_fetch {
            if let Some(lib_track) = self.library.get_track(*id).await? {
                // Assuming From implementation exists or mapping manually
                batch.push(CursorItem::Track(lib_track.into()));
            }
        }

        self.index = end;
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
    pub fn new(streaming: Arc<StreamingService>, system: String, id: String, content_type: ContentType) -> Self {
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

        // Use search or a specialized resolve method if available. 
        // For now, we simulate resolving a playlist/album.
        let results = self.streaming.search(&self.system, Some(self.content_type), &self.id, 100).await?;
        
        let items = results.into_iter().filter_map(|res| match res {
            SearchResult::Track(info) => Some(CursorItem::Track(info.into())),
            SearchResult::Playlist(info) => Some(CursorItem::Container(ContainerContext::ExternalPlaylist { 
                system: self.system.clone(), 
                id: info.url 
            })),
            SearchResult::Album(info) => Some(CursorItem::Container(ContainerContext::ExternalAlbum { 
                system: self.system.clone(), 
                id: info.url 
            })),
            _ => None,
        }).collect();

        self.items = Some(items);
        Ok(())
    }
}

#[async_trait]
impl CursorSource for ProviderCursor {
    async fn next_batch(&mut self, limit: usize) -> Result<Vec<CursorItem>> {
        self.ensure_loaded().await?;
        
        let items = self.items.as_ref().unwrap();
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
    items: Vec<CursorItem>,
    index: usize,
}

impl StreamingSearchCursor {
    pub fn new(streaming: Arc<StreamingService>, system: String, query: String, initial_items: Vec<CursorItem>) -> Self {
        Self {
            streaming,
            system,
            query,
            items: initial_items,
            index: 0,
        }
    }
}

#[async_trait]
impl CursorSource for StreamingSearchCursor {
    async fn next_batch(&mut self, limit: usize) -> Result<Vec<CursorItem>> {
        if self.index >= self.items.len() {
            // In the future: implement pagination here using self.streaming.search
            return Ok(Vec::new());
        }

        let end = (self.index + limit).min(self.items.len());
        let batch = self.items[self.index..end].iter().cloned().collect();
        
        self.index = end;
        Ok(batch)
    }

    fn has_more(&self) -> bool {
        // For now simplified, later check pagination
        self.index < self.items.len()
    }
}
