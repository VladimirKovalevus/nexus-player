use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use crate::audio::types::{MediaSource, TrackSource};
use crate::config::{AudioConfig, ProviderConfig};
use crate::library::{CacheStatus, LibraryService};
use crate::streaming::external::{StreamingMediaSource, StreamingSourceWrapper};
use crate::streaming::types::{AlbumInfo, ResolveOptions, TrackInfo};
use streaming::multi_downloader::source::Source;

use super::error::{StreamError, StreamResult};
use super::provider::StreamProvider;
use super::providers::{BandcampProvider, SoundCloudProvider, VkProvider, YtDlpProvider};
use super::types::{
    ContentType, PlaylistInfo, SearchResult, StreamingService as StreamingServiceEnum,
};
use std::str::FromStr;
use streaming::state::manager::DownloadManager;
use streaming::state::types::{DownloadOptions, ManagerConfig, TaskInfo, DefaultCallbacks};

pub struct StreamingService {
    providers: HashMap<String, Arc<dyn StreamProvider>>,
    download_manager: DownloadManager,
    config: RwLock<ProviderConfig>,
    library: Arc<LibraryService>,
}

impl StreamingService {
    pub fn new(
        config: ProviderConfig,
        config_audio: AudioConfig,
        library: Arc<LibraryService>,
        default_callbacks: DefaultCallbacks,
    ) -> StreamResult<Self> {
        let bandcamp = Arc::new(BandcampProvider::new()) as Arc<dyn StreamProvider>;
        let soundcloud = Arc::new(SoundCloudProvider::new()) as Arc<dyn StreamProvider>;
        let vk =
            Arc::new(VkProvider::new(config.vk.credentials.clone())) as Arc<dyn StreamProvider>;

        let mut providers = HashMap::new();

        providers.insert("bandcamp".to_string(), bandcamp);
        providers.insert("soundcloud".to_string(), soundcloud);
        providers.insert("vk".to_string(), vk);

        let manager = DownloadManager::new(ManagerConfig {
            waveform_enabled: config_audio.waveform_enabled,
            mem_limit_bytes: config_audio.mem_limit,
            default_callbacks,
        });

        Ok(Self {
            providers,
            download_manager: manager,
            config: RwLock::new(config),
            library: library,
        })
    }

    pub async fn update_config(&self, config: ProviderConfig, config_audio: AudioConfig) {
        if let Ok(mut c) = self.config.write() {
            *c = config.clone();
        }
        
        let mut manager_config = self.download_manager.config.read().await.clone();
        manager_config.waveform_enabled = config_audio.waveform_enabled;
        manager_config.mem_limit_bytes = config_audio.mem_limit;

        self.download_manager
            .update_config(manager_config)
            .await;

        if let Some(vk) = self.providers.get("vk") {
            vk.set_credentials(config.vk.credentials.clone()).await;
        }
    }

    pub async fn list_tasks(&self) -> Vec<TaskInfo> {
        self.download_manager.list_tasks().await
    }

    pub async fn remove_task(&self, track_id: u64) {
        self.download_manager.remove_track(track_id).await;
    }
    pub async fn update_task(&self, task_id: u64, options: DownloadOptions) {
        self.download_manager.update_task(task_id, options).await;
    }
    pub async fn add_download_task(
        &self,
        source: TrackSource,
        task_id: u64,
        options: Option<DownloadOptions>,
    ) -> StreamResult<()> {
        if let Some(provider) = self.providers.get(source.system()) {
            let mut effective_options = options.unwrap_or(DownloadOptions {
                user_agent: None,
                on_start: None,
                on_progress: None,
                on_finish: None,
            });

            if source.system() == "vk" && effective_options.user_agent.is_none() {
                effective_options.user_agent = Some(vk_api::auth::VK_USER_AGENT.to_string());
            }
            return match provider
                .resolve(
                    &source,
                    Some(ResolveOptions {
                        user_agent: effective_options.user_agent.clone(),
                    }),
                )
                .await
            {
                Ok(url) => {
                    match self
                        .download_manager
                        .add_download_task(url.as_str(), task_id, Some(effective_options))
                        .await
                    {
                        Ok(_) => Ok(()),
                        Err(e) => Err(StreamError::Other(e.to_string())),
                    }
                }
                Err(e) => Err(e),
            };
        }
        Err(StreamError::NoProviderFound)
    }

