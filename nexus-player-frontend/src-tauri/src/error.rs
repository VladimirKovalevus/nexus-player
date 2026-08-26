//! Structured error returned by every Tauri command.

use nexus_player_backend::streaming::error::StreamError;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandErrorCode {
    ChallengeRequired,
    Internal,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: CommandErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl CommandError {
    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            code: CommandErrorCode::Internal,
            message: message.into(),
            provider: None,
            url: None,
        }
    }
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl From<String> for CommandError {
    fn from(message: String) -> Self {
        Self::internal(message)
    }
}

impl From<&str> for CommandError {
    fn from(message: &str) -> Self {
        Self::internal(message)
    }
}

impl From<StreamError> for CommandError {
    fn from(e: StreamError) -> Self {
        match e {
            StreamError::ChallengeRequired { provider, url } => Self {
                code: CommandErrorCode::ChallengeRequired,
                message: format!("Challenge required for {provider}: {url}"),
                provider: Some(provider),
                url: Some(url),
            },
            other => Self::internal(other.to_string()),
        }
    }
}

impl From<anyhow::Error> for CommandError {
    fn from(e: anyhow::Error) -> Self {
        if let Some(StreamError::ChallengeRequired { provider, url }) =
            e.chain().find_map(|c| c.downcast_ref::<StreamError>())
        {
            return Self::from(StreamError::ChallengeRequired {
                provider: provider.clone(),
                url: url.clone(),
            });
        }
        Self::internal(format!("{e:#}"))
    }
}

impl From<tauri::Error> for CommandError {
    fn from(e: tauri::Error) -> Self {
        Self::internal(e.to_string())
    }
}

impl From<std::io::Error> for CommandError {
    fn from(e: std::io::Error) -> Self {
        Self::internal(e.to_string())
    }
}

impl From<tauri_plugin_fs::Error> for CommandError {
    fn from(e: tauri_plugin_fs::Error) -> Self {
        Self::internal(e.to_string())
    }
}
