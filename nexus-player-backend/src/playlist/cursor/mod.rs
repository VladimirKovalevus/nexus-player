use anyhow::Result;
use async_trait::async_trait;

pub mod types;
pub mod engine;
pub mod sources;
pub mod resolver;

pub use types::{ContainerContext, CursorItem, RadioMode};
pub use engine::{PlayerCursor, CursorResolver};
pub use sources::{StaticTrackCursor, DbTrackCursor, ProviderCursor, StreamingSearchCursor};
pub use resolver::AppCursorResolver;

#[async_trait]
pub trait CursorSource: Send + Sync {
    /// Fetch the next batch of items from the source.
    /// Returns an empty vector if the source is exhausted.
    async fn next_batch(&mut self, limit: usize) -> Result<Vec<CursorItem>>;

    /// Checks if the source has more items.
    /// Note: This might be an estimation or require a network check.
    fn has_more(&self) -> bool;
}
