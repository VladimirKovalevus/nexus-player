use serde::{Deserialize, Serialize};

/// Трек из VK
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    /// ID трека
    pub id: i64,
    /// ID владельца (пользователя или группы)
    pub owner_id: i64,
    /// Исполнитель
    pub artist: String,
    /// Название трека
    pub title: String,
    /// Длительность в секундах
    pub duration: u32,
    /// Прямая ссылка на MP3 (временная)
    pub url: String,
    /// Дата добавления (Unix timestamp)
    #[serde(default)]
    pub date: i64,
    /// Access key для доступа к треку
    #[serde(default)]
    pub access_key: String,
}

impl Track {
    /// Получить полное название трека
    pub fn full_title(&self) -> String {
        format!("{} - {}", self.artist, self.title)
    }

    /// Получить уникальный идентификатор в формате owner_id_id[_access_key]
    pub fn audio_id(&self) -> String {
        if self.access_key.is_empty() {
            format!("{}_{}", self.owner_id, self.id)
        } else {
            format!("{}_{}_{}", self.owner_id, self.id, self.access_key)
        }
    }
}

/// Ответ VK API для методов audio.*
#[derive(Debug, Deserialize)]
pub struct VkResponse<T> {
    pub response: T,
}

/// Ответ для списка аудиозаписей
#[derive(Debug, Deserialize)]
pub struct AudioList {
    pub count: u32,
    pub items: Vec<Track>,
}

/// Плейлист из VK
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Photo {
    pub photo_135: Option<String>,
    pub photo_270: Option<String>,
    pub photo_300: Option<String>,
    pub photo_600: Option<String>,
    pub photo_1200: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    /// ID плейлиста
    pub id: i64,
    /// ID владельца
    pub owner_id: i64,
    /// Название плейлиста
    pub title: String,
    /// Описание плейлиста
    #[serde(default)]
    pub description: String,
    /// Количество треков
    #[serde(default)]
    pub count: u32,
    /// Дата создания (Unix timestamp)
    #[serde(default)]
    pub create_time: i64,
    /// Дата обновления (Unix timestamp)
    #[serde(default)]
    pub update_time: i64,
    /// Access key для доступа к плейлисту
    #[serde(default)]
    pub access_key: String,
    /// Обложка плейлиста
    pub photo: Option<Photo>,
}

impl Playlist {
    /// Получить уникальный идентификатор плейлиста
    pub fn playlist_id(&self) -> String {
        format!("{}_{}", self.owner_id, self.id)
    }

    /// Получить полный идентификатор с access_key
    pub fn full_id(&self) -> String {
        if self.access_key.is_empty() {
            self.playlist_id()
        } else {
            format!("{}_{}_{}", self.owner_id, self.id, self.access_key)
        }
    }
}

/// Ответ для списка плейлистов
#[derive(Debug, Deserialize)]
pub struct PlaylistList {
    pub count: u32,
    pub items: Vec<Playlist>,
}

/// Блок в каталоге (может содержать треки или плейлисты)
#[derive(Debug, Deserialize, Clone)]
pub struct CatalogBlock {
    pub id: String,
    pub data_type: String, // "music_playlists", "music_audios", и т.д.
    pub playlists: Option<Vec<Playlist>>,
    pub audios: Option<Vec<Track>>,
}

/// Секция в каталоге
#[derive(Debug, Deserialize, Clone)]
pub struct CatalogSection {
    pub id: String,
    pub title: Option<String>,
    pub blocks: Vec<CatalogBlock>,
}

/// Ответ от метода audio.getCatalog
#[derive(Debug, Deserialize, Clone)]
pub struct CatalogResponse {
    pub sections: Vec<CatalogSection>,
}

/// Пользователь VK
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// ID пользователя
    pub id: i64,
    /// Имя
    pub first_name: String,
    /// Фамилия
    pub last_name: String,
}

impl User {
    /// Получить полное имя пользователя
    pub fn full_name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name)
    }
}

/// Ошибка VK API
#[derive(Debug, Deserialize)]
pub struct VkError {
    pub error_code: u32,
    pub error_msg: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum VkResult<T> {
    Ok(VkResponse<T>),
    Err { error: VkError },
}
