use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::SystemTime;

use sha2::{Digest, Sha256};

use super::ImageVariant;
use crate::error::StreamingError;

/// Subdirectory of the artwork root holding covers the user picked from disk.
pub const CUSTOM_DIR: &str = "custom";

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// On-disk artwork store, laid out as `<root>/<variant>/<ab>/<sha256(url)>.webp`.
///
/// Keyed on the *original* provider URL — the same string already persisted in
/// `artworks_v2` — so nothing in the database has to change and existing rows
/// resolve without a migration. The two-hex-char fan-out keeps directories small
/// enough for a library of any size.
pub(crate) struct Store {
    root: PathBuf,
    /// Keys known to be on disk, so repeated resolves of a visible list don't
    /// re-`stat` the same files on every scroll.
    known: Mutex<HashSet<(ImageVariant, String)>>,
}

impl Store {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self {
            root,
            known: Mutex::new(HashSet::new()),
        }
    }

    pub(crate) fn key(&self, url: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn path(&self, key: &str, variant: ImageVariant) -> PathBuf {
        self.root
            .join(variant.as_str())
            .join(&key[..2])
            .join(format!("{key}.webp"))
    }

    pub(crate) fn path_if_present(&self, key: &str, variant: ImageVariant) -> Option<PathBuf> {
        let entry = (variant, key.to_string());

        if self.known.lock().ok()?.contains(&entry) {
            return Some(self.path(key, variant));
        }

        let path = self.path(key, variant);
        if path.is_file() {
            if let Ok(mut known) = self.known.lock() {
                known.insert(entry);
            }
            Some(path)
        } else {
            None
        }
    }

    /// Write via a unique temp file plus `rename`, so a reader can never observe
    /// a half-written image (the asset protocol serves these straight to the webview).
    pub(crate) async fn write(
        &self,
        key: &str,
        variant: ImageVariant,
        data: &[u8],
    ) -> Result<PathBuf, StreamingError> {
        let path = self.path(key, variant);
        let dir = path
            .parent()
            .ok_or_else(|| StreamingError::Io(format!("no parent for {}", path.display())))?;

        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|e| StreamingError::Io(e.to_string()))?;

        let tmp = dir.join(format!(
            "{key}.{}.{}.tmp",
            std::process::id(),
            TMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));

        tokio::fs::write(&tmp, data)
            .await
            .map_err(|e| StreamingError::Io(e.to_string()))?;

        if let Err(e) = tokio::fs::rename(&tmp, &path).await {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(StreamingError::Io(e.to_string()));
        }

        if let Ok(mut known) = self.known.lock() {
            known.insert((variant, key.to_string()));
        }

        Ok(path)
    }

    /// Drop the oldest images until the store fits in `max_bytes`.
    ///
    /// Age is taken from mtime, which `write` refreshes, so a cover that keeps
    /// being re-fetched keeps its place. Cheap enough to run once on startup.
    pub(crate) async fn prune(&self, max_bytes: u64) -> Result<(), StreamingError> {
        let mut files: Vec<(PathBuf, u64, SystemTime)> = Vec::new();
        let mut total: u64 = 0;

        let mut dirs = vec![self.root.clone()];
        while let Some(dir) = dirs.pop() {
            let mut entries = match tokio::fs::read_dir(&dir).await {
                Ok(entries) => entries,
                // A cache directory that doesn't exist yet is simply empty.
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(StreamingError::Io(e.to_string())),
            };

            while let Some(entry) = entries
                .next_entry()
                .await
                .map_err(|e| StreamingError::Io(e.to_string()))?
            {
                let path = entry.path();
                let meta = match entry.metadata().await {
                    Ok(meta) => meta,
                    Err(_) => continue,
                };

                if meta.is_dir() {
                    // User-picked covers live here and are never evicted.
                    if dir == self.root && path.file_name().is_some_and(|n| n == CUSTOM_DIR) {
                        continue;
                    }
                    dirs.push(path);
                } else if path.extension().is_some_and(|ext| ext == "webp") {
                    let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                    total += meta.len();
                    files.push((path, meta.len(), modified));
                } else if path.extension().is_some_and(|ext| ext == "tmp") {
                    // Leftover from a write interrupted by a crash.
                    let _ = tokio::fs::remove_file(&path).await;
                }
            }
        }

        if total <= max_bytes {
            return Ok(());
        }

        files.sort_by_key(|(_, _, modified)| *modified);

        let mut evicted = 0usize;
        for (path, size, _) in files {
            if total <= max_bytes {
                break;
            }
            if tokio::fs::remove_file(&path).await.is_ok() {
                total = total.saturating_sub(size);
                evicted += 1;
                self.forget(&path);
            }
        }

        tracing::debug!(
            evicted,
            remaining_bytes = total,
            "pruned artwork cache to fit budget"
        );

        Ok(())
    }

    pub(crate) async fn clear(&self) -> Result<(), StreamingError> {
        let mut entries = match tokio::fs::read_dir(&self.root).await {
            Ok(entries) => Some(entries),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(StreamingError::Io(e.to_string())),
        };
        while let Some(entries) = entries.as_mut() {
            let Some(entry) = entries
                .next_entry()
                .await
                .map_err(|e| StreamingError::Io(e.to_string()))?
            else {
                break;
            };
            let path = entry.path();
            // User-picked covers live here and are never cleared.
            if path.file_name().is_some_and(|n| n == CUSTOM_DIR) {
                continue;
            }
            let is_dir = entry.metadata().await.map(|m| m.is_dir()).unwrap_or(false);
            let result = if is_dir {
                tokio::fs::remove_dir_all(&path).await
            } else {
                tokio::fs::remove_file(&path).await
            };
            match result {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(StreamingError::Io(e.to_string())),
            }
        }

        if let Ok(mut known) = self.known.lock() {
            known.clear();
        }

        Ok(())
    }

    /// Drop a pruned file from the `known` set, so a later resolve re-`stat`s it
    /// (and re-queues a fetch) instead of handing out a path that no longer exists.
    fn forget(&self, path: &Path) {
        let Some(key) = path.file_stem().and_then(|s| s.to_str()) else {
            return;
        };
        let variant = path
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .and_then(ImageVariant::parse);

        if let (Some(variant), Ok(mut known)) = (variant, self.known.lock()) {
            known.remove(&(variant, key.to_string()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (Store, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        (Store::new(dir.path().join("artwork")), dir)
    }

    #[test]
    fn key_is_stable_and_url_specific() {
        let (store, _guard) = store();
        let a = store.key("https://example.com/cover.jpg");
        let b = store.key("https://example.com/cover.jpg");
        let c = store.key("https://example.com/other.jpg");

        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn variants_of_one_url_do_not_collide() {
        let (store, _guard) = store();
        let key = store.key("https://example.com/cover.jpg");

        assert_ne!(
            store.path(&key, ImageVariant::Thumb),
            store.path(&key, ImageVariant::Large)
        );
    }

    #[tokio::test]
    async fn write_then_resolve_leaves_no_temp_file() {
        let (store, _guard) = store();
        let key = store.key("https://example.com/cover.jpg");

        assert!(store.path_if_present(&key, ImageVariant::Thumb).is_none());

        let path = store
            .write(&key, ImageVariant::Thumb, b"webp-bytes")
            .await
            .unwrap();

        assert_eq!(tokio::fs::read(&path).await.unwrap(), b"webp-bytes");
        assert_eq!(
            store.path_if_present(&key, ImageVariant::Thumb),
            Some(path.clone())
        );
        // The other variant is still absent.
        assert!(store.path_if_present(&key, ImageVariant::Large).is_none());

        let mut entries = tokio::fs::read_dir(path.parent().unwrap()).await.unwrap();
        while let Some(entry) = entries.next_entry().await.unwrap() {
            assert_ne!(entry.path().extension().unwrap(), "tmp");
        }
    }

    #[tokio::test]
    async fn prune_evicts_oldest_first_and_forgets_them() {
        let (store, _guard) = store();
        let keys: Vec<String> = (0..4).map(|i| store.key(&format!("url-{i}"))).collect();

        for key in &keys {
            store
                .write(key, ImageVariant::Thumb, &vec![0u8; 1000])
                .await
                .unwrap();
            // Keep mtimes distinguishable on coarse-grained filesystems.
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }

        // Budget for two of the four files.
        store.prune(2100).await.unwrap();

        assert!(store
            .path_if_present(&keys[0], ImageVariant::Thumb)
            .is_none());
        assert!(store
            .path_if_present(&keys[1], ImageVariant::Thumb)
            .is_none());
        assert!(store
            .path_if_present(&keys[3], ImageVariant::Thumb)
            .is_some());
    }

    #[tokio::test]
    async fn prune_is_a_noop_under_budget_and_on_a_missing_root() {
        let (store, _guard) = store();
        store.prune(1024).await.unwrap();

        let key = store.key("url");
        store
            .write(&key, ImageVariant::Thumb, &[0u8; 10])
            .await
            .unwrap();
        store.prune(1024).await.unwrap();

        assert!(store.path_if_present(&key, ImageVariant::Thumb).is_some());
    }

    #[tokio::test]
    async fn prune_sweeps_abandoned_temp_files() {
        let (store, _guard) = store();
        let key = store.key("url");
        let path = store
            .write(&key, ImageVariant::Thumb, &[0u8; 10])
            .await
            .unwrap();

        let stray = path.parent().unwrap().join("leftover.tmp");
        tokio::fs::write(&stray, b"partial").await.unwrap();

        store.prune(u64::MAX).await.unwrap();

        assert!(!stray.exists());
    }

    #[tokio::test]
    async fn prune_and_clear_keep_custom_covers() {
        let (store, dir) = store();
        let custom = dir.path().join("artwork").join(CUSTOM_DIR);
        tokio::fs::create_dir_all(&custom).await.unwrap();
        let cover = custom.join("abc.webp");
        tokio::fs::write(&cover, vec![0u8; 5000]).await.unwrap();

        store.prune(0).await.unwrap();
        assert!(cover.exists());
        store.clear().await.unwrap();
        assert!(cover.exists());
    }

    #[tokio::test]
    async fn clear_removes_everything() {
        let (store, _guard) = store();
        let key = store.key("url");
        store
            .write(&key, ImageVariant::Thumb, b"data")
            .await
            .unwrap();

        store.clear().await.unwrap();

        assert!(store.path_if_present(&key, ImageVariant::Thumb).is_none());
        // Clearing an already-absent store is fine.
        store.clear().await.unwrap();
    }
}
