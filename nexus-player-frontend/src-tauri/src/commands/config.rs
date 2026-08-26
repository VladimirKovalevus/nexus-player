use super::Result;
use nexus_player_backend::state::state::AppState;
use std::sync::Arc;
use tauri::{Emitter, State};

use nexus_player_backend::config::AppConfig;

#[tauri::command]
pub fn get_app_config(state: State<'_, Arc<AppState>>) -> Result<AppConfig> {
    Ok(state.config())
}

#[tauri::command]
pub async fn update_app_config(state: State<'_, Arc<AppState>>, config: AppConfig) -> Result<()> {
    state
        .update_config(config)
        .await
        .map_err(crate::error::CommandError::from)
}

#[tauri::command]
pub async fn update_provider_auth(
    state: State<'_, Arc<AppState>>,
    provider: String,
    auth_data: String,
) -> Result<()> {
    tracing::info!(
        "Updating auth for provider {}: (data len={})",
        provider,
        auth_data.len()
    );

    // 1. Update the active provider in memory
    state
        .streaming()
        .update_provider_auth(&provider, auth_data.clone())
        .await;

    // 2. Persist to configuration file
    let mut config = state.config();
    match provider.as_str() {
        "soundcloud" => {
            config.providers.soundcloud.credentials = Some(auth_data);
        }
        "bandcamp" => {
            config.providers.bandcamp.credentials = Some(auth_data);
        }
        _ => {
            tracing::warn!(
                "Unknown provider: {}, skipping config persistence",
                provider
            );
        }
    }

    state
        .update_config(config)
        .await
        .map_err(crate::error::CommandError::from)
}

#[tauri::command]
pub async fn submit_challenge_result(
    app: tauri::AppHandle,
    client_id: String,
    label: String,
) -> Result<()> {
    tracing::info!("Challenge result submitted from {}: {}", label, client_id);
    // Emit a global event from the backend which is more reliable
    app.emit(
        "challenge-solved-global",
        serde_json::json!({
            "clientId": client_id,
            "label": label
        }),
    )?;
    Ok(())
}

#[tauri::command]
pub async fn open_challenge_window(
    app: tauri::AppHandle,
    label: String,
    url: String,
    script: String,
) -> Result<()> {
    tracing::info!("Opening challenge window: {} for {}", label, url);

    #[cfg(desktop)]
    {
        if let Some(win) = tauri::Manager::get_webview_window(&app, &label) {
            let _ = win.close();
        }

        let parsed_url = url
            .parse()
            .map_err(|e| format!("Invalid challenge URL: {}", e))?;
        tauri::WebviewWindowBuilder::new(&app, &label, tauri::WebviewUrl::External(parsed_url))
            .initialization_script(&script)
            .visible(false)
            .build()
            .map_err(|e: tauri::Error| e.to_string())?;
    }

    // On Android the native SoundCloudChallengePlugin handles this:
    // it opens a Dialog with a WebView, injects the XHR/fetch interception script,
    // and calls submit_challenge_result via JavascriptInterface when client_id is found.
    // The Kotlin plugin fires the resolve() callback which the frontend awaits.
    // Nothing to do here on mobile — the frontend calls the Kotlin command directly.

    Ok(())
}
