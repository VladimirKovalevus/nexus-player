use super::AppState;
use crate::config::AppConfig;
use crate::streaming::artwork::ArtworkLocation;
use std::sync::Arc;
use streaming::images::{ImageVariant, OnReady};

impl AppState {
    // ========================================================================
    // Artwork cache
    // ========================================================================

    /// Resolve a batch of cover URLs to local files, queueing background fetches
    /// for the ones that aren't cached yet.
    ///
    /// Batched because the UI resolves a whole viewport at once: a virtualized
    /// list re-resolves every visible row on each scroll pass, and one call per
    /// row would trade a rendering stall for an IPC one.
    /// Only covers belonging to something saved in the library are fetched.
    /// Transient listings — search results above all — resolve to `None` and pull
    /// no traffic; the UI keeps showing the remote URL for those.
    pub async fn resolve_artwork(
        &self,
        urls: &[String],
        variant: ImageVariant,
    ) -> Vec<Option<String>> {
        let mut out = Vec::with_capacity(urls.len());
        let mut uncached = Vec::new();

        for url in urls {
            match self.artwork.lookup(url, variant) {
                ArtworkLocation::Local(path) => out.push(Some(path.to_string_lossy().into_owned())),
                ArtworkLocation::Passthrough(url) => out.push(Some(url)),
                ArtworkLocation::Pending => {
                    out.push(None);
                    uncached.push(url.clone());
                }
            }
        }

        if uncached.is_empty() {
            return out;
        }

        // Already-cached covers are served above without consulting the library:
        // the file exists, and re-checking would only cost a query.
        match self.library.known_artwork_urls(&uncached).await {
            Ok(known) => {
                for url in uncached.iter().filter(|url| known.contains(*url)) {
                    self.artwork.request(url, variant);
                }
            }
            Err(e) => {
                // Fetching nothing is the safe failure here — the remote URL still renders.
                tracing::warn!("Artwork library check failed, skipping prefetch: {}", e);
            }
        }

        out
    }

    /// Hold artwork fetching while audio is downloading, so covers never compete
    /// with playback for bandwidth.
    pub fn set_artwork_paused(&self, paused: bool) {
        self.artwork.set_paused(paused);
    }

    pub fn set_artwork_on_ready(&self, callback: Arc<OnReady>) {
        self.artwork.set_on_ready(callback);
    }

    pub async fn prune_artwork_cache(&self) -> anyhow::Result<()> {
        self.artwork.prune().await
    }

    pub async fn clear_artwork_cache(&self) -> anyhow::Result<()> {
        self.artwork.clear().await
    }

    pub fn config(&self) -> AppConfig {
        self.config_manager.get()
    }

    pub async fn update_config(&self, config: AppConfig) -> anyhow::Result<()> {
        self.config_manager.update(config.clone())?;
        let fade_in_ms = if config.audio.fade_in_enabled {
            config.audio.fade_in_ms
        } else {
            0
        };
        let fade_out_ms = if config.audio.fade_out_enabled {
            config.audio.fade_out_ms
        } else {
            0
        };
        self.audio_engine.set_fade(fade_in_ms, fade_out_ms);
        self.streaming
            .update_config(config.providers, config.audio, config.network)
            .await;
        Ok(())
    }
}
