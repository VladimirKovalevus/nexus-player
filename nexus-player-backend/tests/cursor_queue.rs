mod common;

use std::sync::Arc;

use common::{
    album_result, channel_result, playlist_result, track_item, track_result, FakeResolver,
    RecordingPages, ScriptedPages, VecSource,
};
use nexus_player_backend::playlist::cursor::{
    ContainerContext, CursorItem, PlayerCursor, StreamingSearchCursor,
};
use nexus_player_backend::playlist::playback_queue::{
    ContainerKind, CursorQueue, QueueEntry, RepeatMode,
};
use nexus_player_backend::streaming::types::{ContentType, SearchResult};

/// A queue playing a search whose root page is `results`.
fn search_queue(results: Vec<SearchResult>, resolver: &Arc<FakeResolver>) -> CursorQueue {
    let source = StreamingSearchCursor::with_source(
        ScriptedPages::new(results, 20),
        "bandcamp".into(),
        "band".into(),
        ContentType::Track,
    );
    let mut queue = CursorQueue::new();
    queue.context = Some(ContainerContext::SearchQuery {
        system: "bandcamp".into(),
        query: "band".into(),
        content_type: ContentType::Track,
    });
    queue.set_cursor(PlayerCursor::new(Box::new(source), resolver.clone()));
    queue
}

fn label(entry: &QueueEntry) -> String {
    match entry {
        QueueEntry::Track(t) => t.title.clone(),
        QueueEntry::Container { info, .. } => format!("[{}]", info.title),
    }
}

fn labels(queue: &CursorQueue) -> Vec<String> {
    queue.snapshot().0.iter().map(label).collect()
}

fn current_label(queue: &CursorQueue) -> Option<String> {
    let (rows, current) = queue.snapshot();
    current.map(|i| label(&rows[i]))
}

fn album_tracks(resolver: &FakeResolver, id: &str, numbers: &[usize]) {
    resolver.serve_album(id, numbers.iter().map(|n| track_item(*n)).collect());
}

#[tokio::test]
async fn unreached_album_is_one_container_row_that_is_not_resolved() {
    let resolver = FakeResolver::new();
    album_tracks(&resolver, "a1", &[11, 12, 13]);
    let mut queue = search_queue(
        vec![
            track_result(1),
            album_result("a1", "Album A", 3),
            track_result(2),
        ],
        &resolver,
    );

    queue.next().await.unwrap();
    queue.prefetch(10).await.unwrap();

    assert_eq!(labels(&queue), vec!["Title 1", "[Album A]", "Title 2"]);
    assert_eq!(queue.snapshot().1, Some(0));
    match &queue.snapshot().0[1] {
        QueueEntry::Container { info, tracks, .. } => {
            assert!(matches!(info.kind, ContainerKind::Album));
            assert_eq!((info.id.as_str(), info.system.as_str()), ("a1", "bandcamp"));
            assert_eq!(info.track_count, 3);
            assert_eq!(info.artist, "Band");
            assert!(tracks.is_none(), "not previewed yet");
        }
        other => panic!("expected a container row, got {}", label(other)),
    }
    assert!(resolver.resolved_contexts().is_empty());
}

#[tokio::test]
async fn playlist_result_is_a_playlist_container_row() {
    let resolver = FakeResolver::new();
    let mut queue = search_queue(
        vec![track_result(1), playlist_result("p1", "Mix", 8)],
        &resolver,
    );
    queue.next().await.unwrap();
    queue.prefetch(10).await.unwrap();

    match &queue.snapshot().0[1] {
        QueueEntry::Container { info, .. } => {
            assert!(matches!(info.kind, ContainerKind::Playlist));
            assert_eq!((info.title.as_str(), info.track_count), ("Mix", 8));
        }
        other => panic!("expected a container row, got {}", label(other)),
    }
}

