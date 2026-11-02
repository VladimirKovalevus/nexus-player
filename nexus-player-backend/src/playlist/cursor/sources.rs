use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

use super::{ContainerContext, ContainerKind, ContainerMeta, CursorItem, CursorSource, RadioBlock};
use crate::library::models::PlaylistItemKind;
use crate::library::types::{PlaylistId, TrackId as LibTrackId};
use crate::library::LibraryService;
use crate::streaming::error::StreamResult;
use crate::streaming::types::{ContentType, SearchResult};
use crate::streaming::StreamingService;

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

/// A cursor over one local playlist's own items. Tracks are yielded as tracks and nested
/// playlists as containers; a child's items are fetched only when the cursor enters it.
/// Each yielded child is described (title, artwork, item count) so a queue can show it
/// as one row.
pub struct LocalPlaylistSource {
    library: Arc<LibraryService>,
    playlist_id: PlaylistId,
    items: Option<Vec<(PlaylistItemKind, i64)>>,
    index: usize,
    metas: Vec<ContainerMeta>,
}

impl LocalPlaylistSource {
    pub fn new(library: Arc<LibraryService>, playlist_id: PlaylistId) -> Self {
        Self {
            library,
            playlist_id,
            items: None,
            index: 0,
            metas: Vec::new(),
        }
    }
}

/// Describes local playlist `id` for a container row; `None` when it no longer exists.
async fn describe_local_playlist(
    library: &LibraryService,
    id: i64,
) -> Result<Option<ContainerMeta>> {
    let Some(pwi) = library.get_playlist_with_items(PlaylistId(id)).await? else {
        return Ok(None);
    };
    let playlist = pwi.playlist;
    let thumbnail = playlist
        .metadata
        .artwork
        .as_ref()
        .and_then(|art| match &art.source {
            crate::library::types::ArtworkSource::Url { url } => Some(url.clone()),
            _ => None,
        });
    Ok(Some(ContainerMeta {
        kind: ContainerKind::Playlist,
        id: id.to_string(),
        system: "local".to_string(),
        title: playlist.metadata.title,
        // A playlist saved from an album keeps the album's artist in its description.
        artist: playlist
            .metadata
            .description
            .or(playlist.creator)
            .unwrap_or_default(),
        track_count: pwi.items.len(),
        thumbnail,
    }))
}

