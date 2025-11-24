use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use super::error::{StreamError, StreamResult};

#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub path: PathBuf,
    pub original_url: String,
    pub size: u64,
    pub created_at: u64,
    pub last_accessed: u64,
}

pub struct CacheManager {
    cache_dir: PathBuf,
    max_size: u64,
    entries: Arc<RwLock<HashMap<String, CacheEntry>>>,
}

impl CacheManager {
    pub fn new(cache_dir: PathBuf, max_size_mb: u64) -> StreamResult<Self> {
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir).map_err(|e| StreamError::IoError(e))?;
        }

        let manager = Self {
            cache_dir,
            max_size: max_size_mb * 1024 * 1024,
            entries: Arc::new(RwLock::new(HashMap::new())),
        };

        let _ = manager.load_existing_cache();

        Ok(manager)
    }

    async fn load_existing_cache(&self) -> StreamResult<()> {
        let entries_dir = fs::read_dir(&self.cache_dir).map_err(|e| StreamError::IoError(e))?;

        let mut cache_entries = HashMap::new();
        let now = Self::current_timestamp();

        for entry in entries_dir {
            let entry = entry.map_err(|e| StreamError::IoError(e))?;
            let path = entry.path();

            if path.is_file() {
                if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                    let metadata = fs::metadata(&path).map_err(|e| StreamError::IoError(e))?;

                    let size = metadata.len();
                    let created = metadata
                        .created()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(now);

                    cache_entries.insert(
                        filename.to_string(),
                        CacheEntry {
                            path: path.clone(),
                            original_url: format!("cached:{}", filename),
                            size,
                            created_at: created,
                            last_accessed: now,
                        },
                    );
                }
            }
        }

        *self.entries.write().unwrap() = cache_entries;

        Ok(())
    }

    fn compute_key(url: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        format!("{:x}.mp3", hasher.finalize())
    }

    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    pub async fn get(&self, url: &str) -> Option<PathBuf> {
        let key = Self::compute_key(url);
        let mut entries = self.entries.write().unwrap();

        if let Some(entry) = entries.get_mut(&key) {
            if entry.path.exists() {
                entry.last_accessed = Self::current_timestamp();
                return Some(entry.path.clone());
            } else {
                entries.remove(&key);
            }
        }

        None
    }

    pub async fn reserve(&self, url: &str) -> StreamResult<PathBuf> {
        let key = Self::compute_key(url);
        let cache_path = self.cache_dir.join(&key);

        Ok(cache_path)
    }

    pub async fn put(&self, url: &str, size: u64) -> StreamResult<()> {
        let key = Self::compute_key(url);
        let cache_path = self.cache_dir.join(&key);

        if !cache_path.exists() {
            return Err(StreamError::IoError(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Cache file not found",
            )));
        }

        let now = Self::current_timestamp();
        let entry = CacheEntry {
            path: cache_path,
            original_url: url.to_string(),
            size,
            created_at: now,
            last_accessed: now,
        };

        let mut entries = self.entries.write().unwrap();
        entries.insert(key, entry);

        self.enforce_size_limit_locked(&entries);

        Ok(())
    }

    fn enforce_size_limit_locked(&self, entries: &HashMap<String, CacheEntry>) -> StreamResult<()> {
        let total_size: u64 = entries.values().map(|e| e.size).sum();

        if total_size <= self.max_size {
            return Ok(());
        }

        let mut sorted_entries: Vec<_> = entries.values().collect();
        sorted_entries.sort_by_key(|e| e.last_accessed);

        let mut to_remove = Vec::new();
        let mut removed_size = 0u64;
        let target_removal = total_size - self.max_size;

        for entry in sorted_entries {
            if removed_size >= target_removal {
                break;
            }

            to_remove.push(entry.path.clone());
            removed_size += entry.size;
        }

        for path in to_remove {
            if let Err(e) = fs::remove_file(&path) {
                tracing::warn!("Failed to remove cache file {:?}: {}", path, e);
            }

            let mut entries = self.entries.write().unwrap();
            entries.retain(|_, e| e.path != path);
        }

        Ok(())
    }

    pub async fn cleanup(&self) -> StreamResult<()> {
        let entries = self.entries.read().unwrap();
        self.enforce_size_limit_locked(&entries)
    }

    pub async fn total_size(&self) -> u64 {
        let entries = self.entries.read().unwrap();
        entries.values().map(|e| e.size).sum()
    }

    pub async fn entry_count(&self) -> usize {
        let entries = self.entries.read().unwrap();
        entries.len()
    }
}
