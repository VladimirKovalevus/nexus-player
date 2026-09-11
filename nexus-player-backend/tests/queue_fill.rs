//! Queue fills build and prefetch outside the queue lock and swap in under a generation
//! check, so the last fill wins.
//!
//! Public fill API these tests settle on (module `nexus_player_backend::playlist::queue_fill`):
//!
//! ```ignore
//! pub enum FillOutcome { Applied, Discarded }
//!
//! pub async fn fill_context_queue(
//!     queue: &Arc<tokio::sync::Mutex<ActiveQueue>>,
//!     generation: &AtomicU64,
//!     resolver: Arc<dyn CursorResolver>,
//!     context: ContainerContext,
//!     start_index: usize,
//!     cancel: &CancellationToken,
//! ) -> anyhow::Result<FillOutcome>;
//!
//! pub async fn fill_search_queue(
//!     queue: &Arc<tokio::sync::Mutex<ActiveQueue>>,
//!     generation: &AtomicU64,
//!     resolver: Arc<dyn CursorResolver>,
//!     pages: Arc<dyn SearchPageSource>,
//!     system: &str,
//!     query: &str,
//!     content_type: ContentType,
//!     position: usize,
//!     cancel: &CancellationToken,
//! ) -> anyhow::Result<FillOutcome>;
//! ```
//!
//! `fill_search_queue_by_track_id` takes the same arguments but a `track_id: usize` (the
//! `Track::id` of the chosen result) instead of `position`; it looks for the track only in the
//! first page and fails when it is not there.
//!
//! A fill bumps `generation` when it starts, builds and prefetches into a fresh queue (repeat
//! mode and shuffle copied from the current queue), positions it at `start_index` / `position`,
//! and swaps it in only if its generation is still the latest. A discarded fill returns
//! `Ok(FillOutcome::Discarded)` and leaves the queue untouched.

mod common;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use common::{track_item, track_result, FailingPages, FlakyPages, GatedResolver, RecordingPages};
use nexus_player_backend::playlist::cursor::{ContainerContext, CursorResolver};
use nexus_player_backend::playlist::playback_queue::{ActiveQueue, CursorQueue, RepeatMode};
use nexus_player_backend::playlist::queue_fill::{
    fill_context_queue, fill_search_queue, fill_search_queue_by_track_id, FillOutcome,
};
use nexus_player_backend::streaming::types::ContentType;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

type Queue = Arc<Mutex<ActiveQueue>>;

fn queue_with_current(title_number: usize) -> Queue {
    let mut cq = CursorQueue::new();
    cq.add_track(track_item_track(title_number));
    cq.jump_to(0);
    Arc::new(Mutex::new(ActiveQueue::Cursor(Box::new(cq))))
}

fn track_item_track(n: usize) -> nexus_player_backend::audio::types::Track {
    match track_item(n) {
        nexus_player_backend::playlist::cursor::CursorItem::Track(t) => t,
        _ => panic!("track_item must produce a track"),
    }
}

async fn titles(queue: &Queue) -> (Option<String>, Vec<String>) {
    let guard = queue.lock().await;
    let ActiveQueue::Cursor(cq) = &*guard;
    (
        cq.current_track.as_ref().map(|t| t.title.clone()),
        cq.peek_upcoming_tracks(100)
            .into_iter()
            .map(|t| t.title)
            .collect(),
    )
}

async fn context_of(queue: &Queue) -> Option<ContainerContext> {
    let guard = queue.lock().await;
    let ActiveQueue::Cursor(cq) = &*guard;
    cq.context.clone()
}

fn local(id: i64) -> ContainerContext {
    ContainerContext::LocalPlaylist(id)
}

#[tokio::test]
async fn queue_lock_is_free_while_the_resolver_is_blocked() {
    let resolver = GatedResolver::new();
    resolver.serve_gated(1, vec![track_item(11), track_item(12)]);
    let queue = queue_with_current(1);
    let generation = Arc::new(AtomicU64::new(0));

    let fill = tokio::spawn({
        let (queue, generation, resolver) = (queue.clone(), generation.clone(), resolver.clone());
        async move {
            fill_context_queue(
                &queue,
                &generation,
                resolver as Arc<dyn CursorResolver>,
                local(1),
                0,
                &CancellationToken::new(),
            )
            .await
        }
    });
    resolver.entered.notified().await;

    let old = tokio::time::timeout(Duration::from_millis(500), titles(&queue))
        .await
        .expect("queue lock must be free while the resolver blocks");
    assert_eq!(old.0.as_deref(), Some("Title 1"));
    assert!(context_of(&queue).await.is_none(), "old state is untouched");

    resolver.open(1);
    let outcome = fill.await.expect("join").expect("fill");
    assert!(matches!(outcome, FillOutcome::Applied));
    let (current, upcoming) = titles(&queue).await;
    assert_eq!(current.as_deref(), Some("Title 11"));
    assert_eq!(upcoming, vec!["Title 12"]);
    assert!(matches!(
        context_of(&queue).await,
        Some(ContainerContext::LocalPlaylist(1))
    ));
}