#[tokio::test]
async fn previewing_a_container_attaches_tracks_without_moving_rows_or_playback() {
    let resolver = FakeResolver::new();
    album_tracks(&resolver, "a1", &[11, 12, 13]);
    let mut queue = search_queue(
        vec![
            track_result(1),
            album_result("a1", "Album A", 3),
            track_result(2),
        ],
        &resolver,
    );
    queue.next().await.unwrap();
    queue.prefetch(10).await.unwrap();

    queue.expand_container(1).await.unwrap();

    let (rows, current) = queue.snapshot();
    assert_eq!(rows.len(), 3);
    assert_eq!(current, Some(0));
    assert_eq!(current_label(&queue).as_deref(), Some("Title 1"));
    match &rows[1] {
        QueueEntry::Container { tracks, .. } => {
            let titles: Vec<_> = tracks
                .as_ref()
                .expect("tracks attached")
                .iter()
                .map(|t| t.title.clone())
                .collect();
            assert_eq!(titles, vec!["Title 11", "Title 12", "Title 13"]);
        }
        other => panic!("expected a container row, got {}", label(other)),
    }
}

#[tokio::test]
async fn next_into_a_container_replaces_it_with_its_tracks() {
    let resolver = FakeResolver::new();
    album_tracks(&resolver, "a1", &[11, 12, 13]);
    let mut queue = search_queue(
        vec![
            track_result(1),
            album_result("a1", "Album A", 3),
            track_result(2),
        ],
        &resolver,
    );
    queue.next().await.unwrap();
    queue.prefetch(10).await.unwrap();

    let played = queue.next().await.unwrap().expect("a track");

    assert_eq!(played.title, "Title 11");
    assert_eq!(
        labels(&queue),
        vec!["Title 1", "Title 11", "Title 12", "Title 13", "Title 2"]
    );
    assert_eq!(queue.snapshot().1, Some(1));
}

#[tokio::test]
async fn playing_a_container_row_with_a_track_index_starts_at_that_track() {
    let resolver = FakeResolver::new();
    album_tracks(&resolver, "a1", &[11, 12, 13]);
    let mut queue = search_queue(
        vec![
            track_result(1),
            album_result("a1", "Album A", 3),
            track_result(2),
        ],
        &resolver,
    );
    queue.next().await.unwrap();
    queue.prefetch(10).await.unwrap();

    let played = queue
        .play_queue_item(1, Some(2))
        .await
        .unwrap()
        .expect("a track");

    assert_eq!(played.title, "Title 13");
    assert_eq!(current_label(&queue).as_deref(), Some("Title 13"));
    let all = labels(&queue);
    assert!(all.iter().all(|l| !l.starts_with('[')), "no container left");
    assert_eq!(all.last().map(String::as_str), Some("Title 2"));
}

#[tokio::test]
async fn playing_a_container_row_without_a_track_index_starts_at_its_first_track() {
    let resolver = FakeResolver::new();
    album_tracks(&resolver, "a1", &[11, 12]);
    let mut queue = search_queue(
        vec![track_result(1), album_result("a1", "Album A", 2)],
        &resolver,
    );
    queue.next().await.unwrap();
    queue.prefetch(10).await.unwrap();

    let played = queue.play_queue_item(1, None).await.unwrap().unwrap();

    assert_eq!(played.title, "Title 11");
}

#[tokio::test]
async fn removing_a_container_row_removes_the_whole_container() {
    let resolver = FakeResolver::new();
    album_tracks(&resolver, "a1", &[11, 12, 13]);
    let mut queue = search_queue(
        vec![
            track_result(1),
            album_result("a1", "Album A", 3),
            track_result(2),
            track_result(3),
        ],
        &resolver,
    );
    queue.next().await.unwrap();
    queue.prefetch(10).await.unwrap();

    let _ = queue.remove_track(1);

    assert_eq!(labels(&queue), vec!["Title 1", "Title 2", "Title 3"]);
    assert!(resolver.resolved_contexts().is_empty());
}

#[tokio::test]
async fn empty_container_is_skipped_when_playback_reaches_it() {
    let resolver = FakeResolver::new();
    // "e1" is not served, so it resolves to nothing.
    let mut queue = search_queue(
        vec![
            track_result(1),
            album_result("e1", "Empty", 3),
            track_result(2),
        ],
        &resolver,
    );
    queue.next().await.unwrap();
    queue.prefetch(10).await.unwrap();

    let played = queue.next().await.unwrap().expect("a track");

    assert_eq!(played.title, "Title 2");
    assert!(labels(&queue).iter().all(|l| !l.starts_with('[')));
}

