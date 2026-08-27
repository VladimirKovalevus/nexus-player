#![allow(dead_code)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use async_trait::async_trait;
use nexus_player_backend::playlist::cursor::{
    ContainerContext, CursorItem, CursorResolver, CursorSource,
};

/// A source that yields a fixed list of items.
pub struct VecSource {
    items: VecDeque<CursorItem>,
}

impl VecSource {
    pub fn new(items: Vec<CursorItem>) -> Self {
        Self {
            items: items.into(),
        }
    }
}

#[async_trait]
impl CursorSource for VecSource {
    async fn next_batch(&mut self, limit: usize) -> Result<Vec<CursorItem>> {
        let n = limit.min(self.items.len());
        Ok(self.items.drain(..n).collect())
    }

    fn has_more(&self) -> bool {
        !self.items.is_empty()
    }
}

/// A resolver that records every context it is asked to resolve and returns empty sources.
#[derive(Default)]
pub struct FakeResolver {
    pub resolved: Mutex<Vec<ContainerContext>>,
}

impl FakeResolver {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn resolved_contexts(&self) -> Vec<ContainerContext> {
        self.resolved.lock().unwrap().clone()
    }
}

#[async_trait]
impl CursorResolver for FakeResolver {
    async fn resolve(&self, context: ContainerContext) -> Result<Box<dyn CursorSource>> {
        self.resolved.lock().unwrap().push(context);
        Ok(Box::new(VecSource::new(Vec::new())))
    }
}
