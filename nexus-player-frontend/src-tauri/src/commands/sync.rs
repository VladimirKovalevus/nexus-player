use super::Result;
use nexus_player_backend::state::state::AppState;
use nexus_player_backend::sync::{
    SyncApplier, SyncBuilder, SyncStore, SyncType, TelegramConfig, TelegramSync,
};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub async fn export_sync(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    sync_type: SyncType,
    device_name: Option<String>,
) -> Result<String> {
    let sync_dir = app.path().app_data_dir()?.join("sync");

    let builder = SyncBuilder::new(state.library());
    let file = match sync_type {
        SyncType::Full => builder.build_full(device_name).await,
        SyncType::Diff => builder.build_diff(device_name).await,
    }?;

    let store = SyncStore::new(sync_dir);
    let path = store.save(&file)?;

    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn import_sync(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    path: String,
) -> Result<()> {
    let sync_dir = app.path().app_data_dir()?.join("sync");

    let store = SyncStore::new(sync_dir);
    let file = store.load(std::path::Path::new(&path))?;

    let applier = SyncApplier::new(state.library());
    applier
        .apply(file)
        .await
        .map_err(crate::error::CommandError::from)
}

fn telegram_config_path(app: &AppHandle) -> Result<std::path::PathBuf> {
    Ok(app
        .path()
        .app_data_dir()?
        .join("sync")
        .join("telegram_config.json"))
}

fn telegram_store(
    app: &AppHandle,
) -> Result<Box<dyn nexus_player_backend::sync::telegram::token_store::TokenStore>> {
    let dir = app.path().app_data_dir()?.join("sync");
    Ok(nexus_player_backend::sync::telegram::token_store::default_token_store(&dir))
}

#[tauri::command]
pub async fn telegram_get_config(app: AppHandle) -> Result<Option<serde_json::Value>> {
    let path = telegram_config_path(&app)?;
    let store = telegram_store(&app)?;
    match TelegramConfig::load_with_store(&path, store.as_ref())? {
        Some(cfg) => Ok(Some(serde_json::json!({ "chatId": cfg.chat_id }))),
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn telegram_save_config(
    app: AppHandle,
    bot_token: String,
    chat_id: String,
) -> Result<()> {
    let path = telegram_config_path(&app)?;
    let config = TelegramConfig {
        bot_token,
        chat_id,
        last_message_id: None,
    };
    let store = telegram_store(&app)?;
    config
        .save_with_store(&path, store.as_ref())
        .map_err(crate::error::CommandError::from)
}

#[tauri::command]
pub async fn telegram_check_config(app: AppHandle) -> Result<bool> {
    let path = telegram_config_path(&app)?;
    let store = telegram_store(&app)?;
    match TelegramSync::load(&path, store.as_ref())? {
        Some(tg) => Ok(tg.check().await),
        None => Ok(false),
    }
}

#[tauri::command]
pub async fn telegram_push(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    sync_type: SyncType,
    device_name: Option<String>,
) -> Result<()> {
    let path = telegram_config_path(&app)?;
    let store = telegram_store(&app)?;
    let mut tg = TelegramSync::load(&path, store.as_ref())?
        .ok_or_else(|| "Telegram is not configured".to_string())?;

    let builder = SyncBuilder::new(state.library());
    let file = match sync_type {
        SyncType::Full => builder.build_full(device_name).await,
        SyncType::Diff => builder.build_diff(device_name).await,
    }?;

    tg.push(&file)
        .await
        .map_err(crate::error::CommandError::from)
}

#[tauri::command]
pub async fn telegram_pull(app: AppHandle, state: State<'_, Arc<AppState>>) -> Result<()> {
    let path = telegram_config_path(&app)?;
    let store = telegram_store(&app)?;
    let tg = TelegramSync::load(&path, store.as_ref())?
        .ok_or_else(|| "Telegram is not configured".to_string())?;

    let file = tg.pull().await?;
    let applier = SyncApplier::new(state.library());
    applier
        .apply(file)
        .await
        .map_err(crate::error::CommandError::from)
}
