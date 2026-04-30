# vk-down

Rust библиотека для работы с VK Music API. Позволяет получать треки из VK с помощью Kate Mobile API.

## Возможности

- ✅ Авторизация через OAuth (официальное Android приложение VK)
- ✅ Получение треков из библиотеки пользователя
- ✅ Поиск треков по названию/исполнителю
- ✅ Получение прямых ссылок на MP3
- ✅ Async/await на базе tokio

## Установка

Добавьте в `Cargo.toml`:

```toml
[dependencies]
vk-down = { path = "." }
tokio = { version = "1", features = ["full"] }
anyhow = "1.0"
```

## Быстрый старт

```rust
use vk_down::{auth, VkClient};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Авторизация (автоматически загружает токен из .env или запрашивает новый)
    let token = auth::interactive_auth()?;

    // 2. Создание клиента
    let client = VkClient::new(token);

    // 3. Получение первого трека
    let tracks = client.get_audio(None, 1, 0).await?;

    if let Some(track) = tracks.first() {
        println!("Трек: {} - {}", track.artist, track.title);
        println!("MP3: {}", track.url);
    }

    Ok(())
}
```

## Запуск примера

```bash
# Получить один трек из библиотеки
cargo run --example get_track
```

### Процесс авторизации:

**Первый запуск:**
1. Программа выведет ссылку для авторизации
2. Откройте её в браузере
3. Войдите в VK и разрешите доступ
4. Скопируйте `access_token` из адресной строки
5. Вставьте токен в консоль
6. Токен автоматически сохранится в файл `.env`

**Последующие запуски:**
- Токен автоматически загружается из `.env` файла
- Повторная авторизация не требуется

**Ручная настройка:**
```bash
# Создайте файл .env в корне проекта
echo "VK_ACCESS_TOKEN=ваш_токен_здесь" > .env
```

## API

### Модуль `auth`

```rust
// Интерактивная авторизация (загружает из .env или запрашивает новый токен)
let token = auth::interactive_auth()?;

// Загрузить токен из .env файла
let token = auth::load_token_from_env();

// Сохранить токен в .env файл
auth::save_token_to_env(&token)?;

// Получить OAuth URL для авторизации
let url = auth::get_oauth_url(&auth::AuthConfig::default());

// Извлечь токен из redirect URL
let token = auth::extract_token_from_url("https://oauth.vk.com/blank.html#access_token=XXX");
```

### Модуль `api`

```rust
let client = VkClient::new(token);

// Получить треки пользователя (owner_id: None = текущий пользователь)
let tracks = client.get_audio(None, 10, 0).await?;

// Поиск треков
let results = client.search_audio("Metallica", 5, 0).await?;

// Получить конкретные треки по ID
let tracks = client.get_by_id(vec!["123456_789".to_string()]).await?;
```

### Структура `Track`

```rust
pub struct Track {
    pub id: i64,
    pub owner_id: i64,
    pub artist: String,
    pub title: String,
    pub duration: u32,
    pub url: String,  // Прямая ссылка на MP3 (временная)
    pub date: i64,
}

// Методы:
track.full_title()  // "Artist - Title"
track.audio_id()    // "owner_id_id"
```

## Важные особенности

### Прямые ссылки на MP3
- Ссылки **временные** (живут 4-24 часа)
- Перед воспроизведением рекомендуется обновлять через `get_by_id()`
- Содержат подпись - нельзя менять параметры URL

### Авторизация
- Используется официальное Android приложение VK (client_id: 2274003)
- Токены с `offline` scope живут вечно
- Рекомендуется сохранять токен локально и переиспользовать

### Ограничения VK API
- Не более 3 запросов в секунду
- Некоторые методы требуют капчу при подозрительной активности
- Официальное Desktop API **НЕ** имеет доступа к audio методам

## Структура проекта

```
vk-down/
├── src/
│   ├── lib.rs      # Основной модуль библиотеки
│   ├── auth.rs     # Модуль авторизации
│   ├── api.rs      # VK API клиент
│   └── models.rs   # Модели данных (Track, AudioList, и т.д.)
├── examples/
│   └── get_track.rs  # Пример получения одного трека
└── Cargo.toml
```

## Лицензия

MIT

## Предупреждение

Эта библиотека использует неофициальное мобильное API VK. Используйте на свой риск.
VK может в любой момент изменить или заблокировать доступ к API.
