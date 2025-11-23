use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;

use crate::audio::types::{MediaSource, TrackSource};
use crate::streaming::types::ResolvedContent;

use super::cache::CacheManager;
use super::error::{StreamError, StreamResult};
use super::provider::StreamProvider;
use super::providers::{BandcampProvider, SoundCloudProvider, YtDlpProvider};
use super::types::{ContentType, SearchResult};

pub struct StreamingService {
    providers: HashMap<String, Arc<dyn StreamProvider>>,
}

impl StreamingService {
    pub fn new(cache_dir: PathBuf, cache_size_mb: u64) -> StreamResult<Self> {
        let cache = Arc::new(CacheManager::new(cache_dir, cache_size_mb)?);

        let ytdlp = Arc::new(YtDlpProvider::new(cache)) as Arc<dyn StreamProvider>;

        let bandcamp = Arc::new(BandcampProvider::new(ytdlp.clone())) as Arc<dyn StreamProvider>;
        let soundcloud =
            Arc::new(SoundCloudProvider::new(ytdlp.clone())) as Arc<dyn StreamProvider>;

        let mut providers = HashMap::new();
        providers.insert("ytdlp".to_string(), ytdlp.clone());
        providers.insert("youtube".to_string(), ytdlp);
        providers.insert("bandcamp".to_string(), bandcamp);
        providers.insert("soundcloud".to_string(), soundcloud);

        Ok(Self { providers })
    }

    pub async fn search(
        &self,
        provider_name: &str,
        content_type: Option<ContentType>,
        query: &str,
        limit: usize,
    ) -> StreamResult<Vec<SearchResult>> {
        let provider = self
            .providers
            .get(provider_name)
            .ok_or_else(|| StreamError::NoProviderFound)?;

        provider.search_typed(query, content_type, limit, 0).await
    }

    pub fn get_provider(&self, name: &str) -> Option<Arc<dyn StreamProvider>> {
        self.providers.get(name).cloned()
    }

    pub fn list_providers(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }

    pub fn resolve(&self, source: &SearchResult) -> StreamResult<ResolvedContent> {
        match source {
            SearchResult::Track(track) => Ok(ResolvedContent::Track(track.into())),
            SearchResult::Playlist(playlist) => Ok(ResolvedContent::Playlist(playlist.into())),
            SearchResult::Album(_) => todo!(),
            SearchResult::Channel(_) => todo!(),
        }
    }
}
