pub mod client;
pub mod config;

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

use client::TelegramClient;
pub use config::TelegramConfig;

use super::types::SyncFile;

pub struct TelegramSync {
    client: TelegramClient,
    config: TelegramConfig,
    config_path: PathBuf,
}

impl TelegramSync {
    pub fn new(config: TelegramConfig, config_path: impl Into<PathBuf>) -> Result<Self> {
        let client = TelegramClient::new(&config.bot_token)?;
        Ok(Self {
            client,
            config,
            config_path: config_path.into(),
        })
    }

    pub fn load(config_path: impl Into<PathBuf>) -> Result<Option<Self>> {
        let path = config_path.into();
        match TelegramConfig::load(&path)? {
            Some(config) => Ok(Some(Self::new(config, path)?)),
            None => Ok(None),
        }
    }

    /// Upload sync file to Telegram, pin it, and remove the previous message.
    pub async fn push(&mut self, file: &SyncFile) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(file)?;
        let filename = "nexus_sync.json";

        // Delete previous sync message if we know its ID
        if let Some(old_id) = self.config.last_message_id {
            if let Err(e) = self.client.delete_message(&self.config.chat_id, old_id).await {
                tracing::warn!("Could not delete old sync message {}: {}", old_id, e);
            }
        }

        // Send new file
        let msg = self
            .client
            .send_document(&self.config.chat_id, bytes, filename)
            .await?;

        // Pin it (suppress notification)
        self.client
            .pin_message(&self.config.chat_id, msg.message_id)
            .await?;

        // Persist the new message ID
        self.config.last_message_id = Some(msg.message_id);
        self.config.save(&self.config_path)?;

        tracing::info!("Telegram sync pushed, message_id={}", msg.message_id);
        Ok(())
    }

    /// Download the pinned sync file from the channel and parse it.
    pub async fn pull(&self) -> Result<SyncFile> {
        let chat = self.client.get_chat(&self.config.chat_id).await?;

        let pinned = chat
            .pinned_message
            .ok_or_else(|| anyhow!("No pinned message in the Telegram chat"))?;

        let doc = pinned
            .document
            .ok_or_else(|| anyhow!("Pinned message is not a document"))?;

        let tg_file = self.client.get_file(&doc.file_id).await?;

        let file_path = tg_file
            .file_path
            .ok_or_else(|| anyhow!("Telegram file has no file_path"))?;

        let bytes = self.client.download_file(&file_path).await?;

        let sync_file: SyncFile = serde_json::from_slice(&bytes)
            .map_err(|e| anyhow!("Failed to parse sync file: {}", e))?;

        tracing::info!(
            "Telegram sync pulled, sync_type={:?}, tracks={}, playlists={}",
            sync_file.sync_type,
            sync_file.tracks.len(),
            sync_file.playlists.len()
        );

        Ok(sync_file)
    }

    /// Verify that the bot token is valid and has access to the chat.
    pub async fn check(&self) -> bool {
        self.client.get_chat(&self.config.chat_id).await.is_ok()
    }
}
