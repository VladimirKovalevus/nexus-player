use std::path::PathBuf;
use std::sync::Arc;

use streaming::images::{ImageCache, ImageCacheConfig, ImageVariant, OnReady};

/// Cover art resolution for the UI.
///
/// Wraps the vendored [`ImageCache`] and adds the one thing it should not know
/// about: which URLs are already local and must be handed back untouched.
pub struct ArtworkCache {
    cache: ImageCache,
}

/// Where a cover should be loaded from right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtworkLocation {
    /// A local file path, ready to render.
    Local(PathBuf),
    /// Already a local URL (`asset:`, `file:`, `data:`) — pass it through as-is.
    Passthrough(String),
    /// Not cached yet; a fetch has been queued and the caller should fall back to
    /// the remote URL until an `on_ready` notification arrives.
    Pending,
}

impl ArtworkCache {
    pub fn new(root: PathBuf) -> Self {
        Self {
            cache: ImageCache::new(ImageCacheConfig::new(root)),
        }
    }

    /// Where a cover can be loaded from right now. Pure lookup — it never queues
    /// a fetch, so the caller decides whether this URL deserves one.
    pub fn lookup(&self, url: &str, variant: ImageVariant) -> ArtworkLocation {
        let url = url.trim();
        if url.is_empty() {
            return ArtworkLocation::Pending;
        }

        if is_local(url) {
            return ArtworkLocation::Passthrough(url.to_string());
        }

        match self.cache.path_if_cached(url, variant) {
            Some(path) => ArtworkLocation::Local(path),
            None => ArtworkLocation::Pending,
        }
    }

    /// Queue a background fetch at the lowest priority.
    pub fn request(&self, url: &str, variant: ImageVariant) {
        self.cache.request(url.trim(), variant);
    }

    /// Hold or release image fetching. Raised while audio is downloading, which is
    /// what keeps artwork at the lowest priority.
    pub fn set_paused(&self, paused: bool) {
        self.cache.set_paused(paused);
    }

    pub fn set_on_ready(&self, callback: Arc<OnReady>) {
        self.cache.set_on_ready(callback);
    }

    /// Jobs waiting to start. Diagnostics and tests only.
    pub fn pending_len(&self) -> usize {
        self.cache.pending_len()
    }

    pub async fn prune(&self) -> anyhow::Result<()> {
        self.cache
            .prune()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to prune artwork cache: {}", e))
    }

    pub async fn clear(&self) -> anyhow::Result<()> {
        self.cache
            .clear()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to clear artwork cache: {}", e))
    }
}

/// Covers that already live on this machine and must never be re-downloaded.
///
/// `asset:` / `asset.localhost` is what `convertFileSrc` produces — a playlist
/// cover picked from disk is stored in that form.
fn is_local(url: &str) -> bool {
    const LOCAL_PREFIXES: [&str; 6] = [
        "asset://",
        "http://asset.localhost",
        "https://asset.localhost",
        "file://",
        "data:",
        "blob:",
    ];

    let lowered = url.to_ascii_lowercase();
    LOCAL_PREFIXES
        .iter()
        .any(|prefix| lowered.starts_with(prefix))
        // A bare filesystem path, as stored by older local-artwork paths.
        || url.starts_with('/')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_local_urls() {
        assert!(is_local("asset://localhost/home/u/cover.png"));
        assert!(is_local("http://asset.localhost/home/u/cover.png"));
        assert!(is_local("file:///home/u/cover.png"));
        assert!(is_local("data:image/png;base64,iVBORw0KGgo="));
        assert!(is_local("/home/u/cover.png"));
        // Case is not significant in a scheme.
        assert!(is_local("ASSET://localhost/cover.png"));
    }

    #[test]
    fn treats_provider_urls_as_remote() {
        assert!(!is_local("https://i1.sndcdn.com/artworks-abc-large.jpg"));
        assert!(!is_local("https://f4.bcbits.com/img/a1234567890_0.jpg"));
        assert!(!is_local("http://example.com/cover.jpg"));
    }

    #[tokio::test]
    async fn local_urls_bypass_the_cache() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ArtworkCache::new(dir.path().to_path_buf());

        assert_eq!(
            cache.lookup("asset://localhost/cover.png", ImageVariant::Thumb),
            ArtworkLocation::Passthrough("asset://localhost/cover.png".to_string())
        );
    }

    #[tokio::test]
    async fn an_uncached_remote_url_is_pending() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ArtworkCache::new(dir.path().to_path_buf());
        // Paused, so the test never actually reaches the network.
        cache.set_paused(true);

        assert_eq!(
            cache.lookup("https://example.com/cover.jpg", ImageVariant::Thumb),
            ArtworkLocation::Pending
        );
        assert_eq!(cache.lookup("", ImageVariant::Thumb), ArtworkLocation::Pending);
    }

    #[tokio::test]
    async fn lookup_alone_never_queues_a_fetch() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ArtworkCache::new(dir.path().to_path_buf());
        cache.set_paused(true);

        cache.lookup("https://example.com/cover.jpg", ImageVariant::Thumb);
        assert_eq!(cache.pending_len(), 0);

        // Only an explicit request enqueues, which is what the library gate guards.
        cache.request("https://example.com/cover.jpg", ImageVariant::Thumb);
        assert_eq!(cache.pending_len(), 1);
    }
}