#[tokio::test]
async fn later_of_two_overlapping_fills_wins_even_when_it_finishes_first() {
    let resolver = GatedResolver::new();
    resolver.serve_gated(1, vec![track_item(11), track_item(12)]);
    resolver.serve_gated(2, vec![track_item(21), track_item(22)]);
    let queue = queue_with_current(1);
    let generation = Arc::new(AtomicU64::new(0));

    let spawn_fill = |id: i64| {
        let (queue, generation, resolver) = (queue.clone(), generation.clone(), resolver.clone());
        tokio::spawn(async move {
            fill_context_queue(
                &queue,
                &generation,
                resolver as Arc<dyn CursorResolver>,
                local(id),
                0,
                &CancellationToken::new(),
            )
            .await
        })
    };

    let first = spawn_fill(1);
    resolver.entered.notified().await;
    let second = spawn_fill(2);
    resolver.entered.notified().await;

    // The second fill is released and swaps in; the first is released last.
    resolver.open(2);
    assert!(matches!(
        second.await.expect("join").expect("second fill"),
        FillOutcome::Applied
    ));
    resolver.open(1);
    assert!(matches!(
        first.await.expect("join").expect("first fill"),
        FillOutcome::Discarded
    ));

    let (current, upcoming) = titles(&queue).await;
    assert_eq!(current.as_deref(), Some("Title 21"));
    assert_eq!(upcoming, vec!["Title 22"]);
    assert!(matches!(
        context_of(&queue).await,
        Some(ContainerContext::LocalPlaylist(2))
    ));
}

#[tokio::test]
async fn fill_with_a_stale_generation_does_not_touch_the_queue() {
    let resolver = GatedResolver::new();
    resolver.serve_gated(1, vec![track_item(11)]);
    let queue = queue_with_current(1);
    let generation = Arc::new(AtomicU64::new(0));

    let fill = tokio::spawn({
        let (queue, generation, resolver) = (queue.clone(), generation.clone(), resolver.clone());
        async move {
            fill_context_queue(
                &queue,
                &generation,
                resolver as Arc<dyn CursorResolver>,
                local(1),
                0,
                &CancellationToken::new(),
            )
            .await
        }
    });
    resolver.entered.notified().await;
    // Something newer started in the meantime.
    generation.fetch_add(1, Ordering::SeqCst);
    resolver.open(1);

    assert!(matches!(
        fill.await.expect("join").expect("fill"),
        FillOutcome::Discarded
    ));
    let (current, upcoming) = titles(&queue).await;
    assert_eq!(current.as_deref(), Some("Title 1"));
    assert!(upcoming.is_empty());
    assert!(context_of(&queue).await.is_none());
}

#[tokio::test]
async fn context_fill_starts_at_the_given_index_and_keeps_repeat_and_shuffle() {
    let resolver = GatedResolver::new();
    resolver.serve_gated(1, vec![track_item(11), track_item(12), track_item(13)]);
    resolver.open(1);
    let queue: Queue = Arc::new(Mutex::new(ActiveQueue::new_cursor_with_settings(
        RepeatMode::All,
        true,
    )));
    let generation = AtomicU64::new(0);

    let outcome = fill_context_queue(
        &queue,
        &generation,
        resolver as Arc<dyn CursorResolver>,
        local(1),
        1,
        &CancellationToken::new(),
    )
    .await
    .expect("fill");
    assert!(matches!(outcome, FillOutcome::Applied));

    let guard = queue.lock().await;
    let ActiveQueue::Cursor(cq) = &*guard;
    assert_eq!(
        cq.current_track.as_ref().map(|t| t.title.as_str()),
        Some("Title 12")
    );
    assert!(matches!(cq.repeat_mode, RepeatMode::All));
    assert!(cq.shuffle_enabled);
}

