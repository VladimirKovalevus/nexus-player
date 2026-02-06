use anyhow::{Result, Context};
use async_trait::async_trait;
use std::sync::Arc;

use crate::library::LibraryService;
use crate::streaming::StreamingService;
use crate::library::types::{PlaylistId, TrackId as LibTrackId};
use crate::playlist::cursor::{
    CursorSource, CursorResolver, ContainerContext, CursorItem, 
    DbTrackCursor, ProviderCursor, StreamingSearchCursor
};
use crate::streaming::types::{SearchResult, ContentType};

pub struct AppCursorResolver {
    library: Arc<LibraryService>,
    streaming: Arc<StreamingService>,
}

impl AppCursorResolver {
    pub fn new(library: Arc<LibraryService>, streaming: Arc<StreamingService>) -> Self {
        Self { library, streaming }
    }
}

#[async_trait]
impl CursorResolver for AppCursorResolver {
    async fn resolve(&self, context: ContainerContext) -> Result<Box<dyn CursorSource>> {
        match context {
            ContainerContext::LocalPlaylist(id) => {
                let playlist_id = PlaylistId(id);
                let playlist_with_tracks = self.library.get_playlist_with_tracks(playlist_id)
                    .await?
                    .context("Playlist not found")?;

                let track_ids: Vec<LibTrackId> = playlist_with_tracks.tracks
                    .into_iter()
                    .map(|pt| LibTrackId(pt.track_id.0))
                    .collect();
                
                Ok(Box::new(DbTrackCursor::new(self.library.clone(), track_ids)))
            }

            ContainerContext::LocalAlbum(id) => {
                // Assuming we can find tracks by album ID or name. 
                // For now, let's say we have a method in library or use a placeholder.
                let tracks = self.library.get_all_tracks().await?; // Temporary: should be filtered by album
                let track_ids: Vec<LibTrackId> = tracks.into_iter().map(|t| t.id).collect();
                
                Ok(Box::new(DbTrackCursor::new(self.library.clone(), track_ids)))
            }

            ContainerContext::ExternalPlaylist { system, id } => {
                Ok(Box::new(ProviderCursor::new(
                    self.streaming.clone(), 
                    system, 
                    id, 
                    ContentType::Playlist
                )))
            }

            ContainerContext::ExternalAlbum { system, id } => {
                Ok(Box::new(ProviderCursor::new(
                    self.streaming.clone(), 
                    system, 
                    id, 
                    ContentType::Album
                )))
            }

            ContainerContext::SearchQuery { system, query } => {
                let results = self.streaming.search(&system, Some(ContentType::Track), &query, 50).await?;
                
                let items = results.into_iter().map(|res| {
                    match res {
                        SearchResult::Track(info) => CursorItem::Track(info.into()),
                        SearchResult::Playlist(info) => CursorItem::Container(ContainerContext::ExternalPlaylist { 
                            system: system.clone(), 
                            id: info.url 
                        }),
                        SearchResult::Album(info) => CursorItem::Container(ContainerContext::ExternalAlbum { 
                            system: system.clone(), 
                            id: info.url 
                        }),
                        _ => CursorItem::Container(ContainerContext::SearchQuery { system: system.clone(), query: query.clone() }) 
                    }
                }).collect();

                Ok(Box::new(StreamingSearchCursor::new(self.streaming.clone(), system, query, items)))
            }
        }
    }
}
