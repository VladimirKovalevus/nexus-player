use super::Result;
use crate::dto::SearchResultDto;
use nexus_player_backend::state::state::AppState;
use std::sync::Arc;
use tauri::State;

// ─── Direct Link Resolve ────────────────────────────────────────────────────

#[tauri::command]
pub async fn direct_link_resolve(
    state: State<'_, Arc<AppState>>,
    provider: String,
    url: String,
) -> Result<SearchResultDto> {
    let result = state
        .direct_link_resolve(&provider, &url)
        .await
        .map_err(|e| e.to_string())?;
    Ok(SearchResultDto::from(result))
}
