use anyhow::Result;
use async_trait::async_trait;

pub mod engine;
pub mod resolver;
pub mod sources;
pub mod types;

pub use engine::{CursorResolver, PlayerCursor};
pub use resolver::AppCursorResolver;
pub use sources::{DbTrackCursor, ProviderCursor, StreamingSearchCursor};
pub use types::{ContainerContext, CursorItem, RadioMode};

#[async_trait]
pub trait CursorSource: Send + Sync {
    /// Fetch the next batch of items from the source.
    /// Returns an empty vector if the source is exhausted.
    async fn next_batch(&mut self, limit: usize) -> Result<Vec<CursorItem>>;

    /// Checks if the source has more items.
    /// Note: This might be an estimation or require a network check.
    fn has_more(&self) -> bool;
}
