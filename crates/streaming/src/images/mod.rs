//! Lowest-priority artwork cache: fetch cover art once, re-encode it small, and
//! serve it from disk forever after.
//!
//! Deliberately separate from [`crate::state::manager::DownloadManager`]. That
//! manager is a single-slot event loop dedicated to audio — it checks one task out
//! of its queue, advances it by exactly one chunk, and puts it back — so threading
//! image fetches through it would compete directly with playback prefetch. Images
//! get one worker of their own here, gated so they yield to audio entirely.
//!
//! ```no_run
//! use std::path::PathBuf;
//! use streaming::images::{ImageCache, ImageCacheConfig, ImageVariant};
//!
//! # async fn example(url: &str) {
//! let cache = ImageCache::new(ImageCacheConfig::new(PathBuf::from("/tmp/artwork")));
//!
//! match cache.path_if_cached(url, ImageVariant::Thumb) {
//!     // Hand the local file to the UI.
//!     Some(path) => println!("{}", path.display()),
//!     // Fetched in the background, whenever audio isn't using the network.
//!     None => cache.request(url, ImageVariant::Thumb),
//! }
//! # }
//! ```

mod resize;
mod store;
mod worker;

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::{watch, Notify};

use crate::error::StreamingError;
use crate::state::types::DEFAULT_USER_AGENT;
use store::Store;

/// How long a URL that failed to fetch is left alone before being retried.
const FAILURE_BACKOFF: Duration = Duration::from_secs(600);

/// Which rendition of a cover to produce.
///
/// `Thumb` covers lists, cards and the player bar; `Large` exists for the
/// full-screen visualizer and the big player control, where 256px would be
/// visibly soft.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageVariant {
    Thumb,
    Large,
}

impl ImageVariant {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Thumb => "thumb",
            Self::Large => "large",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "thumb" => Some(Self::Thumb),
            "large" => Some(Self::Large),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImageCacheConfig {
    /// Root of the on-disk cache. Created on first write.
    pub root: PathBuf,
    pub thumb_size: u32,
    pub large_size: u32,
    /// WebP quality, 0..=100.
    pub quality: f32,
    pub user_agent: Option<String>,
    /// Refuse sources larger than this, so one absurd cover can't stall the queue.
    pub max_source_bytes: u64,
    /// Budget the store is pruned back to.
    pub max_cache_bytes: u64,
    /// Idle settling time after audio traffic stops, before fetching resumes.
    pub grace: Duration,
    /// Bound on queued-but-not-started jobs. Overflow drops the oldest.
    pub queue_cap: usize,
}

impl ImageCacheConfig {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            thumb_size: 256,
            large_size: 512,
            quality: 80.0,
            user_agent: None,
            max_source_bytes: 16 * 1024 * 1024,
            max_cache_bytes: 300 * 1024 * 1024,
            grace: Duration::from_millis(400),
            queue_cap: 512,
        }
    }

    fn size_for(&self, variant: ImageVariant) -> u32 {
        match variant {
            ImageVariant::Thumb => self.thumb_size,
            ImageVariant::Large => self.large_size,
        }
    }
}

/// Called with `(url, variant, local path)` once an image has landed on disk.
pub type OnReady = dyn Fn(String, ImageVariant, PathBuf) + Send + Sync + 'static;

#[derive(Debug, Clone)]
struct Job {
    url: String,
    variant: ImageVariant,
    key: String,
}

impl Job {
    fn dedupe_key(&self) -> (ImageVariant, String) {
        (self.variant, self.key.clone())
    }
}

#[derive(Default)]
struct Queue {
    /// LIFO: `push_back` to request, `pop_back` to work. The newest request is
    /// whatever just scrolled into view, so it deserves to go first.
    pending: VecDeque<Job>,
    /// Queued *or* in flight, so a re-scroll over the same rows costs nothing.
    claimed: HashSet<(ImageVariant, String)>,
    failed: HashMap<(ImageVariant, String), Instant>,
}

pub(crate) struct Inner {
    cfg: ImageCacheConfig,
    store: Store,
    queue: Mutex<Queue>,
    wake: Notify,
    pause_tx: watch::Sender<bool>,
    on_ready: Mutex<Option<Arc<OnReady>>>,
    client: reqwest::Client,
}

impl Inner {
    /// Pull the next job, leaving its key claimed so nothing re-queues it while
    /// it is being fetched.
    fn take_next(&self) -> Option<Job> {
        self.queue.lock().ok()?.pending.pop_back()
    }

    fn release(&self, job: &Job, failed: bool) {
        if let Ok(mut queue) = self.queue.lock() {
            let key = job.dedupe_key();
            queue.claimed.remove(&key);
            if failed {
                queue.failed.insert(key, Instant::now());
            }
        }
    }