#[async_trait]
impl CursorSource for LocalPlaylistSource {
    async fn next_batch(&mut self, limit: usize) -> Result<Vec<CursorItem>> {
        if self.items.is_none() {
            let pwi = self
                .library
                .get_playlist_with_items(self.playlist_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Playlist not found"))?;
            self.items = Some(
                pwi.items
                    .into_iter()
                    .map(|i| (i.item_kind, i.item_id))
                    .collect(),
            );
        }
        let items = self.items.as_deref().unwrap_or_default();

        let mut batch = Vec::new();
        while batch.len() < limit && self.index < items.len() {
            let (kind, id) = items[self.index];
            self.index += 1;
            match kind {
                PlaylistItemKind::Playlist => {
                    if let Some(meta) = describe_local_playlist(&self.library, id).await? {
                        self.metas.push(meta);
                    }
                    batch.push(CursorItem::Container(ContainerContext::LocalPlaylist(id)));
                }
                PlaylistItemKind::Track => match self.library.get_track(LibTrackId(id)).await? {
                    Some(t) => batch.push(CursorItem::Track(t.into())),
                    None => tracing::debug!("LocalPlaylistSource: skipping missing track {id}"),
                },
            }
        }
        Ok(batch)
    }

    fn has_more(&self) -> bool {
        match &self.items {
            Some(items) => self.index < items.len(),
            None => true,
        }
    }

    fn container_meta(&self, context: &ContainerContext) -> Option<ContainerMeta> {
        let ContainerContext::LocalPlaylist(id) = context else {
            return None;
        };
        let id = id.to_string();
        self.metas.iter().find(|m| m.id == id).cloned()
    }
}

/// A cursor over a frozen radio order. Loose tracks are yielded as tracks, playlist
/// blocks as `RadioPlaylist` containers described like local playlists.
pub struct RadioSource {
    library: Arc<LibraryService>,
    blocks: Vec<RadioBlock>,
    index: usize,
    metas: Vec<ContainerMeta>,
}

impl RadioSource {
    pub fn new(library: Arc<LibraryService>, blocks: Vec<RadioBlock>) -> Self {
        Self {
            library,
            blocks,
            index: 0,
            metas: Vec::new(),
        }
    }
}

#[async_trait]
impl CursorSource for RadioSource {
    async fn next_batch(&mut self, limit: usize) -> Result<Vec<CursorItem>> {
        let mut batch = Vec::new();
        while batch.len() < limit && self.index < self.blocks.len() {
            let block = self.blocks[self.index].clone();
            self.index += 1;
            match block.playlist_id {
                Some(id) => {
                    if let Some(mut meta) = describe_local_playlist(&self.library, id).await? {
                        meta.track_count = block.track_ids.len();
                        self.metas.push(meta);
                    }
                    batch.push(CursorItem::Container(ContainerContext::RadioPlaylist {
                        id,
                        track_ids: block.track_ids,
                    }));
                }
                None => {
                    for id in block.track_ids {
                        match self.library.get_track(LibTrackId(id)).await? {
                            Some(t) => batch.push(CursorItem::Track(t.into())),
                            None => tracing::debug!("RadioSource: skipping missing track {id}"),
                        }
                    }
                }
            }
        }
        Ok(batch)
    }

    fn has_more(&self) -> bool {
        self.index < self.blocks.len()
    }

