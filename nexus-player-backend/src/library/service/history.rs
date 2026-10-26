use super::LibraryService;
use crate::library::models;
use anyhow::Result;

impl LibraryService {
    // ========================================================================
    // Play History Operations
    // ========================================================================

    pub async fn add_play_history(&self, entry: models::PlayHistoryEntry) -> Result<()> {
        self.history_repo.add(entry).await
    }

    pub async fn get_play_history(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<models::PlayHistoryEntry>> {
        self.history_repo.list_recent(limit, offset).await
    }

    pub async fn clear_play_history(&self) -> Result<()> {
        self.history_repo.clear().await
    }
}
