use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSettings {
    pub enabled: bool,
    pub credentials: Option<String>,
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            credentials: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    pub youtube: ProviderSettings,
    pub soundcloud: ProviderSettings,
    pub bandcamp: ProviderSettings,
    pub vk: ProviderSettings,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            youtube: ProviderSettings::default(),
            soundcloud: ProviderSettings::default(),
            bandcamp: ProviderSettings::default(),
            vk: ProviderSettings {
                enabled: false,
                credentials: None,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioConfig {
    pub waveform_enabled: bool,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            waveform_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkConfig {
    pub timeout_seconds: u64,
    pub chunk_size: usize,
    pub buffer_size: usize,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: 30,
            chunk_size: 1024 * 1024,      // 1MB
            buffer_size: 10 * 1024 * 1024, // 10MB
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub providers: ProviderConfig,
    pub audio: AudioConfig,
    pub network: NetworkConfig,
    #[serde(skip)]
    config_path: Option<PathBuf>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            providers: ProviderConfig::default(),
            audio: AudioConfig::default(),
            network: NetworkConfig::default(),
            config_path: None,
        }
    }
}

impl AppConfig {
    pub fn load(path: PathBuf) -> Result<Self, ConfigError> {
        if !path.exists() {
            let config = Self {
                config_path: Some(path.clone()),
                ..Default::default()
            };
            config.save()?;
            return Ok(config);
        }

        let content = fs::read_to_string(&path)?;
        let mut config: AppConfig = serde_json::from_str(&content)?;
        config.config_path = Some(path);
        Ok(config)
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        if let Some(path) = &self.config_path {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let content = serde_json::to_string_pretty(self)?;
            fs::write(path, content)?;
        }
        Ok(())
    }
}

pub struct ConfigManager {
    config: RwLock<AppConfig>,
}

impl ConfigManager {
    pub fn new(path: PathBuf) -> Result<Self, ConfigError> {
        let config = AppConfig::load(path)?;
        Ok(Self {
            config: RwLock::new(config),
        })
    }

    pub fn get(&self) -> AppConfig {
        self.config.read().unwrap().clone()
    }

    pub fn update(&self, new_config: AppConfig) -> Result<(), ConfigError> {
        let mut config = self.config.write().unwrap();
        // Preserve the path which is skipped in serialization
        let path = config.config_path.clone();
        *config = new_config;
        config.config_path = path;
        config.save()
    }
}
