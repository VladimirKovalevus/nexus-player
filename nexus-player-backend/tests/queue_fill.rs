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
//! A fill bumps `generation` when it starts, builds and prefetches into a fresh queue (repeat
//! mode and shuffle copied from the current queue), positions it at `start_index` / `position`,
//! and swaps it in only if its generation is still the latest. A discarded fill returns
//! `Ok(FillOutcome::Discarded)` and leaves the queue untouched.

mod common;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use common::{track_item, FailingPages, GatedResolver};
use nexus_player_backend::playlist::cursor::{ContainerContext, CursorResolver};
use nexus_player_backend::playlist::playback_queue::{ActiveQueue, CursorQueue, RepeatMode};
use nexus_player_backend::playlist::queue_fill::{
    fill_context_queue, fill_search_queue, FillOutcome,
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
