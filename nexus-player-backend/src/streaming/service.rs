use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use crate::audio::types::{MediaSource, TrackSource};
use crate::config::ProviderConfig;
use crate::streaming::external::{StreamingMediaSource, StreamingSourceWrapper};
use crate::streaming::types::{AlbumInfo, TrackInfo};
use streaming::multi_downloader::source::Source;

use super::cache::CacheManager;
use super::error::{StreamError, StreamResult};
use super::provider::StreamProvider;
use super::providers::{BandcampProvider, SoundCloudProvider, YtDlpProvider};
use super::types::{ContentType, SearchResult};
use streaming::state::manager::DownloadManager;

pub struct StreamingService {
    providers: HashMap<String, Arc<dyn StreamProvider>>,
    download_manager: DownloadManager,
    config: RwLock<ProviderConfig>,
}

impl StreamingService {
    pub fn new(
        cache_dir: PathBuf,
        cache_size_mb: u64,
        config: ProviderConfig,
    ) -> StreamResult<Self> {
        let cache = Arc::new(CacheManager::new(cache_dir, cache_size_mb)?);

        let ytdlp = Arc::new(YtDlpProvider::new(cache)) as Arc<dyn StreamProvider>;

        let bandcamp = Arc::new(BandcampProvider::new(ytdlp.clone())) as Arc<dyn StreamProvider>;
        let soundcloud =
            Arc::new(SoundCloudProvider::new(ytdlp.clone())) as Arc<dyn StreamProvider>;
        // let vk = Arc::new(VkProvider::new(ytdlp.clone())) as Arc<dyn StreamProvider>;

        let mut providers = HashMap::new();
        providers.insert("ytdlp".to_string(), ytdlp.clone());
        providers.insert("youtube".to_string(), ytdlp);
        providers.insert("bandcamp".to_string(), bandcamp);
        providers.insert("soundcloud".to_string(), soundcloud);
        // providers.insert("vk".to_string(), vk);

        let manager = DownloadManager::new();

        Ok(Self {
            providers,
            download_manager: manager,
            config: RwLock::new(config),
        })
    }

    pub fn update_config(&self, config: ProviderConfig) {
        if let Ok(mut c) = self.config.write() {
            *c = config;
        }
    }

    pub async fn search(
        &self,
        provider_name: &str,
        content_type: Option<ContentType>,
        query: &str,
        limit: usize,
    ) -> StreamResult<Vec<SearchResult>> {
        // Check if provider is enabled
        {
            let config = self.config.read().unwrap();
            let enabled = match provider_name {
                "youtube" | "ytdlp" => config.youtube.enabled,
                "soundcloud" => config.soundcloud.enabled,
                "bandcamp" => config.bandcamp.enabled,
                "vk" => config.vk.enabled,
                _ => true,
            };
            if !enabled {
                return Err(StreamError::Other(format!(
                    "Provider {} is disabled",
                    provider_name
                )));
            }
        }

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
    pub async fn expand_album(&self, search: AlbumInfo) -> StreamResult<Vec<TrackInfo>> {
        let provider_name = search.service.name().to_lowercase();
        if let Some(provider) = self.providers.get(&provider_name) {
            provider
                .expand_collection(SearchResult::Album(search))
                .await
        } else {
            Ok(vec![])
        }
    }

    pub async fn resolve(
        &self,
        source: TrackSource,
        track_id: u64,
    ) -> StreamResult<StreamingSourceWrapper> {
        println!("Source {:?}", source);
        match source {
            TrackSource::Local(path) => {
                let file = File::open(path)?;
                Ok(StreamingSourceWrapper::Local(Box::new(file)))
            }
            TrackSource::Http(_) => {
                if let Some(ytdlp_provider) = self.providers.get("ytdlp") {
                    let media_source = ytdlp_provider.get_stream(&source).await?;
                    Ok(StreamingSourceWrapper::Local(media_source))
                } else {
                    Err(StreamError::NoProviderFound)
                }
            }
            TrackSource::External { system, id } => {
                if let Some(provider) = self.providers.get(&system) {
                    let source = TrackSource::External { system, id };
                    if let Some(stream) = self.download_manager.get_track(track_id).await {
                        Ok(StreamingSourceWrapper::from_source(stream))
                    } else {
                        match provider.resolve(&source).await {
                            Ok(url) => {
                                match self.download_manager.add_track(&url, track_id).await {
                                    Ok(s) => Ok(StreamingSourceWrapper::from_source(s)),
                                    Err(e) => Err(StreamError::Other(e.to_string())),
                                }
                            }
                            Err(e) => Err(StreamError::Other(e.to_string())),
                        }
                    }
                } else {
                    Err(StreamError::NoProviderFound)
                }
            }
            _ => todo!(),
        }
    }
    pub async fn set_context(&self, track_id: u64, next: Option<u64>) {
        self.download_manager
            .set_context(Some(track_id), next)
            .await;
    }
    pub async fn debug(&self) {
        // let _ = self.download_manager.debug_to_file().await;
    }

    pub async fn save_track_to_file(
        &self,
        track_id: u64,
        dir: &std::path::Path,
    ) -> StreamResult<std::path::PathBuf> {
        if let Some((source, url)) = self.download_manager.get_track_with_url(track_id).await {
            let sanitized_filename: String = url
                .chars()
                .map(|c| match c {
                    '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                    _ => c,
                })
                .collect();

            let file_path = dir.join(format!("{}.mp3", sanitized_filename));

            source
                .save_to_file(&file_path)
                .map_err(|e| StreamError::Other(e.to_string()))?;

            Ok(file_path)
        } else {
            Err(StreamError::Other(
                "Track not found in download manager".to_string(),
            ))
        }
    }
}
