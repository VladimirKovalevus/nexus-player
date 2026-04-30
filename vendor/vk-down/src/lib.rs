//! # vk-down
//!
//! Библиотека для работы с VK Audio API на Rust.
//! Позволяет получать треки из VK Music с помощью официального мобильного API.
//!
//! ## Примеры использования
//!
//! ```no_run
//! use vk_down::{VkClient, auth};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Интерактивная авторизация
//!     let token = auth::interactive_auth()?;
//!     
//!     // Создать клиент
//!     let client = VkClient::new(token);
//!     
//!     // Получить первый трек из библиотеки
//!     let tracks = client.get_audio(None, 1, 0).await?;
//!     
//!     if let Some(track) = tracks.first() {
//!         println!("Трек: {}", track.full_title());
//!         println!("URL: {}", track.url);
//!     }
//!     
//!     Ok(())
//! }
//! ```

pub mod api;
pub mod auth;
pub mod models;

// Re-export основных типов для удобства
pub use api::VkClient;
pub use auth::{extract_token_from_url, get_auth_html, get_default_auth_url};
pub use models::{AudioList, Playlist, PlaylistList, Track, User, VkError, VkResponse, VkResult};
