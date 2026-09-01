mod common;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use common::{FakeResolver, VecSource};
use nexus_player_backend::audio::types::{Track, TrackSource};
use nexus_player_backend::playlist::cursor::{
    search_result_to_item, ContainerContext, CursorItem, CursorSource, PlayerCursor,
    SearchPageSource, StreamingSearchCursor,
};
use nexus_player_backend::streaming::error::StreamResult;
use nexus_player_backend::streaming::types::{
    AlbumInfo, ChannelInfo, ContentType, PlaylistInfo, SearchResult, StreamingService, TrackInfo,
};

fn track_info(n: usize) -> TrackInfo {
    TrackInfo {
        source: TrackSource::External {
            system: "soundcloud".into(),
            id: format!("t{n}"),
        },
        title: format!("Title {n}"),
        artist: "Artist".into(),
        duration: None,
        thumbnail: None,
        description: None,
    }
}

fn album(id: &str) -> SearchResult {
    SearchResult::Album(AlbumInfo {
        id: id.into(),
        tracks: vec![],
        url: String::new(),
        service: StreamingService::Bandcamp,
        title: "Album".into(),
        artist: "Artist".into(),
        track_count: 0,
        year: None,
        thumbnail: None,
        description: None,
    })
}

fn playlist(id: &str) -> SearchResult {
    SearchResult::Playlist(PlaylistInfo {
        id: id.into(),
        url: String::new(),
        service: StreamingService::SoundCloud,
        title: "Playlist".into(),
        creator: "Someone".into(),
        tracks: vec![],
        track_count: 0,
        thumbnail: None,
        description: None,
    })
}

fn channel(id: &str) -> SearchResult {
    SearchResult::Channel(ChannelInfo {
        id: id.into(),
        url: String::new(),
        service: StreamingService::SoundCloud,
        name: "Channel".into(),
        subscriber_count: None,
        track_count: None,
        thumbnail: None,
        description: None,
    })
}

fn title_of(item: &CursorItem) -> String {
    match item {
        CursorItem::Track(t) => t.title.clone(),
        CursorItem::Container(c) => format!("container:{c:?}"),
    }
}

#[test]
fn mapping_turns_track_album_and_playlist_into_items() {
    match search_result_to_item("soundcloud", &SearchResult::Track(track_info(1))) {
        Some(CursorItem::Track(t)) => assert_eq!(t.title, "Title 1"),
        other => panic!("expected a track item, got {other:?}"),
    }
    match search_result_to_item("bandcamp", &album("a1")) {
        Some(CursorItem::Container(ContainerContext::ExternalAlbum { system, id })) => {
            assert_eq!((system.as_str(), id.as_str()), ("bandcamp", "a1"));
        }
        other => panic!("expected an album container, got {other:?}"),
    }
    match search_result_to_item("soundcloud", &playlist("p1")) {
        Some(CursorItem::Container(ContainerContext::ExternalPlaylist { system, id })) => {
            assert_eq!((system.as_str(), id.as_str()), ("soundcloud", "p1"));
        }
        other => panic!("expected a playlist container, got {other:?}"),
    }
}

#[test]
fn mapping_skips_channels() {
    assert!(search_result_to_item("soundcloud", &channel("c1")).is_none());
}

#[tokio::test]
async fn cursor_over_page_with_channel_plays_tracks_and_never_searches_again() {
    let page = [
        SearchResult::Track(track_info(1)),
        channel("c1"),
        SearchResult::Track(track_info(2)),
    ];
    let items: Vec<CursorItem> = page
        .iter()
        .filter_map(|r| search_result_to_item("soundcloud", r))
        .collect();
    let resolver = FakeResolver::new();
    let mut cursor = PlayerCursor::new(Box::new(VecSource::new(items)), resolver.clone());

    let mut played = Vec::new();
    while let Some(t) = cursor.next_track().await.unwrap() {
        played.push(t.title);
    }

    assert_eq!(played, vec!["Title 1", "Title 2"]);
    assert!(
        resolver.resolved_contexts().is_empty(),
        "no container, and so no search query, may be resolved"
    );
}

struct FakePages {
    total: usize,
    channel_every: usize,
    requests: Mutex<Vec<usize>>,
}

#[async_trait]
impl SearchPageSource for FakePages {
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
            .push(offset);
        let page_size = limit.min(20);
        Ok((offset..self.total.min(offset + page_size))
            .map(|i| {
                if i % self.channel_every == 0 && i < 20 && i / self.channel_every < 3 {
                    channel(&format!("c{i}"))
                } else {
                    SearchResult::Track(track_info(i))
                }
            })
            .collect())
    }
}