#[tokio::test]
async fn local_playlist_context_yields_track_rows_only() {
    let resolver = FakeResolver::new();
    resolver.serve_local_playlist(9, vec![track_item(2), track_item(3)]);
    let source = VecSource::new(vec![
        track_item(1),
        CursorItem::Container(ContainerContext::LocalPlaylist(9)),
        track_item(4),
    ]);
    let mut queue = CursorQueue::new();
    queue.context = Some(ContainerContext::LocalPlaylist(1));
    queue.set_cursor(PlayerCursor::new(Box::new(source), resolver.clone()));

    queue.prefetch(10).await.unwrap();

    assert_eq!(
        labels(&queue),
        vec!["Title 1", "Title 2", "Title 3", "Title 4"]
    );
}

#[tokio::test]
async fn container_rows_count_as_one_position_of_the_prefetch_window() {
    let resolver = FakeResolver::new();
    let mut results = Vec::new();
    for i in 0..30 {
        results.push(track_result(i));
        results.push(album_result(&format!("a{i}"), &format!("Album {i}"), 5));
    }
    let mut queue = search_queue(results, &resolver);

    queue.prefetch(50).await.unwrap();

    let (rows, current) = queue.snapshot();
    assert_eq!(rows.len(), 50);
    assert_eq!(current, None);
    assert_eq!(
        rows.iter()
            .filter(|r| matches!(r, QueueEntry::Container { .. }))
            .count(),
        25
    );
    assert!(resolver.resolved_contexts().is_empty());
}

/// Starts a queue on a search at `position`, as the search playback path does.
async fn start_at(
    pages: Arc<RecordingPages>,
    resolver: &Arc<FakeResolver>,
    position: usize,
) -> anyhow::Result<(CursorQueue, nexus_player_backend::audio::types::Track)> {
    let mut queue = CursorQueue::new();
    let track = queue
        .start_search_at(
            pages,
            resolver.clone(),
            "bandcamp",
            "band",
            ContentType::Track,
            position,
        )
        .await?;
    Ok((queue, track))
}

/// 5 pages of 20 results; an album sits at position 47.
fn hundred_with_album_at_47() -> Vec<SearchResult> {
    (0..100)
        .map(|i| {
            if i == 47 {
                album_result("a47", "Album 47", 3)
            } else {
                track_result(i)
            }
        })
        .collect()
}

#[tokio::test]
async fn starting_at_an_album_position_plays_its_first_track_from_one_page() {
    let resolver = FakeResolver::new();
    album_tracks(&resolver, "a47", &[501, 502, 503]);
    let pages = RecordingPages::new(hundred_with_album_at_47(), 20);

    let (queue, track) = start_at(pages.clone(), &resolver, 47).await.unwrap();

    assert_eq!(track.title, "Title 501");
    assert_eq!(current_label(&queue).as_deref(), Some("Title 501"));
    assert!(resolver
        .resolved_contexts()
        .iter()
        .any(|c| matches!(c, ContainerContext::ExternalAlbum { id, .. } if id == "a47")));
    let requests = pages.recorded();
    assert_eq!(requests.len(), 1, "requests: {requests:?}");
    let (offset, limit) = requests[0];
    assert!(
        offset <= 47 && 47 < offset + limit,
        "requests: {requests:?}"
    );
}

#[tokio::test]
async fn starting_at_a_track_position_plays_that_track_without_listing_earlier_results() {
    let resolver = FakeResolver::new();
    let pages = RecordingPages::new(hundred_with_album_at_47(), 20);

    let (mut queue, track) = start_at(pages.clone(), &resolver, 63).await.unwrap();
    queue.prefetch(10).await.unwrap();

    assert_eq!(track.title, "Title 63");
    let all = labels(&queue);
    assert_eq!(all.first().map(String::as_str), Some("Title 63"));
    assert_eq!(queue.snapshot().1, Some(0));
    assert!(pages.recorded().iter().all(|(offset, _)| *offset > 20));
}

#[tokio::test]
async fn starting_past_the_last_result_is_an_error() {
    let resolver = FakeResolver::new();
    for position in [100, 500] {
        let pages = RecordingPages::new(hundred_with_album_at_47(), 20);
        assert!(start_at(pages, &resolver, position).await.is_err());
    }
}

