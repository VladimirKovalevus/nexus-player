use anyhow::{Result, Context};
use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::Arc;

use crate::library::LibraryService;
use crate::library::models::PlaylistItemKind;
use crate::streaming::StreamingService;
use crate::library::types::{PlaylistId, TrackId as LibTrackId, TrackOrder};
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

    /// Recursively flatten playlist items into track IDs, handling nested playlists.
    fn flatten_playlist_tracks<'a>(
        library: &'a LibraryService,
        playlist_id: PlaylistId,
        visited: &'a mut HashSet<i64>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<LibTrackId>>> + Send + 'a>> {
        Box::pin(async move {
            if !visited.insert(playlist_id.0) {
                return Ok(vec![]);
            }

            let pwi = library
                .get_playlist_with_items(playlist_id)
                .await?
                .context("Playlist not found")?;

            let mut track_ids = Vec::new();
            for item in pwi.items {
                match item.item_kind {
                    PlaylistItemKind::Track => {
                        track_ids.push(LibTrackId(item.item_id));
                    }
                    PlaylistItemKind::Playlist => {
                        let nested = Self::flatten_playlist_tracks(
                            library,
                            PlaylistId(item.item_id),
                            visited,
                        )
                        .await?;
                        track_ids.extend(nested);
                    }
                }
            }
            Ok(track_ids)
        })
    }
}

#[async_trait]
impl CursorResolver for AppCursorResolver {
    async fn resolve(&self, context: ContainerContext) -> Result<Box<dyn CursorSource>> {
        match context {
            ContainerContext::LocalPlaylist(id) => {
                let playlist_id = PlaylistId(id);
                let mut visited = HashSet::new();
                let track_ids = Self::flatten_playlist_tracks(&self.library, playlist_id, &mut visited).await?;

                Ok(Box::new(DbTrackCursor::new(self.library.clone(), track_ids)))
            }

            ContainerContext::LocalAlbum(id) => {
                // Assuming we can find tracks by album ID or name. 
                // For now, let's say we have a method in library or use a placeholder.
                let tracks = self.library.get_all_tracks(TrackOrder::default()).await?; // Temporary: should be filtered by album
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

            ContainerContext::SearchQuery { system, query, content_type } => {
                let results = self.streaming.search(&system, Some(content_type), &query, 50, 0).await?;
                
                let items = results.into_iter().map(|res| {
                    match res {
                        SearchResult::Track(info) => CursorItem::Track(info.into()),
                        SearchResult::Playlist(info) => CursorItem::Container(ContainerContext::ExternalPlaylist { 
                            system: system.clone(), 
                            id: info.id 
                        }),
                        SearchResult::Album(info) => CursorItem::Container(ContainerContext::ExternalAlbum { 
                            system: system.clone(), 
                            id: info.id 
                        }),
                        _ => CursorItem::Container(ContainerContext::SearchQuery { system: system.clone(), query: query.clone(), content_type }) 
                    }
                }).collect();

                Ok(Box::new(StreamingSearchCursor::new(self.streaming.clone(), system, query, content_type, items)))
            }
        }
    }
}
