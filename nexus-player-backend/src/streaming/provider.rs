use crate::audio::types::MediaSource;
use crate::library::TrackSource;

use super::error::StreamResult;
use super::types::{ContentType, SearchResult, StreamMetadata};

use async_trait::async_trait;

#[async_trait]
pub trait StreamProvider: Send + Sync {
    fn name(&self) -> &str;

    fn resolve(&self, source: &TrackSource) -> bool;

    async fn fetch_metadata(&self, source: &TrackSource) -> StreamResult<StreamMetadata>;

    async fn get_stream(&self, source: &TrackSource) -> StreamResult<Box<dyn MediaSource>>;

    async fn search_typed(
        &self,
        _query: &str,
        _content_type: Option<ContentType>,
        _limit: usize,
        _offset: usize,
    ) -> StreamResult<Vec<SearchResult>> {
        use super::error::StreamError;
        Err(StreamError::NotSupported)
    }

    async fn expand_collection(&self, _source: &TrackSource) -> StreamResult<Vec<TrackSource>> {
        use super::error::StreamError;
        Err(StreamError::NotSupported)
    }
    async fn is_available(&self) -> bool {
        true
    }
}
