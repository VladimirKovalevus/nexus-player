use anyhow::{Context, Result};
use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Kate Mobile - неофициальное приложение с доступом к audio API
pub const VK_KATE_MOBILE_CLIENT_ID: &str = "2685278";
pub const VK_KATE_MOBILE_CLIENT_SECRET: &str = "lxhD8OD7dMsqtXIm5IUY";
/// VK Admin (альтернативный client_id с доступом к audio)
pub const VK_ADMIN_CLIENT_ID: &str = "6121396";
pub const VK_API_VERSION: &str = "5.186";
/// User-Agent для Kate Mobile (необходим для доступа к audio API)
pub const VK_USER_AGENT: &str = "KateMobileAndroid/129 lite-604 (Android 14; SDK 34; arm64-v8a; samsung SM-G998B; ru; 3088x1440)";

/// Конфигурация авторизации
#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub client_id: String,
    pub scope: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            client_id: VK_KATE_MOBILE_CLIENT_ID.to_string(),
            scope: "notify,friends,photos,audio,video,docs,status,notes,pages,wall,groups,messages,offline,notifications,stories".to_string(),
        }
    }
}

/// Сгенерировать device_id в формате Fenrir: 16hex:32hex
fn generate_device_id() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    let h1 = hasher.finish();
    (seed ^ 0xDEAD_BEEF).hash(&mut hasher);
    let h2 = hasher.finish();
    (seed ^ 0xCAFE_BABE).hash(&mut hasher);
    let h3 = hasher.finish();

    format!("{:016x}:{:016x}{:016x}", h1, h2, h3)
}

/// Загрузить device_id из .env, либо сгенерировать и сохранить новый
pub fn get_or_create_device_id() -> String {
    dotenv::dotenv().ok();
    if let Ok(id) = std::env::var("VK_DEVICE_ID") {
        if !id.is_empty() {
            return id;
        }
    }
    let id = generate_device_id();
    let _ = upsert_env_var("VK_DEVICE_ID", &id);
    id
}

/// Получить URL для авторизации через OAuth (браузерный implicit flow).
/// ВНИМАНИЕ: токены полученные этим способом имеют флаг oauth=1 и
/// не имеют доступа к audio API (error 3). Используйте direct_login для аудио.
pub fn get_oauth_url(config: &AuthConfig) -> String {
    let device_id = get_or_create_device_id();
    format!(
        "https://oauth.vk.ru/authorize?\
         client_id={}&\
         display=mobile&\
         scope={}&\
         redirect_uri=https%3A%2F%2Foauth.vk.ru%2Fblank.html&\
         response_type=token&\
         v={}&\
         lang=ru&\
         device_id={}&\
         https=1",
        config.client_id, config.scope, VK_API_VERSION, device_id
    )
}

/// Прямая авторизация через логин/пароль (grant_type=password).
/// Только такой токен имеет доступ к audio API — OAuth implicit flow
/// возвращает токен с oauth=1, для которого VK блокирует audio методы (error 3).
pub async fn direct_login(username: &str, password: &str) -> Result<String> {
    let device_id = get_or_create_device_id();

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::HeaderName::from_static("x-vk-android-client"),
        reqwest::header::HeaderValue::from_static("new"),
    );

    let client = reqwest::Client::builder()
        .user_agent(VK_USER_AGENT)
        .default_headers(headers)
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .context("Не удалось создать HTTP клиент")?;

    let params = [
        ("grant_type", "password"),
        ("client_id", VK_KATE_MOBILE_CLIENT_ID),
        ("client_secret", VK_KATE_MOBILE_CLIENT_SECRET),
        ("username", username),
        ("password", password),
        ("scope", "notify,friends,photos,audio,video,docs,status,notes,pages,wall,groups,messages,offline,notifications,stories"),
        ("v", VK_API_VERSION),
        ("device_id", &device_id),
        ("2fa_supported", "1"),
        ("libverify_support", "1"),
        ("lang", "ru"),
        ("https", "1"),
    ];

    let response = client
        .post("https://oauth.vk.ru/token")
        .form(&params)
        .send()
        .await
        .context("Не удалось отправить запрос авторизации")?;

    let body: serde_json::Value = response
        .json()
        .await
        .context("Не удалось распарсить ответ авторизации")?;

    if let Some(redirect) = body.get("redirect_uri").and_then(|v| v.as_str()) {
        anyhow::bail!(
            "Требуется дополнительная верификация: {}\n\
             VK просит подтвердить вход — попробуйте ещё раз после подтверждения.",
            redirect
        );
    }

    if let Some(err) = body.get("error") {
        let desc = body
            .get("error_description")
            .and_then(|v| v.as_str())
            .unwrap_or("неизвестная ошибка");
        anyhow::bail!("Ошибка авторизации: {} — {}", err, desc);
    }

    let token = body
        .get("access_token")
        .and_then(|v| v.as_str())
        .context("Поле access_token отсутствует в ответе")?
        .to_string();

    upsert_env_var("VK_ACCESS_TOKEN", &token)
        .context("Не удалось сохранить токен в .env")?;

    Ok(token)
}