#[tokio::test]
async fn search_cursor_pages_by_raw_count_and_yields_each_result_once() {
    // First page: 20 raw results, channels at indices 0, 7 and 14.
    let pages = Arc::new(FakePages {
        total: 30,
        channel_every: 7,
        requests: Mutex::new(Vec::new()),
    });
    let mut cursor = StreamingSearchCursor::with_source(
        pages.clone(),
        "soundcloud".into(),
        "query".into(),
        ContentType::Track,
    );

    let mut titles = Vec::new();
    loop {
        let batch = cursor.next_batch(5).await.unwrap();
        if batch.is_empty() {
            break;
        }
        titles.extend(batch.iter().map(title_of));
    }

    let requests = pages.requests.lock().expect("test mutex poisoned").clone();
    assert_eq!(requests[0], 0);
    assert_eq!(requests[1], 20, "second page starts after the raw results");

    let mut unique = titles.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), titles.len(), "no result is yielded twice");
    // 30 raw results minus 3 channels.
    assert_eq!(titles.len(), 27);
    assert!(titles.iter().all(|t| !t.starts_with("container:")));
}

#[tokio::test]
async fn first_page_of_search_cursor_has_17_items_when_three_are_channels() {
    let pages = Arc::new(FakePages {
        total: 20,
        channel_every: 7,
        requests: Mutex::new(Vec::new()),
    });
    let mut cursor = StreamingSearchCursor::with_source(
        pages.clone(),
        "soundcloud".into(),
        "q".into(),
        ContentType::Track,
    );
    let batch = cursor.next_batch(50).await.unwrap();
    let tracks: Vec<&Track> = batch
        .iter()
        .filter_map(|i| match i {
            CursorItem::Track(t) => Some(t),
            _ => None,
        })
        .collect();
    assert_eq!(batch.len(), 17);
    assert_eq!(tracks.len(), 17);
}

fn hundred_results() -> Vec<SearchResult> {
    (0..100)
        .map(|i| SearchResult::Track(track_info(i)))
        .collect()
}

#[tokio::test]
async fn positioned_search_cursor_requests_one_page_holding_the_position() {
    let pages = common::RecordingPages::new(hundred_results(), 20);
    let mut cursor = StreamingSearchCursor::with_source(
        pages.clone(),
        "soundcloud".into(),
        "q".into(),
        ContentType::Track,
    )
    .starting_at(47);

    let batch = cursor.next_batch(1).await.unwrap();

    let requests = pages.recorded();
    assert_eq!(requests.len(), 1, "requests: {requests:?}");
    let (offset, limit) = requests[0];
    assert_ne!(offset, 0, "must not start from the first page");
    assert!(
        offset <= 47 && 47 < offset + limit,
        "requests: {requests:?}"
    );
    assert_eq!(batch.len(), 1);
    assert_eq!(title_of(&batch[0]), "Title 47");
}

#[tokio::test]
async fn positioned_search_cursor_continues_with_the_results_after_the_position() {
    let pages = common::RecordingPages::new(hundred_results(), 20);
    let mut cursor = StreamingSearchCursor::with_source(
        pages.clone(),
        "soundcloud".into(),
        "q".into(),
        ContentType::Track,
    )
    .starting_at(47);

    let mut titles = Vec::new();
    loop {
        let batch = cursor.next_batch(10).await.unwrap();
        if batch.is_empty() {
            break;
        }
        titles.extend(batch.iter().map(title_of));
    }

    assert_eq!(titles.len(), 53);
    assert_eq!(titles.first().map(String::as_str), Some("Title 47"));
    assert_eq!(titles.last().map(String::as_str), Some("Title 99"));
}

#[tokio::test]
async fn positioned_search_cursor_counts_raw_positions_past_earlier_channels() {
    // Channels at raw positions 0, 7 and 14; the chosen result is at raw position 10.
    let pages = Arc::new(FakePages {
        total: 40,
        channel_every: 7,
        requests: Mutex::new(Vec::new()),
    });
    let mut cursor = StreamingSearchCursor::with_source(
        pages.clone(),
        "soundcloud".into(),
        "q".into(),
        ContentType::Track,
    )
    .starting_at(10);

    let batch = cursor.next_batch(1).await.unwrap();

    assert_eq!(title_of(&batch[0]), "Title 10");
    assert_eq!(pages.requests.lock().expect("test mutex poisoned").len(), 1);
}