    fn container_meta(&self, context: &ContainerContext) -> Option<ContainerMeta> {
        let ContainerContext::RadioPlaylist { id, .. } = context else {
            return None;
        };
        let id = id.to_string();
        self.metas.iter().find(|m| m.id == id).cloned()
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
        let batch = items[self.index..end].to_vec();

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

/// Maps one streaming search result to a cursor item.
/// Channels are skipped: they must never start a recursive search.
pub fn search_result_to_item(system: &str, result: &SearchResult) -> Option<CursorItem> {
    match result {
        SearchResult::Track(info) => Some(CursorItem::Track(info.clone().into())),
        SearchResult::Playlist(info) => {
            Some(CursorItem::Container(ContainerContext::ExternalPlaylist {
                system: system.to_string(),
                id: info.id.clone(),
            }))
        }
        SearchResult::Album(info) => Some(CursorItem::Container(ContainerContext::ExternalAlbum {
            system: system.to_string(),
            id: info.id.clone(),
        })),
        SearchResult::Channel(_) => {
            tracing::debug!("Skipping channel search result");
            None
        }
    }
}

/// Describes the album or playlist behind a search result, for showing it as a queue row.
fn container_meta_of(system: &str, result: &SearchResult) -> Option<ContainerMeta> {
    match result {
        SearchResult::Album(info) => Some(ContainerMeta {
            kind: ContainerKind::Album,
            id: info.id.clone(),
            system: system.to_string(),
            title: info.title.clone(),
            artist: info.artist.clone(),
            track_count: info.track_count,
            thumbnail: info.thumbnail.clone(),
        }),
        SearchResult::Playlist(info) => Some(ContainerMeta {
            kind: ContainerKind::Playlist,
            id: info.id.clone(),
            system: system.to_string(),
            title: info.title.clone(),
            artist: info.creator.clone(),
            track_count: info.track_count,
            thumbnail: info.thumbnail.clone(),
        }),
        SearchResult::Track(_) | SearchResult::Channel(_) => None,
    }
}

/// Fetches one page of search results.
#[async_trait]
pub trait SearchPageSource: Send + Sync {
    async fn search(
        &self,
        system: &str,
        content_type: Option<ContentType>,
        query: &str,
        limit: usize,
        offset: usize,
    ) -> StreamResult<Vec<SearchResult>>;
}

#[async_trait]
impl SearchPageSource for StreamingService {
    async fn search(
        &self,
        system: &str,
        content_type: Option<ContentType>,
        query: &str,
        limit: usize,
        offset: usize,
    ) -> StreamResult<Vec<SearchResult>> {
        StreamingService::search(self, system, content_type, query, limit, offset).await
    }
}

/// A cursor that wraps streaming search results.
pub struct StreamingSearchCursor {
    source: Arc<dyn SearchPageSource>,
    system: String,
    query: String,
    content_type: Option<ContentType>,
    items: Vec<CursorItem>,
    metas: Vec<ContainerMeta>,
    index: usize,
    /// Raw results requested so far (before channels are dropped).
    offset: usize,
    limit: usize,
    has_more: bool,
    /// Set by `starting_at`: the first result requested is the chosen one and must be playable.
    check_first: bool,
}

impl StreamingSearchCursor {
    pub fn new(
        streaming: Arc<StreamingService>,
        system: String,
        query: String,
        content_type: impl Into<Option<ContentType>>,
    ) -> Self {
        Self::with_source(streaming, system, query, content_type)
    }

    pub fn with_source(
        source: Arc<dyn SearchPageSource>,
        system: String,
        query: String,
        content_type: impl Into<Option<ContentType>>,
    ) -> Self {
        Self {
            source,
            system,
            query,
            content_type: content_type.into(),
            items: Vec::new(),
            metas: Vec::new(),
            index: 0,
            offset: 0,
            limit: 50,
            has_more: true,
            check_first: false,
        }
    }

    /// Positions the cursor at raw result `position`: the first request starts at that
    /// offset itself, so earlier pages are never fetched. A channel there is an error.
    pub fn starting_at(mut self, position: usize) -> Self {
        self.offset = position;
        self.check_first = true;
        self
    }

    async fn fetch_next_page(&mut self) -> Result<()> {
        // A page made only of channels yields nothing; keep going until
        // something is mapped or the results run out.
        while self.has_more && self.index >= self.items.len() {
            tracing::info!(
                "Fetching next page for search: {} (offset: {})",
                self.query,
                self.offset
            );

            let results = self
                .source
                .search(
                    &self.system,
                    self.content_type,
                    &self.query,
                    self.limit,
                    self.offset,
                )
                .await?;

            if results.is_empty() {
                self.check_first = false;
                self.has_more = false;
                return Ok(());
            }

            if std::mem::take(&mut self.check_first)
                && matches!(results.first(), Some(SearchResult::Channel(_)))
            {
                anyhow::bail!("The chosen search result is a channel and cannot be played");
            }

            self.offset += results.len();
            for result in &results {
                if let Some(item) = search_result_to_item(&self.system, result) {
                    self.items.push(item);
                    self.metas.extend(container_meta_of(&self.system, result));
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl CursorSource for StreamingSearchCursor {
    async fn next_batch(&mut self, limit: usize) -> Result<Vec<CursorItem>> {
        self.fetch_next_page().await?;

        if self.index >= self.items.len() {
            return Ok(Vec::new());
        }

        let end = (self.index + limit).min(self.items.len());
        let batch = self.items[self.index..end].to_vec();

        self.index = end;
        Ok(batch)
    }

    fn has_more(&self) -> bool {
        self.has_more || self.index < self.items.len()
    }

    fn container_meta(&self, context: &ContainerContext) -> Option<ContainerMeta> {
        let (kind, id) = match context {
            ContainerContext::ExternalAlbum { id, .. } => (ContainerKind::Album, id),
            ContainerContext::ExternalPlaylist { id, .. } => (ContainerKind::Playlist, id),
            _ => return None,
        };
        self.metas
            .iter()
            .find(|m| m.id == *id && m.kind == kind)
            .cloned()
    }
}
