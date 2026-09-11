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
            .expect("test mutex poisoned")
            .insert(format!("local:{id}"), items);
    }

    /// Serves `items` for an external album.
    pub fn serve_album(&self, id: &str, items: Vec<CursorItem>) {
        self.contents
            .lock()
            .expect("test mutex poisoned")
            .insert(format!("album:{id}"), items);
    }

    /// Serves `items` for an external playlist.
    pub fn serve_playlist(&self, id: &str, items: Vec<CursorItem>) {
        self.contents
            .lock()
            .expect("test mutex poisoned")
            .insert(format!("playlist:{id}"), items);
    }

    pub fn resolved_contexts(&self) -> Vec<ContainerContext> {
        self.resolved.lock().expect("test mutex poisoned").clone()
    }
}

#[async_trait]
impl CursorResolver for FakeResolver {
    async fn resolve(&self, context: ContainerContext) -> Result<Box<dyn CursorSource>> {
        let items = self
            .contents
            .lock()
            .expect("test mutex poisoned")
            .get(&context_key(&context))
            .cloned()
            .unwrap_or_default();
        self.resolved
            .lock()
            .expect("test mutex poisoned")
            .push(context);
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

/// Serves search results by raw offset, up to `page_size` per request, and records every
/// request as `(offset, limit)`.
pub struct RecordingPages {
    results: Vec<SearchResult>,
    page_size: usize,
    pub requests: Mutex<Vec<(usize, usize)>>,
}

impl RecordingPages {
    pub fn new(results: Vec<SearchResult>, page_size: usize) -> Arc<Self> {
        Arc::new(Self {
            results,
            page_size,
            requests: Mutex::new(Vec::new()),
        })
    }

    pub fn recorded(&self) -> Vec<(usize, usize)> {
        self.requests.lock().expect("test mutex poisoned").clone()
    }
}

#[async_trait]
impl SearchPageSource for RecordingPages {
    async fn search(
        &self,
        _system: &str,
        _content_type: Option<ContentType>,
        _query: &str,
        limit: usize,
        offset: usize,
    ) -> StreamResult<Vec<SearchResult>> {
        self.requests
            .lock()
            .expect("test mutex poisoned")
            .push((offset, limit));
        let end = self.results.len().min(offset + self.page_size.min(limit));
        Ok(self.results.get(offset..end).unwrap_or_default().to_vec())
    }
}

pub fn channel_result(id: &str) -> SearchResult {
    SearchResult::Channel(nexus_player_backend::streaming::types::ChannelInfo {
        id: id.into(),
        url: String::new(),
        service: StreamingService::Bandcamp,
        name: "Channel".into(),
        subscriber_count: None,
        track_count: None,
        thumbnail: None,
        description: None,
    })
}

/// A library on a temporary file database (not `sqlite::memory:`, where every pooled
/// connection would get its own database).
pub struct TempLibrary {
    pub service: Arc<nexus_player_backend::library::LibraryService>,
    _dir: tempfile::TempDir,
}

impl TempLibrary {
    pub async fn new() -> Self {
        let dir = tempfile::tempdir().expect("create temp dir");
        let service =
            nexus_player_backend::library::LibraryService::new(&dir.path().join("test.db"))
                .await
                .expect("open library");
        Self {
            service: Arc::new(service),
            _dir: dir,
        }
    }

    pub async fn track(&self, title: &str) -> nexus_player_backend::library::TrackId {
        use nexus_player_backend::library::models::{Metadata, Track};
        use nexus_player_backend::library::{CacheStatus, TrackId, TrackSource};
        let now = chrono::Utc::now();
        let track = Track {
            id: TrackId(0),
            metadata: Metadata {
                title: title.into(),
                description: None,
                artwork: None,
            },
            artist: Some("Artist".into()),
            album: None,
            album_artist: None,
            year: None,
            genre: None,
            track_number: None,
            source: TrackSource::External {
                system: "bandcamp".into(),
                id: format!("lib-{title}"),
            },
            cache: CacheStatus::None,
            audio_info: None,
            play_count: 0,
            last_played: None,
            added_at: now,
            updated_at: now,
            liked: false,
            liked_position: None,
            unavailable: false,
        };
        self.service.add_track(track).await.expect("add track")
    }

    pub async fn playlist(&self, title: &str) -> nexus_player_backend::library::PlaylistId {
        use nexus_player_backend::library::models::{Metadata, Playlist};
        use nexus_player_backend::library::PlaylistId;
        let now = chrono::Utc::now();
        self.service
            .create_playlist(Playlist {
                id: PlaylistId(0),
                metadata: Metadata {
                    title: title.into(),
                    description: None,
                    artwork: None,
                },
                creator: None,
                created_at: now,
                updated_at: now,
            })
            .await
            .expect("create playlist")
    }
}

/// Serves an endless run of track results, `page_size` per request, and remembers how many
/// results it has handed out.
pub struct EndlessPages {
    page_size: usize,
    served: std::sync::atomic::AtomicUsize,
}

impl EndlessPages {
    pub fn new(page_size: usize) -> Arc<Self> {
        Arc::new(Self {
            page_size,
            served: std::sync::atomic::AtomicUsize::new(0),
        })
    }

    pub fn served(&self) -> usize {
        self.served.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[async_trait]
impl SearchPageSource for EndlessPages {
    async fn search(
        &self,
        _system: &str,
        _content_type: Option<ContentType>,
        _query: &str,
        limit: usize,
        offset: usize,
    ) -> StreamResult<Vec<SearchResult>> {
        let count = self.page_size.min(limit);
        self.served
            .fetch_add(count, std::sync::atomic::Ordering::SeqCst);
        Ok((offset..offset + count).map(track_result).collect())
    }
}

/// A resolver whose local-playlist resolution blocks until the test opens that playlist's
/// gate. `entered` is notified each time a resolution starts waiting.
pub struct GatedResolver {
    inner: Arc<FakeResolver>,
    gates: Mutex<HashMap<i64, Arc<tokio::sync::Semaphore>>>,
    failing: Mutex<std::collections::HashSet<i64>>,
    pub entered: tokio::sync::Notify,
}

impl GatedResolver {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: FakeResolver::new(),
            gates: Mutex::new(HashMap::new()),
            failing: Mutex::new(Default::default()),
            entered: tokio::sync::Notify::new(),
        })
    }

    /// Serves `items` for local playlist `id`, behind a closed gate.
    pub fn serve_gated(&self, id: i64, items: Vec<CursorItem>) {
        self.inner.serve_local_playlist(id, items);
        self.gates
            .lock()
            .expect("test mutex poisoned")
            .insert(id, Arc::new(tokio::sync::Semaphore::new(0)));
    }

    /// Makes resolving local playlist `id` fail once its gate is open (or at once when it
    /// has no gate).
    pub fn fail_on(&self, id: i64) {
        self.failing.lock().expect("test mutex poisoned").insert(id);
    }

    pub fn open(&self, id: i64) {
        if let Some(gate) = self.gates.lock().expect("test mutex poisoned").get(&id) {
            gate.add_permits(1);
        }
    }
}

#[async_trait]
impl CursorResolver for GatedResolver {
    async fn resolve(&self, context: ContainerContext) -> Result<Box<dyn CursorSource>> {
        if let ContainerContext::LocalPlaylist(id) = &context {
            let gate = self
                .gates
                .lock()
                .expect("test mutex poisoned")
                .get(id)
                .cloned();
            if let Some(gate) = gate {
                self.entered.notify_one();
                gate.acquire().await.expect("gate closed").forget();
            }
            if self.failing.lock().expect("test mutex poisoned").contains(id) {
                anyhow::bail!("resolver failed for playlist {id}");
            }
        }
        self.inner.resolve(context).await
    }
}

/// Search pages that always fail.
pub struct FailingPages;

#[async_trait]
impl SearchPageSource for FailingPages {
    async fn search(
        &self,
        _system: &str,
        _content_type: Option<ContentType>,
        _query: &str,
        _limit: usize,
        _offset: usize,
    ) -> StreamResult<Vec<SearchResult>> {
        Err(nexus_player_backend::streaming::error::StreamError::Other(
            "search backend down".into(),
        ))
    }
}

/// Paged search results that record every request as `(offset, limit)` and fail any request
/// whose page covers raw position `fail_at`.
pub struct FlakyPages {
    inner: Arc<RecordingPages>,
    page_size: usize,
    fail_at: usize,
}

impl FlakyPages {
    pub fn new(results: Vec<SearchResult>, page_size: usize, fail_at: usize) -> Arc<Self> {
        Arc::new(Self {
            inner: RecordingPages::new(results, page_size),
            page_size,
            fail_at,
        })
    }
}

#[async_trait]
impl SearchPageSource for FlakyPages {
    async fn search(
        &self,
        system: &str,
        content_type: Option<ContentType>,
        query: &str,
        limit: usize,
        offset: usize,
    ) -> StreamResult<Vec<SearchResult>> {
        let end = offset + self.page_size.min(limit);
        if (offset..end).contains(&self.fail_at) {
            return Err(nexus_player_backend::streaming::error::StreamError::Other(
                "page unavailable".into(),
            ));
        }
        self.inner
            .search(system, content_type, query, limit, offset)
            .await
    }
}
