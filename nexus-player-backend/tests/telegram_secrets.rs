//! The Telegram bot token never shows up in error text or Debug output.

use nexus_player_backend::sync::telegram::client::TelegramClient;
use nexus_player_backend::sync::TelegramConfig;

const TOKEN: &str = "123456:FAKE-secret-token-XYZ";
const REFUSED_BASE: &str = "http://127.0.0.1:1";

fn client() -> TelegramClient {
    TelegramClient::with_base_url(TOKEN, REFUSED_BASE).expect("client builds")
}

fn assert_no_token(err: &anyhow::Error) {
    for text in [err.to_string(), format!("{err:#}"), format!("{err:?}")] {
        assert!(!text.contains(TOKEN), "error leaks token: {text}");
    }
}

#[tokio::test]
async fn get_chat_error_does_not_contain_token() {
    let err = client()
        .get_chat("42")
        .await
        .err()
        .expect("connection to port 1 must fail");
    assert_no_token(&err);
}

#[tokio::test]
async fn download_file_error_does_not_contain_token() {
    let err = client()
        .download_file("documents/file.json")
        .await
        .err()
        .expect("connection to port 1 must fail");
    assert_no_token(&err);
}

#[test]
fn telegram_config_debug_masks_token() {
    let config = TelegramConfig {
        bot_token: TOKEN.to_string(),
        chat_id: "-100777".to_string(),
        last_message_id: Some(5),
    };
    let text = format!("{config:?}");
    assert!(!text.contains(TOKEN), "Debug leaks token: {text}");
    assert!(text.contains("-100777"), "Debug should keep chat id: {text}");
}