#[tokio::test]
async fn starting_at_a_channel_position_is_an_error() {
    let resolver = FakeResolver::new();
    let mut results: Vec<SearchResult> = (0..40).map(track_result).collect();
    results[25] = channel_result("c25");
    let pages = RecordingPages::new(results, 20);

    assert!(start_at(pages, &resolver, 25).await.is_err());
}

fn numbered_search(count: usize, resolver: &Arc<FakeResolver>) -> CursorQueue {
    search_queue((0..count).map(track_result).collect(), resolver)
}

#[tokio::test]
async fn next_walks_a_paged_search_until_it_is_exhausted() {
    let resolver = FakeResolver::new();
    let mut queue = numbered_search(45, &resolver);

    let mut played = Vec::new();
    while let Some(track) = queue.next().await.unwrap() {
        played.push(track.title);
    }

    assert_eq!(played.len(), 45);
    assert_eq!(played[0], "Title 0");
    assert_eq!(played[44], "Title 44");
    assert!(queue.next().await.unwrap().is_none());
}

#[tokio::test]
async fn previous_after_next_returns_to_the_earlier_track() {
    let resolver = FakeResolver::new();
    let mut queue = numbered_search(5, &resolver);
    queue.next().await.unwrap();
    queue.next().await.unwrap();

    let back = queue.previous().expect("a previous track");

    assert_eq!(back.title, "Title 0");
    assert_eq!(current_label(&queue).as_deref(), Some("Title 0"));
    assert_eq!(queue.next().await.unwrap().unwrap().title, "Title 1");
}

#[tokio::test]
async fn jump_to_a_snapshot_row_makes_it_current() {
    let resolver = FakeResolver::new();
    let mut queue = numbered_search(6, &resolver);
    queue.next().await.unwrap();
    queue.prefetch(10).await.unwrap();

    queue.jump_to(3);

    assert_eq!(current_label(&queue).as_deref(), Some("Title 3"));
    assert_eq!(queue.snapshot().1, Some(3));
    assert_eq!(labels(&queue).len(), 6);
}

#[tokio::test]
async fn remove_track_drops_a_snapshot_row_and_keeps_the_current_track() {
    let resolver = FakeResolver::new();
    let mut queue = numbered_search(5, &resolver);
    queue.next().await.unwrap();
    queue.next().await.unwrap();
    queue.prefetch(10).await.unwrap();

    let removed = queue.remove_track(3).expect("a track");

    assert_eq!(removed.title, "Title 3");
    assert_eq!(current_label(&queue).as_deref(), Some("Title 1"));
    assert_eq!(
        labels(&queue),
        vec!["Title 0", "Title 1", "Title 2", "Title 4"]
    );
}

#[tokio::test]
async fn repeat_and_shuffle_toggling_keep_the_current_track() {
    let resolver = FakeResolver::new();
    let mut queue = numbered_search(5, &resolver);
    queue.next().await.unwrap();
    queue.prefetch(10).await.unwrap();

    queue.toggle_shuffle();
    queue.repeat_mode = RepeatMode::All;
    queue.repeat_mode = RepeatMode::One;
    queue.toggle_shuffle();

    assert_eq!(current_label(&queue).as_deref(), Some("Title 0"));
    assert_eq!(queue.next().await.unwrap().unwrap().title, "Title 0");
}

#[tokio::test]
async fn clear_empties_the_snapshot() {
    let resolver = FakeResolver::new();
    let mut queue = numbered_search(5, &resolver);
    queue.next().await.unwrap();
    queue.prefetch(10).await.unwrap();

    queue.clear();

    let (rows, current) = queue.snapshot();
    assert!(rows.is_empty());
    assert_eq!(current, None);
}

#[tokio::test]
async fn snapshot_current_index_follows_playback() {
    let resolver = FakeResolver::new();
    let mut queue = numbered_search(5, &resolver);
    assert_eq!(queue.snapshot().1, None);

    for expected in 0..3 {
        queue.next().await.unwrap();
        queue.prefetch(10).await.unwrap();
        assert_eq!(queue.snapshot().1, Some(expected));
    }
}
