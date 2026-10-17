//! Storage for the Telegram bot token: OS keyring on desktop, with a private
//! app-data file as fallback (and as the only store on Android).

use std::path::{Path, PathBuf};

use anyhow::Result;

pub trait TokenStore: Send + Sync {
    fn load(&self) -> Result<Option<String>>;
    fn save(&self, token: &str) -> Result<()>;
    fn delete(&self) -> Result<()>;
}

/// A secret backend such as the OS keyring. Same shape as [`TokenStore`];
/// `load` returns `Ok(None)` when there is no entry and `Err` when the
/// backend itself is unavailable.
pub trait SecretBackend: Send + Sync {
    fn load(&self) -> Result<Option<String>>;
    fn save(&self, token: &str) -> Result<()>;
    fn delete(&self) -> Result<()>;
}

/// Token kept in a file readable only by the owner (on Unix).
pub struct FileTokenStore {
    path: PathBuf,
}

impl FileTokenStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl TokenStore for FileTokenStore {
    fn load(&self) -> Result<Option<String>> {
        match std::fs::read_to_string(&self.path) {
            Ok(s) => {
                let t = s.trim();
                Ok(if t.is_empty() {
                    None
                } else {
                    Some(t.to_string())
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn save(&self, token: &str) -> Result<()> {
        use std::io::Write;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut file = opts.open(&self.path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        file.write_all(token.as_bytes())?;
        Ok(())
    }

    fn delete(&self) -> Result<()> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

/// Prefers the secret backend; any backend failure falls back to the file.
pub struct FallbackTokenStore {
    backend: Box<dyn SecretBackend>,
    file: FileTokenStore,
}

impl FallbackTokenStore {
    pub fn new(backend: Box<dyn SecretBackend>, file: FileTokenStore) -> Self {
        Self { backend, file }
    }
}

impl TokenStore for FallbackTokenStore {
    fn load(&self) -> Result<Option<String>> {
        match self.backend.load() {
            Ok(Some(t)) => Ok(Some(t)),
            Ok(None) => self.file.load(),
            Err(e) => {
                tracing::warn!("Keyring unavailable, reading Telegram token from file: {e}");
                self.file.load()
            }
        }
    }

    fn save(&self, token: &str) -> Result<()> {
        match self.backend.save(token) {
            Ok(()) => {
                // Do not leave a stale copy on disk.
                self.file.delete()
            }
            Err(e) => {
                tracing::warn!("Keyring unavailable, storing Telegram token in app-data file: {e}");
                self.file.save(token)
            }
        }
    }

    fn delete(&self) -> Result<()> {
        if let Err(e) = self.backend.delete() {
            tracing::warn!("Keyring unavailable while deleting Telegram token: {e}");
        }
        self.file.delete()
    }
}

#[cfg(not(target_os = "android"))]
pub struct KeyringBackend {
    service: String,
    user: String,
}

#[cfg(not(target_os = "android"))]
impl KeyringBackend {
    pub fn new() -> Self {
        Self {
            service: "nexus-player".to_string(),
            user: "telegram-bot-token".to_string(),
        }
    }

    fn entry(&self) -> Result<keyring::Entry> {
        Ok(keyring::Entry::new(&self.service, &self.user)?)
    }
}

#[cfg(not(target_os = "android"))]
impl Default for KeyringBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_os = "android"))]
impl SecretBackend for KeyringBackend {
    fn load(&self) -> Result<Option<String>> {
        match self.entry()?.get_password() {
            Ok(t) => Ok(Some(t)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn save(&self, token: &str) -> Result<()> {
        Ok(self.entry()?.set_password(token)?)
    }

    fn delete(&self) -> Result<()> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

/// Default store for the platform: keyring + file fallback on desktop,
/// the app-data file on Android.
pub fn default_token_store(sync_dir: &Path) -> Box<dyn TokenStore> {
    let file = FileTokenStore::new(sync_dir.join("telegram_token"));
    #[cfg(not(target_os = "android"))]
    {
        Box::new(FallbackTokenStore::new(
            Box::new(KeyringBackend::new()),
            file,
        ))
    }
    #[cfg(target_os = "android")]
    {
        Box::new(file)
    }
}
