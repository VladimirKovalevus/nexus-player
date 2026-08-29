#![allow(dead_code)]

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use async_trait::async_trait;
use nexus_player_backend::audio::types::TrackSource;
use nexus_player_backend::playlist::cursor::{
    ContainerContext, CursorItem, CursorResolver, CursorSource, SearchPageSource,
};
use nexus_player_backend::streaming::error::StreamResult;
use nexus_player_backend::streaming::types::{
    AlbumInfo, ContentType, PlaylistInfo, SearchResult, StreamingService, TrackInfo,
};

/// A source that yields a fixed list of items.
pub struct VecSource {
    items: VecDeque<CursorItem>,
}

impl VecSource {
    pub fn new(items: Vec<CursorItem>) -> Self {
        Self {
            items: items.into(),
        }
    }
}

#[async_trait]
impl CursorSource for VecSource {
    async fn next_batch(&mut self, limit: usize) -> Result<Vec<CursorItem>> {
        let n = limit.min(self.items.len());
        Ok(self.items.drain(..n).collect())
    }

    fn has_more(&self) -> bool {
        !self.items.is_empty()
    }
}

/// A resolver that records every context it is asked to resolve. Contexts without served
/// contents resolve to an empty source.
#[derive(Default)]
pub struct FakeResolver {
    pub resolved: Mutex<Vec<ContainerContext>>,
    contents: Mutex<HashMap<String, Vec<CursorItem>>>,
}

fn context_key(context: &ContainerContext) -> String {
    match context {
        ContainerContext::LocalPlaylist(id) => format!("local:{id}"),
        ContainerContext::ExternalAlbum { id, .. } => format!("album:{id}"),
        ContainerContext::ExternalPlaylist { id, .. } => format!("playlist:{id}"),
        other => format!("other:{other:?}"),
    }
}

impl FakeResolver {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Serves `items` for a local playlist.
    pub fn serve_local_playlist(&self, id: i64, items: Vec<CursorItem>) {
        self.contents
            .lock()
            .unwrap()
            .insert(format!("local:{id}"), items);
    }

    /// Serves `items` for an external album.
    pub fn serve_album(&self, id: &str, items: Vec<CursorItem>) {
        self.contents
            .lock()
            .unwrap()
            .insert(format!("album:{id}"), items);
    }

    /// Serves `items` for an external playlist.
    pub fn serve_playlist(&self, id: &str, items: Vec<CursorItem>) {
        self.contents
            .lock()
            .unwrap()
            .insert(format!("playlist:{id}"), items);
    }

    pub fn resolved_contexts(&self) -> Vec<ContainerContext> {
        self.resolved.lock().unwrap().clone()
    }
}

#[async_trait]
impl CursorResolver for FakeResolver {
    async fn resolve(&self, context: ContainerContext) -> Result<Box<dyn CursorSource>> {
        let items = self
            .contents
            .lock()
            .unwrap()
            .get(&context_key(&context))
            .cloned()
            .unwrap_or_default();
        self.resolved.lock().unwrap().push(context);
        Ok(Box::new(VecSource::new(items)))
    }
}

pub fn track_info(n: usize) -> TrackInfo {
    TrackInfo {
        source: TrackSource::External {
            system: "bandcamp".into(),
            id: format!("t{n}"),
        },
        title: format!("Title {n}"),
        artist: "Artist".into(),
        duration: None,
        thumbnail: None,
        description: None,
    }
}

/// A track item as a collection would serve it, titled `Title {n}`.
pub fn track_item(n: usize) -> CursorItem {
    CursorItem::Track(track_info(n).into())
}

pub fn track_result(n: usize) -> SearchResult {
    SearchResult::Track(track_info(n))
}

pub fn album_result(id: &str, title: &str, track_count: usize) -> SearchResult {
    SearchResult::Album(AlbumInfo {
        id: id.into(),
        tracks: vec![],
        url: String::new(),
        service: StreamingService::Bandcamp,
        title: title.into(),
        artist: "Band".into(),
        track_count,
        year: None,
        thumbnail: None,
        description: None,
    })
}

pub fn playlist_result(id: &str, title: &str, track_count: usize) -> SearchResult {
    SearchResult::Playlist(PlaylistInfo {
        id: id.into(),
        url: String::new(),
        service: StreamingService::Bandcamp,
        title: title.into(),
        creator: "Someone".into(),
        tracks: vec![],
        track_count,
        thumbnail: None,
        description: None,
    })
}

/// Serves a fixed list of search results page by page.
pub struct ScriptedPages {
    results: Vec<SearchResult>,
    page_size: usize,
}

impl ScriptedPages {
    pub fn new(results: Vec<SearchResult>, page_size: usize) -> Arc<Self> {
        Arc::new(Self { results, page_size })
    }
}

#[async_trait]
impl SearchPageSource for ScriptedPages {
    async fn search(
        &self,
        _system: &str,
        _content_type: Option<ContentType>,
        _query: &str,
        limit: usize,
        offset: usize,
    ) -> StreamResult<Vec<SearchResult>> {
        let end = self.results.len().min(offset + self.page_size.min(limit));
        Ok(self.results.get(offset..end).unwrap_or_default().to_vec())
    }
}
