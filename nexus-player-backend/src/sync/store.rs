use std::path::{Path, PathBuf};

use anyhow::Result;

use super::types::{SyncFile, SyncType};

pub struct SyncStore {
    dir: PathBuf,
}

impl SyncStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn save(&self, file: &SyncFile) -> Result<PathBuf> {
        std::fs::create_dir_all(&self.dir)?;

        let filename = match file.sync_type {
            SyncType::Full => "latest_full.json".to_string(),
            SyncType::Diff => format!("diff_{}.json", file.created_at),
        };

        let path = self.dir.join(filename);
        let json = serde_json::to_string_pretty(file)?;
        std::fs::write(&path, &json)?;
        Ok(path)
    }

    pub fn load(&self, path: &Path) -> Result<SyncFile> {
        let json = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&json)?)
    }

    pub fn load_latest_full(&self) -> Result<Option<SyncFile>> {
        let path = self.dir.join("latest_full.json");
        if !path.exists() {
            return Ok(None);
        }
        Ok(Some(self.load(&path)?))
    }
}
