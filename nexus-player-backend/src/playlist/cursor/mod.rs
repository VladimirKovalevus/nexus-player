use anyhow::Result;
use async_trait::async_trait;

pub mod engine;
pub mod resolver;
pub mod sources;
pub mod types;

pub use engine::{CursorResolver, PlayerCursor};
pub use resolver::AppCursorResolver;
pub use sources::{
    search_result_to_item, DbTrackCursor, LocalPlaylistSource, ProviderCursor, RadioSource,
    SearchPageSource, StreamingSearchCursor,
};
pub use types::{
    ContainerContext, ContainerKind, ContainerMeta, CursorItem, CursorRow, RadioBlock, RadioMode,
};

#[async_trait]
pub trait CursorSource: Send + Sync {
    /// Fetch the next batch of items from the source.
    /// Returns an empty vector if the source is exhausted.
    async fn next_batch(&mut self, limit: usize) -> Result<Vec<CursorItem>>;

    /// Checks if the source has more items.
    /// Note: This might be an estimation or require a network check.
    fn has_more(&self) -> bool;

    /// Describes a container this source yielded, so a queue can show it as a row before
    /// it is expanded. Sources whose containers carry no description return `None`.
    fn container_meta(&self, _context: &ContainerContext) -> Option<ContainerMeta> {
        None
    }
}
