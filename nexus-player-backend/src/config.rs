use crate::audio::eq::EqSettings;
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
#[serde(rename_all = "camelCase", default)]
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
#[serde(rename_all = "camelCase", default)]
pub struct ProviderConfig {
    pub soundcloud: ProviderSettings,
    pub bandcamp: ProviderSettings,
    pub vk: ProviderSettings,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
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
#[serde(rename_all = "camelCase", default)]
pub struct AudioConfig {
    pub waveform_enabled: bool,
    pub mem_limit: u64,
    /// Fraction (0.0–1.0) of each upcoming track to pre-download before fully downloading any one.
    /// Default: 0.2 (20%).
    pub prefetch_limit: f64,
    pub fade_in_enabled: bool,
    pub fade_out_enabled: bool,
    /// Duration of fade-in at the start of a track, in milliseconds.
    pub fade_in_ms: u32,
    /// Duration of fade-out at the end of a track, in milliseconds.
    pub fade_out_ms: u32,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            waveform_enabled: true,
            mem_limit: 100 * 1024 * 1024,
            prefetch_limit: 0.2,
            fade_in_enabled: false,
            fade_out_enabled: false,
            fade_in_ms: 500,
            fade_out_ms: 500,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NetworkConfig {
    pub timeout_seconds: u64,
    pub chunk_size: usize,
    pub buffer_size: usize,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: 30,
            chunk_size: 1024 * 1024,       // 1MB
            buffer_size: 10 * 1024 * 1024, // 10MB
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub providers: ProviderConfig,
    pub audio: AudioConfig,
    pub network: NetworkConfig,
    pub eq: EqSettings,
    #[serde(skip)]
    config_path: Option<PathBuf>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            providers: ProviderConfig::default(),
            audio: AudioConfig::default(),
            network: NetworkConfig::default(),
            eq: EqSettings::default(),
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