/// Интерактивная авторизация - показывает URL и просит ввести токен (OAuth flow)
pub fn interactive_auth() -> Result<String> {
    // Сначала пытаемся загрузить токен из .env файла
    if let Some(token) = load_token_from_env() {
        println!("✓ Токен загружен из .env файла\n");
        return Ok(token);
    }

    // Если токена нет в .env, запрашиваем у пользователя
    let config = AuthConfig::default();
    let auth_url = get_oauth_url(&config);

    println!("╔═══════════════════════════════════════════════════════════╗");
    println!("║           Авторизация в VK для доступа к музыке          ║");
    println!("║              (используется Kate Mobile API)              ║");
    println!("╚═══════════════════════════════════════════════════════════╝\n");

    println!("Шаги для авторизации:");
    println!("1. Откройте эту ссылку в браузере:");
    println!("\n   {}\n", auth_url);
    println!("2. Войдите в VK и разрешите доступ к аудиозаписям");
    println!("3. После авторизации вы будете перенаправлены на страницу");
    println!("   https://oauth.vk.ru/blank.html#access_token=...");
    println!("4. Скопируйте значение access_token из адресной строки");
    println!("   (можно скопировать весь URL или только токен)\n");
    println!("ВАЖНО: OAuth-токен не даёт доступ к audio API.");
    println!("       Для аудио используйте direct_login().\n");

    print!("Введите access_token: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("Не удалось прочитать токен")?;

    let input = input.trim();

    if input.is_empty() {
        anyhow::bail!("Токен не может быть пустым");
    }

    let token = extract_token_from_url(input);

    save_token_to_env(&token).context("Не удалось сохранить токен в .env файл")?;

    println!("\n✓ Токен получен и сохранен в .env файл!\n");

    Ok(token)
}

/// Извлечь токен из redirect URL (если пользователь вставил полный URL)
pub fn extract_token_from_url(url_or_token: &str) -> String {
    if let Some(token_part) = url_or_token.split("access_token=").nth(1) {
        token_part
            .split('&')
            .next()
            .unwrap_or(url_or_token)
            .to_string()
    } else {
        url_or_token.to_string()
    }
}

/// Получить стандартную ссылку для авторизации
pub fn get_default_auth_url() -> String {
    get_oauth_url(&AuthConfig::default())
}

/// Получить HTML код для встраивания (например, в WebView)
pub fn get_auth_html() -> String {
    let url = get_default_auth_url();
    format!(
        r#"<!DOCTYPE html><html><head><meta charset="utf-8"><style>body,html{{margin:0;padding:0;height:100%;overflow:hidden}}iframe{{width:100%;height:100%;border:none}}</style></head><body><iframe src="{url}"></iframe></body></html>"#
    )
}

/// Загрузить .env файл (вызов dotenv::dotenv)
pub fn dotenv_load() {
    dotenv::dotenv().ok();
}

/// Загрузить токен из .env файла
pub fn load_token_from_env() -> Option<String> {
    dotenv::dotenv().ok();
    std::env::var("VK_ACCESS_TOKEN").ok()
}

/// Обновить или добавить переменную в .env файл
pub fn upsert_env_var(key: &str, value: &str) -> Result<()> {
    let env_path = ".env";
    let line = format!("{}={}", key, value);

    if Path::new(env_path).exists() {
        let existing = fs::read_to_string(env_path)?;
        let prefix = format!("{}=", key);
        if existing.lines().any(|l| l.starts_with(&prefix)) {
            let new_content = existing
                .lines()
                .map(|l| if l.starts_with(&prefix) { line.clone() } else { l.to_string() })
                .collect::<Vec<_>>()
                .join("\n");
            fs::write(env_path, new_content + "\n")?;
        } else {
            fs::write(env_path, format!("{}\n{}\n", existing.trim_end(), line))?;
        }
    } else {
        fs::write(env_path, format!("{}\n", line))?;
    }

    Ok(())
}

/// Сохранить токен в .env файл
pub fn save_token_to_env(token: &str) -> Result<()> {
    upsert_env_var("VK_ACCESS_TOKEN", token)
}
