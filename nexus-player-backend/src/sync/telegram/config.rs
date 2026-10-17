use std::path::Path;

use anyhow::Result;

use super::token_store::TokenStore;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    /// Never written to the JSON; filled from the token store on load.
    #[serde(default, skip_serializing)]
    pub bot_token: String,
    pub chat_id: String,
    #[serde(default)]
    pub last_message_id: Option<i64>,
}

impl std::fmt::Debug for TelegramConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TelegramConfig")
            .field("bot_token", &"***")
            .field("chat_id", &self.chat_id)
            .field("last_message_id", &self.last_message_id)
            .finish()
    }
}

impl TelegramConfig {
    pub fn load(path: &Path) -> Result<Option<Self>> {
        if !path.exists() {
            return Ok(None);
        }
        let s = std::fs::read_to_string(path)?;
        Ok(Some(serde_json::from_str(&s)?))
    }

    /// Load the config and the token from `store`. A legacy `bot_token` found
    /// in the JSON is moved into the store; the JSON is rewritten without it
    /// only after the store write succeeded.
    pub fn load_with_store(path: &Path, store: &dyn TokenStore) -> Result<Option<Self>> {
        let Some(mut config) = Self::load(path)? else {
            return Ok(None);
        };
        if config.bot_token.is_empty() {
            config.bot_token = store.load()?.unwrap_or_default();
            return Ok(Some(config));
        }
        match store.save(&config.bot_token) {
            Ok(()) => config.save(path)?,
            Err(e) => tracing::warn!("Could not migrate Telegram token to secure storage: {e}"),
        }
        Ok(Some(config))
    }

    /// Store the token in `store` and write the rest of the config to JSON.
    pub fn save_with_store(&self, path: &Path, store: &dyn TokenStore) -> Result<()> {
        store.save(&self.bot_token)?;
        self.save(path)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}
