use crate::auth::{get_or_create_device_id, VK_API_VERSION, VK_USER_AGENT};
use crate::models::{AudioList, Playlist, PlaylistList, Track, User, VkResult};
use anyhow::{Context, Result};
use reqwest::{header, Client};

/// Клиент для работы с VK API
#[derive(Debug, Clone)]
pub struct VkClient {
    client: Client,
    token: String,
    device_id: String,
}

impl VkClient {
    /// Создать новый клиент с токеном доступа
    pub fn new(token: String) -> Self {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::USER_AGENT,
            header::HeaderValue::from_static(VK_USER_AGENT),
        );
        headers.insert(
            header::HeaderName::from_static("x-vk-android-client"),
            header::HeaderValue::from_static("new"),
        );

        let client = Client::builder()
            .default_headers(headers)
            .build()
            .expect("Не удалось создать HTTP клиент");

        let device_id = get_or_create_device_id();

        Self {
            client,
            token,
            device_id,
        }
    }

    /// Базовые параметры для всех запросов (без access_token — он в заголовке)
    fn base_params(&self) -> Vec<(&'static str, String)> {
        vec![
            ("v", VK_API_VERSION.to_string()),
            ("lang", "ru".to_string()),
            ("https", "1".to_string()),
            ("device_id", self.device_id.clone()),
        ]
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.token)
    }

    /// Получить информацию о текущем пользователе
    pub async fn get_me(&self) -> Result<User> {
        let params = vec![
            ("v", VK_API_VERSION.to_string()),
            ("lang", "ru".to_string()),
        ];

        let response = self
            .client
            .post("https://api.vk.ru/method/users.get")
            .header("Authorization", self.auth_header())
            .form(&params)
            .send()
            .await
            .context("Не удалось отправить запрос к VK API")?;

        let result: VkResult<Vec<User>> = response
            .json()
            .await
            .context("Не удалось распарсить ответ VK API")?;

        match result {
            VkResult::Ok(data) => data
                .response
                .into_iter()
                .next()
                .context("Пустой ответ от VK API"),
            VkResult::Err { error } => {
                anyhow::bail!(
                    "Ошибка VK API: {} (код: {})",
                    error.error_msg,
                    error.error_code
                )
            }
        }
    }

    /// Получить аудиозаписи пользователя
    pub async fn get_audio(
        &self,
        owner_id: Option<i64>,
        count: u32,
        offset: u32,
    ) -> Result<Vec<Track>> {
        let mut params = self.base_params();
        params.push(("count", count.to_string()));
        params.push(("offset", offset.to_string()));
        if let Some(id) = owner_id {
            params.push(("owner_id", id.to_string()));
        }

        let response = self
            .client
            .post("https://api.vk.ru/method/audio.get")
            .header("Authorization", self.auth_header())
            .form(&params)
            .send()
            .await
            .context("Не удалось отправить запрос к VK API")?;

        let result: VkResult<AudioList> = response
            .json()
            .await
            .context("Не удалось распарсить ответ VK API")?;

        match result {
            VkResult::Ok(data) => Ok(data.response.items),
            VkResult::Err { error } => {
                anyhow::bail!(
                    "Ошибка VK API: {} (код: {})",
                    error.error_msg,
                    error.error_code
                )
            }
        }
    }

    /// Найти аудиозаписи по запросу
    pub async fn search_audio(&self, query: &str, count: u32, offset: u32) -> Result<Vec<Track>> {
        let mut params = self.base_params();
        params.push(("q", query.to_string()));
        params.push(("count", count.to_string()));
        params.push(("offset", offset.to_string()));

        let response = self
            .client
            .post("https://api.vk.ru/method/audio.search")
            .header("Authorization", self.auth_header())
            .form(&params)
            .send()
            .await
            .context("Не удалось отправить запрос к VK API")?;

        let result: VkResult<AudioList> = response
            .json()
            .await
            .context("Не удалось распарсить ответ VK API")?;

        match result {
            VkResult::Ok(data) => Ok(data.response.items),
            VkResult::Err { error } => {
                anyhow::bail!(
                    "Ошибка VK API: {} (код: {})",
                    error.error_msg,
                    error.error_code
                )
            }
        }
    }

    /// Получить конкретные аудиозаписи по ID
    ///
    /// `audio_ids` — список в формате "owner_id_audio_id"
    pub async fn get_by_id(&self, audio_ids: Vec<String>) -> Result<Vec<Track>> {
        let mut params = self.base_params();
        params.push(("audios", audio_ids.join(",")));

        let response = self
            .client
            .post("https://api.vk.ru/method/audio.getById")
            .header("Authorization", self.auth_header())
            .form(&params)
            .send()
            .await
            .context("Не удалось отправить запрос к VK API")?;

        let body = response
            .text()
            .await
            .context("Не удалось прочитать ответ")?;
        let result: VkResult<Vec<Track>> =
            serde_json::from_str(&body).context("Не удалось распарсить ответ VK API")?;

        match result {
            VkResult::Ok(data) => Ok(data.response),
            VkResult::Err { error } => {
                anyhow::bail!(
                    "Ошибка VK API: {} (код: {})",
                    error.error_msg,
                    error.error_code
                )
            }
        }
    }

    /// Получить плейлисты пользователя
    pub async fn get_my_playlists(
        &self,
        owner_id: Option<i64>,
        count: u32,
        offset: u32,
    ) -> Result<(Vec<Playlist>, u32)> {
        let mut params = self.base_params();
        params.push(("count", count.to_string()));
        params.push(("offset", offset.to_string()));
        if let Some(id) = owner_id {
            params.push(("owner_id", id.to_string()));
        }

        let response = self
            .client
            .post("https://api.vk.ru/method/audio.getPlaylists")
            .header("Authorization", self.auth_header())
            .form(&params)
            .send()
            .await
            .context("Не удалось отправить запрос к VK API")?;

        let result: VkResult<PlaylistList> = response
            .json()
            .await
            .context("Не удалось распарсить ответ VK API")?;

        match result {
            VkResult::Ok(data) => Ok((data.response.items, data.response.count)),
            VkResult::Err { error } => {
                anyhow::bail!(
                    "Ошибка VK API: {} (код: {})",
                    error.error_msg,
                    error.error_code
                )
            }
        }
    }

    /// Получить все плейлисты пользователя с автоматической пагинацией
    pub async fn get_all_playlists(&self, owner_id: Option<i64>) -> Result<Vec<Playlist>> {
        let mut all_playlists = Vec::new();
        let mut offset = 0;
        const PAGE_SIZE: u32 = 100;

        loop {
            let (playlists, total) = self.get_my_playlists(owner_id, PAGE_SIZE, offset).await?;

            if playlists.is_empty() {
                break;
            }

            all_playlists.extend(playlists);

            offset += PAGE_SIZE;
            if offset >= total {
                break;
            }
        }

        Ok(all_playlists)
    }

    /// Получить треки из плейлиста
    pub async fn get_playlist_tracks(
        &self,
        playlist: &Playlist,
        count: u32,
        offset: u32,
    ) -> Result<Vec<Track>> {
        let mut params = self.base_params();
        params.push(("owner_id", playlist.owner_id.to_string()));
        params.push(("album_id", playlist.id.to_string()));
        params.push(("count", count.to_string()));
        params.push(("offset", offset.to_string()));
        if !playlist.access_key.is_empty() {
            params.push(("access_key", playlist.access_key.clone()));
        }

        let response = self
            .client
            .post("https://api.vk.ru/method/audio.get")
            .header("Authorization", self.auth_header())
            .form(&params)
            .send()
            .await
            .context("Не удалось отправить запрос к VK API")?;

        let result: VkResult<AudioList> = response
            .json()
            .await
            .context("Не удалось распарсить ответ VK API")?;

        match result {
            VkResult::Ok(data) => Ok(data.response.items),
            VkResult::Err { error } => {
                anyhow::bail!(
                    "Ошибка VK API: {} (код: {})",
                    error.error_msg,
                    error.error_code
                )
            }
        }
    }

    /// Получить все треки из плейлиста с автоматической пагинацией
    pub async fn get_all_playlist_tracks(&self, playlist: &Playlist) -> Result<Vec<Track>> {
        let mut all_tracks = Vec::new();
        let mut offset = 0;
        const PAGE_SIZE: u32 = 100;

        loop {
            let tracks = self
                .get_playlist_tracks(playlist, PAGE_SIZE, offset)
                .await?;

            if tracks.is_empty() {
                break;
            }

            let tracks_len = tracks.len() as u32;
            all_tracks.extend(tracks);

            offset += PAGE_SIZE;
            if tracks_len < PAGE_SIZE {
                break;
            }
        }

        Ok(all_tracks)
    }

    /// Поиск плейлистов по текстовому запросу
    pub async fn search_albums(
        &self,
        query: &str,
        count: u32,
        offset: u32,
    ) -> Result<(Vec<Playlist>, u32)> {
        let mut params = self.base_params();
        params.push(("q", query.to_string()));
        params.push(("count", count.to_string()));
        params.push(("offset", offset.to_string()));

        let response = self
            .client
            .post("https://api.vk.ru/method/audio.searchPlaylists")
            .header("Authorization", self.auth_header())
            .form(&params)
            .send()
            .await
            .context("Не удалось отправить запрос к VK API")?;

        let result: VkResult<PlaylistList> = response
            .json()
            .await
            .context("Не удалось распарсить ответ VK API")?;

        match result {
            VkResult::Ok(data) => Ok((data.response.items, data.response.count)),
            VkResult::Err { error } => {
                anyhow::bail!(
                    "Ошибка VK API: {} (код: {})",
                    error.error_msg,
                    error.error_code
                )
            }
        }
    }

    /// Получить рекомендованные треки
    pub async fn get_recommendations(
        &self,
        count: u32,
        offset: u32,
        target_audio: Option<String>,
    ) -> Result<Vec<Track>> {
        let mut params = self.base_params();
        params.push(("count", count.to_string()));
        params.push(("offset", offset.to_string()));
        if let Some(audio_id) = target_audio {
            params.push(("target_audio", audio_id));
        }

        let response = self
            .client
            .post("https://api.vk.ru/method/audio.getRecommendations")
            .header("Authorization", self.auth_header())
            .form(&params)
            .send()
            .await
            .context("Не удалось отправить запрос к VK API")?;

        let result: VkResult<AudioList> = response
            .json()
            .await
            .context("Не удалось распарсить ответ VK API")?;

        match result {
            VkResult::Ok(data) => Ok(data.response.items),
            VkResult::Err { error } => {
                anyhow::bail!(
                    "Ошибка VK API: {} (код: {})",
                    error.error_msg,
                    error.error_code
                )
            }
        }
    }
}