    pub async fn search(
        &self,
        provider_name: &str,
        content_type: Option<ContentType>,
        query: &str,
        limit: usize,
        offset: usize,
    ) -> StreamResult<Vec<SearchResult>> {
        // Check if provider is enabled
        {
            let config = self.config.read().unwrap();
            let enabled = match provider_name {
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

        provider
            .search_typed(query, content_type, limit, offset)
            .await
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

    pub async fn expand_collection(
        &self,
        provider_name: &str,
        content_type: ContentType,
        id: &str,
    ) -> StreamResult<Vec<TrackInfo>> {
        let provider = self
            .providers
            .get(provider_name)
            .ok_or_else(|| StreamError::NoProviderFound)?;

        // We can create a dummy SearchResult or change expand_collection signature.
        // For now, let's use a dummy SearchResult since that's what the trait expects.
        let collection = match content_type {
            ContentType::Playlist => SearchResult::Playlist(PlaylistInfo {
                id: id.to_string(),
                url: id.to_string(),
                service: StreamingServiceEnum::from_str(provider_name)
                    .unwrap_or(StreamingServiceEnum::Bandcamp),
                title: "".to_string(),
                creator: "".to_string(),
                tracks: vec![],
                track_count: 0,
                thumbnail: None,
                description: None,
            }),
            ContentType::Album => SearchResult::Album(AlbumInfo {
                id: id.to_string(),
                url: id.to_string(),
                service: StreamingServiceEnum::from_str(provider_name)
                    .unwrap_or(StreamingServiceEnum::Bandcamp),
                title: "".to_string(),
                artist: "".to_string(),
                tracks: vec![],
                track_count: 0,
                year: None,
                thumbnail: None,
                description: None,
            }),
            _ => return Err(StreamError::NotSupported),
        };

        let res = provider.expand_collection(collection).await;
        println!("EXPAND RESULT{res:?}");
        res
    }

    pub async fn resolve(
        &self,
        source: TrackSource,
        track_id: u64,
        options: Option<DownloadOptions>,
    ) -> StreamResult<StreamingSourceWrapper> {
        println!("Source {:?}", source);
        if let Ok(track) = self.library.find_by_source(&source).await {
            if let Some(track) = track {
                match track.cache {
                    CacheStatus::Cached {
                        path,
                        downloaded_at: _,
                    } => {
                        let file = File::open(path)?;
                        return Ok(StreamingSourceWrapper::Local(Box::new(file)));
                    }
                    _ => {}
                }
            }
        }

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
                    let mut effective_options = options.unwrap_or(DownloadOptions {
                        user_agent: None,
                        on_start: None,
                        on_progress: None,
                        on_finish: None,
                    });

                    if system == "vk" && effective_options.user_agent.is_none() {
                        effective_options.user_agent =
                            Some(vk_api::auth::VK_USER_AGENT.to_string());
                    }

                    let source = TrackSource::External { system, id };
                    if let Some(stream) = self.download_manager.get_track(track_id).await {
                        Ok(StreamingSourceWrapper::from_source(stream))
                    } else {
                        match provider
                            .resolve(
                                &source,
                                Some(ResolveOptions {
                                    user_agent: effective_options.user_agent.clone(),
                                }),
                            )
                            .await
                        {
                            Ok(url) => {
                                match self
                                    .download_manager
                                    .add_track(&url, track_id, Some(effective_options))
                                    .await
                                {
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

    pub async fn has_track(&self, track_id: u64) -> bool {
        self.download_manager.get_track(track_id).await.is_some()
    }
}
