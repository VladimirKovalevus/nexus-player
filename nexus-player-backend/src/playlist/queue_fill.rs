//! Queue fills: the cursor is built and prefetched into a fresh queue without holding the
//! queue lock; the lock is taken only to swap the result in, and only when no newer fill has
//! started since.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::playlist::cursor::{ContainerContext, CursorResolver, PlayerCursor, SearchPageSource};
use crate::playlist::playback_queue::{ActiveQueue, CursorQueue};
use crate::streaming::types::ContentType;

const PREFETCH: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillOutcome {
    /// The fresh queue replaced the old one.
    Applied,
    /// A newer fill started, or the operation was cancelled; the queue is untouched.
    Discarded,
}

/// A fresh queue carrying over repeat mode and shuffle, read under a short lock.
async fn fresh_queue(queue: &Mutex<ActiveQueue>) -> CursorQueue {
    let guard = queue.lock().await;
    let ActiveQueue::Cursor(old) = &*guard;
    let mut fresh = CursorQueue::new();
    fresh.repeat_mode = old.repeat_mode;
    fresh.shuffle_enabled = old.shuffle_enabled;
    fresh
}

/// Swaps `fresh` in when `my_generation` is still the latest.
async fn swap_in(
    queue: &Mutex<ActiveQueue>,
    generation: &AtomicU64,
    my_generation: u64,
    fresh: CursorQueue,
) -> FillOutcome {
    let mut guard = queue.lock().await;
    if generation.load(Ordering::SeqCst) != my_generation {
        tracing::debug!("Discarding stale queue fill (generation {my_generation})");
        return FillOutcome::Discarded;
    }
    *guard = ActiveQueue::Cursor(Box::new(fresh));
    FillOutcome::Applied
}

/// Fills the queue from `context` and positions it at row `start_index`.
pub async fn fill_context_queue(
    queue: &Arc<Mutex<ActiveQueue>>,
    generation: &AtomicU64,
    resolver: Arc<dyn CursorResolver>,
    context: ContainerContext,
    start_index: usize,
    cancel: &CancellationToken,
) -> Result<FillOutcome> {
    fill_context_queue_at(
        queue,
        generation,
        resolver,
        context,
        Some(start_index),
        cancel,
    )
    .await
}

/// Like `fill_context_queue`; with `start_index: None` nothing becomes current.
pub async fn fill_context_queue_at(
    queue: &Arc<Mutex<ActiveQueue>>,
    generation: &AtomicU64,
    resolver: Arc<dyn CursorResolver>,
    context: ContainerContext,
    start_index: Option<usize>,
    cancel: &CancellationToken,
) -> Result<FillOutcome> {
    let my_generation = generation.fetch_add(1, Ordering::SeqCst) + 1;
    let mut fresh = fresh_queue(queue).await;

    tokio::select! {
        r = async {
            let cursor = PlayerCursor::from_context(context.clone(), resolver).await?;
            fresh.context = Some(context);
            fresh.set_cursor(cursor);
            let prefetched = fresh.prefetch(PREFETCH).await?;
            tracing::info!("Prefetched {} tracks from cursor", prefetched);
            if let Some(index) = start_index {
                fresh.jump_to(index);
                if fresh.current_track.is_none() && index == 0 {
                    fresh.next().await?;
                }
            }
            Ok::<_, anyhow::Error>(())
        } => r?,
        _ = cancel.cancelled() => return Ok(FillOutcome::Discarded),
    };

    Ok(swap_in(queue, generation, my_generation, fresh).await)
}

/// Fills the queue from a search and starts at raw result `position`.
#[allow(clippy::too_many_arguments)]
pub async fn fill_search_queue(
    queue: &Arc<Mutex<ActiveQueue>>,
    generation: &AtomicU64,
    resolver: Arc<dyn CursorResolver>,
    pages: Arc<dyn SearchPageSource>,
    system: &str,
    query: &str,
    content_type: ContentType,
    position: usize,
    cancel: &CancellationToken,
) -> Result<FillOutcome> {
    let my_generation = generation.fetch_add(1, Ordering::SeqCst) + 1;
    let mut fresh = fresh_queue(queue).await;

    tokio::select! {
        r = async {
            fresh
                .start_search_at(pages, resolver, system, query, content_type, position)
                .await?;
            fresh.prefetch(PREFETCH).await?;
            Ok::<_, anyhow::Error>(())
        } => r?,
        _ = cancel.cancelled() => return Ok(FillOutcome::Discarded),
    }

    Ok(swap_in(queue, generation, my_generation, fresh).await)
}
