//! The Telegram bot token lives in a secret backend (OS keyring) or a private
//! app-data file, and migrates once out of the old plain JSON config.
//!
//! Expected API (module `sync::telegram::token_store`):
//! - `trait TokenStore: Send + Sync { load() -> Result<Option<String>>; save(&str) -> Result<()>; delete() -> Result<()> }`
//! - `trait SecretBackend: Send + Sync` with the same three methods (the keyring wrapper)
//! - `FileTokenStore::new(path)` implements `TokenStore`
//! - `FallbackTokenStore::new(Box<dyn SecretBackend>, FileTokenStore)` implements `TokenStore`
//! - `TelegramConfig::load_with_store(path, &dyn TokenStore) -> Result<Option<TelegramConfig>>`
//!   migrates a legacy `bot_token` from the JSON into the store.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use nexus_player_backend::sync::telegram::token_store::{
    FallbackTokenStore, FileTokenStore, SecretBackend, TokenStore,
};
use nexus_player_backend::sync::TelegramConfig;

const TOKEN: &str = "123456:FAKE-secret-token-XYZ";

#[derive(Clone, Default)]
struct FakeKeyring {
    slot: Arc<Mutex<Option<String>>>,
}

impl SecretBackend for FakeKeyring {
    fn load(&self) -> Result<Option<String>> {
        Ok(self.slot.lock().expect("lock").clone())
    }
    fn save(&self, token: &str) -> Result<()> {
        *self.slot.lock().expect("lock") = Some(token.to_string());
        Ok(())
    }
    fn delete(&self) -> Result<()> {
        *self.slot.lock().expect("lock") = None;
        Ok(())
    }
}

/// Behaves like a keyring with no secret service running.
struct UnavailableKeyring;

impl SecretBackend for UnavailableKeyring {
    fn load(&self) -> Result<Option<String>> {
        Err(anyhow!("no secret service"))
    }
    fn save(&self, _token: &str) -> Result<()> {
        Err(anyhow!("no secret service"))
    }
    fn delete(&self) -> Result<()> {
        Err(anyhow!("no secret service"))
    }
}

/// A store whose writes always fail, counting attempts.
#[derive(Default)]
struct BrokenStore {
    save_attempts: AtomicUsize,
}

impl TokenStore for BrokenStore {
    fn load(&self) -> Result<Option<String>> {
        Ok(None)
    }
    fn save(&self, _token: &str) -> Result<()> {
        self.save_attempts.fetch_add(1, Ordering::SeqCst);
        Err(anyhow!("disk full"))
    }
    fn delete(&self) -> Result<()> {
        Ok(())
    }
}

/// Counts successful saves on top of an inner store.
struct CountingStore<S: TokenStore> {
    inner: S,
    saves: AtomicUsize,
}

impl<S: TokenStore> TokenStore for CountingStore<S> {
    fn load(&self) -> Result<Option<String>> {
        self.inner.load()
    }
    fn save(&self, token: &str) -> Result<()> {
        self.saves.fetch_add(1, Ordering::SeqCst);
        self.inner.save(token)
    }
    fn delete(&self) -> Result<()> {
        self.inner.delete()
    }
}

fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).expect("read dir") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            out.extend(files_under(&path));
        } else {
            out.push(path);
        }
    }
    out
}

fn files_containing_token(dir: &Path) -> Vec<PathBuf> {
    files_under(dir)
        .into_iter()
        .filter(|p| {
            std::fs::read(p)
                .map(|b| String::from_utf8_lossy(&b).contains(TOKEN))
                .unwrap_or(false)
        })
        .collect()
}

fn write_legacy_config(path: &Path) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    let json = format!(
        r#"{{"bot_token":"{TOKEN}","chat_id":"-100777","last_message_id":42}}"#
    );
    std::fs::write(path, json).expect("write legacy config");
}

#[test]
fn keyring_save_then_load_returns_token_and_writes_no_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FallbackTokenStore::new(
        Box::new(FakeKeyring::default()),
        FileTokenStore::new(dir.path().join("sync/telegram_token")),
    );
    store.save(TOKEN).expect("save");
    assert_eq!(store.load().expect("load").as_deref(), Some(TOKEN));
    assert!(
        files_containing_token(dir.path()).is_empty(),
        "token must not be written to disk when the keyring works"
    );
}

#[test]
fn load_with_nothing_stored_returns_none() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FallbackTokenStore::new(
        Box::new(FakeKeyring::default()),
        FileTokenStore::new(dir.path().join("sync/telegram_token")),
    );
    assert_eq!(store.load().expect("load"), None);
}