#[tokio::test]
async fn failed_context_fill_reports_the_error_and_keeps_the_old_queue() {
    let resolver = GatedResolver::new();
    resolver.fail_on(7);
    let queue = queue_with_current(1);
    let generation = AtomicU64::new(0);

    let result = fill_context_queue(
        &queue,
        &generation,
        resolver as Arc<dyn CursorResolver>,
        local(7),
        0,
        &CancellationToken::new(),
    )
    .await;

    assert!(result.is_err());
    let (current, _) = titles(&queue).await;
    assert_eq!(current.as_deref(), Some("Title 1"));
}

#[tokio::test]
async fn failed_search_fill_reports_the_error_and_keeps_the_old_queue() {
    let resolver = GatedResolver::new();
    let queue = queue_with_current(1);
    let generation = AtomicU64::new(0);

    let result = fill_search_queue(
        &queue,
        &generation,
        resolver as Arc<dyn CursorResolver>,
        Arc::new(FailingPages),
        "bandcamp",
        "band",
        ContentType::Track,
        3,
        &CancellationToken::new(),
    )
    .await;

    assert!(result.is_err());
    let (current, _) = titles(&queue).await;
    assert_eq!(current.as_deref(), Some("Title 1"));
    assert!(context_of(&queue).await.is_none());
}

fn hundred_tracks() -> Vec<nexus_player_backend::streaming::types::SearchResult> {
    (0..100).map(track_result).collect()
}

const PRODUCTION_PREFETCH_PAGE_LIMIT: usize = 50;

#[tokio::test]
async fn search_fill_at_a_position_pages_only_as_far_as_needed() {
    let pages = RecordingPages::new(hundred_tracks(), 20);
    let queue = queue_with_current(1);
    let generation = AtomicU64::new(0);

    let outcome = fill_search_queue(
        &queue,
        &generation,
        common::FakeResolver::new() as Arc<dyn CursorResolver>,
        pages.clone(),
        "bandcamp",
        "band",
        ContentType::Track,
        47,
        &CancellationToken::new(),
    )
    .await
    .expect("fill");
    assert!(matches!(outcome, FillOutcome::Applied));

    let (current, upcoming) = titles(&queue).await;
    assert_eq!(current.as_deref(), Some("Title 47"));
    assert_eq!(upcoming.first().map(String::as_str), Some("Title 48"));
    assert_eq!(upcoming.len(), PRODUCTION_PREFETCH_PAGE_LIMIT);

    let requests = pages.recorded();
    assert!(requests.len() <= 3, "too many page requests: {requests:?}");
    for (offset, limit) in &requests {
        let end = offset + (*limit).min(20);
        assert!(
            end > 40,
            "request at {offset} only holds results before position 40: {requests:?}"
        );
    }
}

#[tokio::test]
async fn search_fill_whose_start_page_fails_keeps_the_previous_queue() {
    let pages = FlakyPages::new(hundred_tracks(), 20, 47);
    let queue = queue_with_current(1);
    let generation = AtomicU64::new(0);

    let result = fill_search_queue(
        &queue,
        &generation,
        common::FakeResolver::new() as Arc<dyn CursorResolver>,
        pages,
        "bandcamp",
        "band",
        ContentType::Track,
        47,
        &CancellationToken::new(),
    )
    .await;

    assert!(result.is_err());
    let (current, upcoming) = titles(&queue).await;
    assert_eq!(current.as_deref(), Some("Title 1"));
    assert!(upcoming.is_empty());
}

#[tokio::test]
async fn search_fill_by_track_id_missing_from_the_first_page_fails_after_one_request() {
    let pages = RecordingPages::new(hundred_tracks(), 20);
    let queue = queue_with_current(1);
    let generation = AtomicU64::new(0);
    let id_on_third_page = nexus_player_backend::audio::types::Track::from(match track_result(45) {
        nexus_player_backend::streaming::types::SearchResult::Track(t) => t,
        _ => panic!("track_result must produce a track"),
    })
    .id;

    let result = fill_search_queue_by_track_id(
        &queue,
        &generation,
        common::FakeResolver::new() as Arc<dyn CursorResolver>,
        pages.clone(),
        "bandcamp",
        "band",
        ContentType::Track,
        id_on_third_page,
        &CancellationToken::new(),
    )
    .await;

    assert!(result.is_err());
    assert_eq!(pages.recorded().len(), 1);
    let (current, _) = titles(&queue).await;
    assert_eq!(current.as_deref(), Some("Title 1"));
}