    fn notify_ready(&self, job: &Job, path: PathBuf) {
        let callback = self.on_ready.lock().ok().and_then(|cb| cb.clone());
        if let Some(callback) = callback {
            callback(job.url.clone(), job.variant, path);
        }
    }
}

/// Handle to the artwork cache. Cheap to clone; all clones share one worker.
#[derive(Clone)]
pub struct ImageCache {
    inner: Arc<Inner>,
}

impl ImageCache {
    /// Spawns the background worker, so this must be called from within a Tokio
    /// runtime.
    pub fn new(cfg: ImageCacheConfig) -> Self {
        let user_agent = cfg
            .user_agent
            .clone()
            .unwrap_or_else(|| DEFAULT_USER_AGENT.to_string());

        // `use_rustls_tls` is explicit for the same reason as the audio
        // downloaders (see `detection.rs`): SChannel's JA3 fingerprint draws
        // Cloudflare challenges. One shared client, so connections are reused
        // across the whole library instead of per-cover.
        let client = reqwest::Client::builder()
            .use_rustls_tls()
            .user_agent(user_agent)
            .timeout(Duration::from_secs(20))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let (pause_tx, _) = watch::channel(false);

        let inner = Arc::new(Inner {
            store: Store::new(cfg.root.clone()),
            cfg,
            queue: Mutex::new(Queue::default()),
            wake: Notify::new(),
            pause_tx,
            on_ready: Mutex::new(None),
            client,
        });

        tokio::spawn(worker::run(Arc::clone(&inner)));

        Self { inner }
    }

    /// The local file for `url`, if it has already been fetched and re-encoded.
    ///
    /// Purely a lookup — it never touches the network, so it is safe to call for
    /// every row of a long list.
    pub fn path_if_cached(&self, url: &str, variant: ImageVariant) -> Option<PathBuf> {
        if url.is_empty() {
            return None;
        }
        let key = self.inner.store.key(url);
        self.inner.store.path_if_present(&key, variant)
    }

    /// Queue `url` for background fetch at the lowest priority.
    ///
    /// Idempotent: already cached, already queued, already in flight, or recently
    /// failed all return without doing anything.
    pub fn request(&self, url: &str, variant: ImageVariant) {
        if url.is_empty() {
            return;
        }

        let key = self.inner.store.key(url);
        if self.inner.store.path_if_present(&key, variant).is_some() {
            return;
        }

        let job = Job {
            url: url.to_string(),
            variant,
            key,
        };
        let dedupe_key = job.dedupe_key();

        {
            let Ok(mut queue) = self.inner.queue.lock() else {
                return;
            };

            if queue.claimed.contains(&dedupe_key) {
                return;
            }
            match queue.failed.get(&dedupe_key) {
                Some(at) if at.elapsed() < FAILURE_BACKOFF => return,
                Some(_) => {
                    queue.failed.remove(&dedupe_key);
                }
                None => {}
            }

            // Shed the oldest request rather than growing without bound — it is
            // the one furthest from the viewport.
            while queue.pending.len() >= self.inner.cfg.queue_cap {
                match queue.pending.pop_front() {
                    Some(stale) => {
                        queue.claimed.remove(&stale.dedupe_key());
                    }
                    None => break,
                }
            }

            queue.claimed.insert(dedupe_key);
            queue.pending.push_back(job);
        }

        self.inner.wake.notify_one();
    }

    /// Hold or release image fetching.
    ///
    /// This is what makes the cache lowest-priority: the caller raises it while
    /// audio is downloading, and no image request is issued until it drops.
    pub fn set_paused(&self, paused: bool) {
        // `send_replace` rather than `send`: the worker subscribes when its task
        // first polls, so until then the channel has no receivers and `send`
        // would drop the update on the floor.
        self.inner.pause_tx.send_replace(paused);
    }

    pub fn is_paused(&self) -> bool {
        *self.inner.pause_tx.borrow()
    }

    pub fn set_on_ready(&self, callback: Arc<OnReady>) {
        if let Ok(mut slot) = self.inner.on_ready.lock() {
            *slot = Some(callback);
        }
    }

    /// Number of jobs waiting to start. Diagnostics only.
    pub fn pending_len(&self) -> usize {
        self.inner
            .queue
            .lock()
            .map(|queue| queue.pending.len())
            .unwrap_or(0)
    }

    /// Trim the store back to `max_cache_bytes`, oldest first.
    pub async fn prune(&self) -> Result<(), StreamingError> {
        self.inner.store.prune(self.inner.cfg.max_cache_bytes).await
    }

    pub async fn clear(&self) -> Result<(), StreamingError> {
        if let Ok(mut queue) = self.inner.queue.lock() {
            queue.pending.clear();
            queue.claimed.clear();
            queue.failed.clear();
        }
        self.inner.store.clear().await
    }
}
