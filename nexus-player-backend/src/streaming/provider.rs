use crate::audio::types::MediaSource;
use crate::library::TrackSource;
use crate::streaming::types::{ResolveOptions, ResolvedContent};

use super::error::StreamResult;
use super::types::{ContentType, PlaylistInfo, SearchResult, StreamMetadata, TrackInfo};

use async_trait::async_trait;

#[async_trait]
pub trait StreamProvider: Send + Sync {
    fn name(&self) -> &str;

    async fn resolve(
        &self,
        source: &TrackSource,
        options: Option<ResolveOptions>,
    ) -> StreamResult<(String, StreamMetadata)>;

    async fn fetch_metadata(&self, source: &TrackSource) -> StreamResult<StreamMetadata>;

    async fn get_stream(&self, source: &TrackSource) -> StreamResult<Box<dyn MediaSource>>;

    async fn get_stream_v2(&self, source: &ResolvedContent) -> StreamResult<Box<dyn MediaSource>>;

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

    async fn expand_collection(&self, _collection: SearchResult) -> StreamResult<Vec<TrackInfo>>;

    async fn get_profile_content(&self) -> StreamResult<(Vec<TrackInfo>, Vec<PlaylistInfo>)> {
        Ok((vec![], vec![]))
    }

    async fn is_available(&self) -> bool {
        true
    }

    async fn set_credentials(&self, _credentials: Option<String>) {}

    async fn direct_link_resolve(&self, _url: &str) -> StreamResult<SearchResult> {
        use super::error::StreamError;
        Err(StreamError::NotSupported)
    }
}
