use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

use crate::library::types::{PlaylistId, TrackId as LibTrackId};
use crate::library::LibraryService;
use crate::playlist::cursor::{
    ContainerContext, CursorResolver, CursorSource, DbTrackCursor, LocalPlaylistSource,
    ProviderCursor, StreamingSearchCursor,
};
use crate::streaming::types::ContentType;
use crate::streaming::StreamingService;

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
                // Lazy: only this playlist's own items; nested playlists stay containers.
                Ok(Box::new(LocalPlaylistSource::new(
                    self.library.clone(),
                    PlaylistId(id),
                )))
            }

            ContainerContext::ExternalPlaylist { system, id } => Ok(Box::new(ProviderCursor::new(
                self.streaming.clone(),
                system,
                id,
                ContentType::Playlist,
            ))),

            ContainerContext::ExternalAlbum { system, id } => Ok(Box::new(ProviderCursor::new(
                self.streaming.clone(),
                system,
                id,
                ContentType::Album,
            ))),

            ContainerContext::Radio { track_ids, .. } => {
                let ids: Vec<LibTrackId> = track_ids.into_iter().map(LibTrackId).collect();
                Ok(Box::new(DbTrackCursor::new(self.library.clone(), ids)))
            }

            ContainerContext::SearchQuery {
                system,
                query,
                content_type,
            } => {
                // Starts empty: the cursor fetches its first page on demand and maps
                // each result through `search_result_to_item()`.
                Ok(Box::new(StreamingSearchCursor::new(
                    self.streaming.clone(),
                    system,
                    query,
                    content_type,
                )))
            }
        }
    }
}
