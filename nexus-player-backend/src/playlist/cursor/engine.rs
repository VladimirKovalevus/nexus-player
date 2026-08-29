use anyhow::Result;
use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::Arc;

use super::{ContainerContext, CursorItem, CursorRow, CursorSource};
use crate::audio::types::Track;

/// Trait for resolving a context into a concrete source.
/// This will be implemented by a factory that has access to Library/Streaming services.
#[async_trait]
pub trait CursorResolver: Send + Sync {
    async fn resolve(&self, context: ContainerContext) -> Result<Box<dyn CursorSource>>;
}

/// The main engine that manages a stack of CursorSources.
/// It implements a DFS-like traversal over nested content.
pub struct PlayerCursor {
    stack: Vec<Box<dyn CursorSource>>,
    resolver: Arc<dyn CursorResolver>,
    buffer: VecDeque<CursorItem>,
}

impl PlayerCursor {
    pub fn new(root_source: Box<dyn CursorSource>, resolver: Arc<dyn CursorResolver>) -> Self {
        Self {
            stack: vec![root_source],
            resolver,
            buffer: VecDeque::new(),
        }
    }

    /// Helper to create a cursor starting from a context.
    pub async fn from_context(
        context: ContainerContext,
        resolver: Arc<dyn CursorResolver>,
    ) -> Result<Self> {
        let root_source = resolver.resolve(context).await?;
        Ok(Self::new(root_source, resolver))
    }

    /// Fetches the next playable track.
    /// It automatically expands containers and pops exhausted cursors from the stack.
    pub async fn next_track(&mut self) -> Result<Option<Track>> {
        match self.next_row(false).await? {
            Some(CursorRow::Track(track)) => Ok(Some(track)),
            // Without `keep_root_containers` every container is expanded.
            Some(CursorRow::Container { .. }) | None => Ok(None),
        }
    }

    /// Like `next_track`, but with `keep_root_containers` a container yielded by the root
    /// source (and described by it) is returned as a row instead of being expanded.
    pub async fn next_row(&mut self, keep_root_containers: bool) -> Result<Option<CursorRow>> {
        loop {
            // 1. Check if we have items in the buffer first
            if let Some(item) = self.buffer.pop_front() {
                match item {
                    CursorItem::Track(track) => return Ok(Some(CursorRow::Track(track))),
                    CursorItem::Container(context) => {
                        if keep_root_containers && self.stack.len() == 1 {
                            if let Some(meta) = self.stack[0].container_meta(&context) {
                                return Ok(Some(CursorRow::Container { context, meta }));
                            }
                        }
                        let new_source = self.resolver.resolve(context).await?;
                        self.stack.push(new_source);
                        continue; // Immediately start fetching from the new cursor
                    }
                }
            }

            // 2. Buffer is empty, get the top cursor from the stack
            let active_cursor = match self.stack.last_mut() {
                Some(c) => c,
                None => return Ok(None), // Stack is empty, nothing to play
            };

            // 3. Fetch next batch from source
            let mut batch = active_cursor.next_batch(1).await?;

            if batch.is_empty() {
                // 4. EOF for current cursor: Pop it and continue with the previous one
                self.stack.pop();
                continue;
            }

            // 5. Put item in buffer and loop to handle it correctly
            self.buffer.push_back(batch.remove(0));
        }
    }

    /// Resolves `context` and collects every track in it, nested containers included.
    pub async fn collect_tracks(&self, context: ContainerContext) -> Result<Vec<Track>> {
        let mut cursor = Self::from_context(context, self.resolver.clone()).await?;
        let mut tracks = Vec::new();
        while let Some(track) = cursor.next_track().await? {
            tracks.push(track);
        }
        Ok(tracks)
    }
}
