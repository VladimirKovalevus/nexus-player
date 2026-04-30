use anyhow::Result;
use std::io::{self, Write};
use vk_api::{auth, VkClient};

#[tokio::main]
async fn main() -> Result<()> {
    println!("╔════════════════════════════════════════════════════════╗");
    println!("║       VK Music - Получение трека (direct auth)         ║");
    println!("╚════════════════════════════════════════════════════════╝\n");

    let token = get_token().await?;
    let client = VkClient::new(token);

    // Первые 3 трека из библиотеки пользователя
    println!("Получаем треки из вашей библиотеки...\n");
    let tracks = client.get_audio(None, 3, 0).await?;

    if tracks.is_empty() {
        println!("⚠ В вашей библиотеке нет треков.");
        return Ok(());
    }

    for track in &tracks {
        println!("╔════════════════════════════════════════════════════════╗");
        println!("║ Исполнитель: {:<41} ║", truncate(&track.artist, 41));
        println!("║ Название:    {:<41} ║", truncate(&track.title, 41));
        println!(
            "║ Длительность: {:>2}:{:02}                                  ║",
            track.duration / 60,
            track.duration % 60
        );
        println!("║ ID: {:<50} ║", track.audio_id());
        println!("╠════════════════════════════════════════════════════════╣");
        println!("║ URL: {:<49} ║", truncate(&track.url, 49));
        println!("╚════════════════════════════════════════════════════════╝");
        println!();
    }

    println!("✓ Готово! Ссылки временные, действуют несколько часов.");

    Ok(())
}

async fn get_token() -> Result<String> {
    auth::dotenv_load();

    if let Some(token) = auth::load_token_from_env() {
        println!("✓ Токен загружен из .env файла\n");
        return Ok(token);
    }

    print!("Телефон или email: ");
    io::stdout().flush()?;
    let mut username = String::new();
    io::stdin().read_line(&mut username)?;

    print!("Пароль: ");
    io::stdout().flush()?;
    let mut password = String::new();
    io::stdin().read_line(&mut password)?;

    let token = auth::direct_login(username.trim(), password.trim()).await?;
    println!("\n✓ Авторизация успешна! Токен сохранён в .env\n");
    Ok(token)
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