#[test]
fn unavailable_keyring_falls_back_to_file_and_reads_it_back() {
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("sync/telegram_token");
    let store = FallbackTokenStore::new(
        Box::new(UnavailableKeyring),
        FileTokenStore::new(&file),
    );
    store.save(TOKEN).expect("save falls back to file");
    assert!(file.exists(), "fallback file should be created");
    assert!(std::fs::read_to_string(&file)
        .expect("read")
        .contains(TOKEN));
    assert_eq!(store.load().expect("load").as_deref(), Some(TOKEN));
}

#[cfg(unix)]
#[test]
fn fallback_file_is_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("sync/telegram_token");
    let store = FileTokenStore::new(&file);
    store.save(TOKEN).expect("save");
    let mode = std::fs::metadata(&file).expect("meta").permissions().mode();
    assert_eq!(mode & 0o077, 0, "group/other bits must be clear: {mode:o}");
}

#[test]
fn file_store_round_trips_on_its_own() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FileTokenStore::new(dir.path().join("sync/telegram_token"));
    assert_eq!(store.load().expect("load empty"), None);
    store.save(TOKEN).expect("save");
    assert_eq!(store.load().expect("load").as_deref(), Some(TOKEN));
    store.delete().expect("delete");
    assert_eq!(store.load().expect("load after delete"), None);
}

#[test]
fn migration_moves_token_into_store_and_strips_json() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config_path = dir.path().join("sync/telegram_config.json");
    write_legacy_config(&config_path);
    let store = FileTokenStore::new(dir.path().join("sync/telegram_token"));

    let config = TelegramConfig::load_with_store(&config_path, &store)
        .expect("load")
        .expect("config present");

    assert_eq!(config.chat_id, "-100777");
    assert_eq!(config.last_message_id, Some(42));
    assert_eq!(store.load().expect("load token").as_deref(), Some(TOKEN));
    let json = std::fs::read_to_string(&config_path).expect("read json");
    assert!(!json.contains(TOKEN), "JSON still has token: {json}");
    assert!(json.contains("-100777"), "chat id must stay: {json}");
    assert!(json.contains("42"), "last message id must stay: {json}");
}

#[test]
fn migration_with_working_keyring_leaves_token_only_in_keyring() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config_path = dir.path().join("sync/telegram_config.json");
    write_legacy_config(&config_path);
    let store = FallbackTokenStore::new(
        Box::new(FakeKeyring::default()),
        FileTokenStore::new(dir.path().join("sync/telegram_token")),
    );
    TelegramConfig::load_with_store(&config_path, &store).expect("load");
    assert_eq!(store.load().expect("load").as_deref(), Some(TOKEN));
    assert!(files_containing_token(dir.path()).is_empty());
}

#[test]
fn failed_store_write_keeps_token_in_json_for_retry() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config_path = dir.path().join("sync/telegram_config.json");
    write_legacy_config(&config_path);
    let store = BrokenStore::default();

    // Whether load reports the failure or tolerates it, the JSON must keep the token.
    let _ = TelegramConfig::load_with_store(&config_path, &store);

    assert!(store.save_attempts.load(Ordering::SeqCst) >= 1);
    let json = std::fs::read_to_string(&config_path).expect("read json");
    assert!(json.contains(TOKEN), "token lost before store write: {json}");

    // Next start with a working store succeeds.
    let good = FileTokenStore::new(dir.path().join("sync/telegram_token"));
    TelegramConfig::load_with_store(&config_path, &good).expect("retry");
    assert_eq!(good.load().expect("load").as_deref(), Some(TOKEN));
    let json = std::fs::read_to_string(&config_path).expect("read json");
    assert!(!json.contains(TOKEN));
}

#[test]
fn loading_twice_migrates_once() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config_path = dir.path().join("sync/telegram_config.json");
    write_legacy_config(&config_path);
    let store = CountingStore {
        inner: FileTokenStore::new(dir.path().join("sync/telegram_token")),
        saves: AtomicUsize::new(0),
    };

    TelegramConfig::load_with_store(&config_path, &store).expect("first load");
    let second = TelegramConfig::load_with_store(&config_path, &store)
        .expect("second load")
        .expect("config present");

    assert_eq!(store.saves.load(Ordering::SeqCst), 1);
    assert_eq!(second.chat_id, "-100777");
    assert_eq!(store.load().expect("load").as_deref(), Some(TOKEN));
}

#[test]
fn config_without_legacy_token_does_not_touch_the_store() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config_path = dir.path().join("sync/telegram_config.json");
    std::fs::create_dir_all(config_path.parent().expect("parent")).expect("mkdir");
    std::fs::write(&config_path, r#"{"chat_id":"-100777"}"#).expect("write");
    let store = BrokenStore::default();

    let config = TelegramConfig::load_with_store(&config_path, &store)
        .expect("load")
        .expect("config present");

    assert_eq!(config.chat_id, "-100777");
    assert_eq!(store.save_attempts.load(Ordering::SeqCst), 0);
}
