use anyhow::Result;
use std::sync::Arc;
use async_trait::async_trait;

use crate::audio::types::Track;
use super::{CursorSource, CursorItem, ContainerContext};

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
}

impl PlayerCursor {
    pub fn new(root_source: Box<dyn CursorSource>, resolver: Arc<dyn CursorResolver>) -> Self {
        Self {
            stack: vec![root_source],
            resolver,
        }
    }

    /// Helper to create a cursor starting from a context.
    pub async fn from_context(context: ContainerContext, resolver: Arc<dyn CursorResolver>) -> Result<Self> {
        let root_source = resolver.resolve(context).await?;
        Ok(Self::new(root_source, resolver))
    }

    /// Fetches the next playable track.
    /// It automatically expands containers and pops exhausted cursors from the stack.
    pub async fn next_track(&mut self) -> Result<Option<Track>> {
        loop {
            // 1. Get the top cursor from the stack
            let active_cursor = match self.stack.last_mut() {
                Some(c) => c,
                None => return Ok(None), // Stack is empty, nothing to play
            };

            // 2. Try to get next item batch (we need at least one)
            let mut batch = active_cursor.next_batch(1).await?;
            
            if batch.is_empty() {
                // 5. EOF for current cursor: Pop it and continue with the previous one
                self.stack.pop();
                continue;
            }

            // Remove the first item from the batch
            let item = batch.remove(0);

            match item {
                // 3. We found a track! Return it to the player.
                CursorItem::Track(track) => return Ok(Some(track)),
                
                // 4. We found a container! Expand it by pushing a new source to the stack.
                CursorItem::Container(context) => {
                    let new_source = self.resolver.resolve(context).await?;
                    self.stack.push(new_source);
                    // Continue the loop to immediately start fetching from the new cursor
                }
            }
        }
    }

    /// Peek ahead to see upcoming items without consuming them.
    /// Note: This is complex with nested stacks and might require 
    /// a temporary clone or state management. 
    /// For now, it returns what's available in the current top cursor.
    pub async fn peek_next_batch(&mut self, limit: usize) -> Result<Vec<CursorItem>> {
        if let Some(active_cursor) = self.stack.last_mut() {
            // Ideally, the source should support peeking. 
            // If not, we might need a buffer inside PlayerCursor.
            active_cursor.next_batch(limit).await
        } else {
            Ok(Vec::new())
        }
    }
}
