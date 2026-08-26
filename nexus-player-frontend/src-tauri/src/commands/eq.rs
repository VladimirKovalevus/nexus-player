use super::Result;
use crate::dto::{EqPresetDto, EqSettingsDto};
use nexus_player_backend::state::state::AppState;
use std::sync::Arc;
use tauri::State;

// ─── EQ ────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_eq_settings(state: State<'_, Arc<AppState>>) -> Result<EqSettingsDto> {
    Ok(state.get_eq_settings().into())
}

#[tauri::command]
pub async fn set_eq_settings(
    state: State<'_, Arc<AppState>>,
    settings: EqSettingsDto,
) -> Result<()> {
    state
        .set_eq_settings(settings.into())
        .map_err(crate::error::CommandError::from)
}

#[tauri::command]
pub async fn get_eq_presets(_state: State<'_, Arc<AppState>>) -> Result<Vec<EqPresetDto>> {
    let presets = nexus_player_backend::audio::eq::builtin_presets()
        .into_iter()
        .map(|(name, gains)| EqPresetDto {
            name: name.to_string(),
            gains: gains.to_vec(),
        })
        .collect();
    Ok(presets)
}
