use std::time::Duration;

use anyhow::{anyhow, Result};
use reqwest::{multipart, Client};
use serde::Deserialize;

const TIMEOUT_SECS: u64 = 30;

#[derive(Deserialize)]
struct TgResponse<T> {
    ok: bool,
    result: Option<T>,
    description: Option<String>,
}

#[derive(Deserialize)]
pub struct TgMessage {
    pub message_id: i64,
    pub document: Option<TgDocument>,
}

#[derive(Deserialize)]
pub struct TgDocument {
    pub file_id: String,
}

#[derive(Deserialize)]
pub struct TgChat {
    pub pinned_message: Option<TgMessage>,
}

#[derive(Deserialize)]
pub struct TgFile {
    pub file_path: Option<String>,
}

pub struct TelegramClient {
    http: Client,
    token: String,
}

impl TelegramClient {
    pub fn new(token: impl Into<String>) -> Result<Self> {
        let http = Client::builder()
            .use_rustls_tls()
            .timeout(Duration::from_secs(TIMEOUT_SECS))
            .build()?;
        Ok(Self { http, token: token.into() })
    }

    fn api_url(&self, method: &str) -> String {
        format!("https://api.telegram.org/bot{}/{}", self.token, method)
    }

    async fn check<T: for<'de> Deserialize<'de>>(&self, resp: reqwest::Response) -> Result<T> {
        let tg: TgResponse<T> = resp.json().await?;
        if tg.ok {
            tg.result.ok_or_else(|| anyhow!("Telegram: ok=true but result is null"))
        } else {
            Err(anyhow!("Telegram API error: {}", tg.description.unwrap_or_default()))
        }
    }

    pub async fn send_document(
        &self,
        chat_id: &str,
        bytes: Vec<u8>,
        filename: &str,
    ) -> Result<TgMessage> {
        let part = multipart::Part::bytes(bytes)
            .file_name(filename.to_string())
            .mime_str("application/json")?;

        let form = multipart::Form::new()
            .text("chat_id", chat_id.to_string())
            .part("document", part);

        let resp = self
            .http
            .post(self.api_url("sendDocument"))
            .multipart(form)
            .send()
            .await?;

        self.check(resp).await
    }

    pub async fn delete_message(&self, chat_id: &str, message_id: i64) -> Result<()> {
        let body = serde_json::json!({
            "chat_id": chat_id,
            "message_id": message_id,
        });

        let resp = self
            .http
            .post(self.api_url("deleteMessage"))
            .json(&body)
            .send()
            .await?;

        // deleteMessage returns True (bool) on success
        let tg: TgResponse<serde_json::Value> = resp.json().await?;
        if tg.ok {
            Ok(())
        } else {
            Err(anyhow!("deleteMessage failed: {}", tg.description.unwrap_or_default()))
        }
    }

    pub async fn pin_message(&self, chat_id: &str, message_id: i64) -> Result<()> {
        let body = serde_json::json!({
            "chat_id": chat_id,
            "message_id": message_id,
            "disable_notification": true,
        });

        let resp = self
            .http
            .post(self.api_url("pinChatMessage"))
            .json(&body)
            .send()
            .await?;

        let tg: TgResponse<serde_json::Value> = resp.json().await?;
        if tg.ok {
            Ok(())
        } else {
            Err(anyhow!("pinChatMessage failed: {}", tg.description.unwrap_or_default()))
        }
    }

    pub async fn get_chat(&self, chat_id: &str) -> Result<TgChat> {
        let resp = self
            .http
            .get(self.api_url("getChat"))
            .query(&[("chat_id", chat_id)])
            .send()
            .await?;

        self.check(resp).await
    }

    pub async fn get_file(&self, file_id: &str) -> Result<TgFile> {
        let resp = self
            .http
            .get(self.api_url("getFile"))
            .query(&[("file_id", file_id)])
            .send()
            .await?;

        self.check(resp).await
    }

    pub async fn download_file(&self, file_path: &str) -> Result<Vec<u8>> {
        let url = format!(
            "https://api.telegram.org/file/bot{}/{}",
            self.token, file_path
        );
        let resp = self.http.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("download_file HTTP {}", resp.status()));
        }
        Ok(resp.bytes().await?.to_vec())
    }
}
