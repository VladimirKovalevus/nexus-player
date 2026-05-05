use anyhow::Result;
use vk_api::{auth, VkClient};

#[tokio::main]
async fn main() -> Result<()> {
    println!("╔════════════════════════════════════════════════════════╗");
    println!("║       VK Music - Поиск треков по артисту               ║");
    println!("╚════════════════════════════════════════════════════════╝\n");

    // Шаг 1: Авторизация
    let token = auth::interactive_auth()?;

    // Шаг 2: Создание клиента
    let client = VkClient::new(token);

    // Шаг 3: Поиск треков по артисту
    let query = "neon scaffold";
    println!("Ищем треки по запросу: \"{}\"\n", query);

    let tracks = client.search_audio(query, 100, 0).await?;

    // Шаг 4: Вывод результатов
    if tracks.is_empty() {
        println!("Ничего не найдено по запросу \"{}\"", query);
    } else {
        println!("Найдено треков: {}\n", tracks.len());
        println!("╔════════════════════════════════════════════════════════╗");
        println!("║                   Результаты поиска                    ║");
        println!("╠════════════════════════════════════════════════════════╣");

        for (i, track) in tracks.iter().enumerate() {
            let duration = format!("{}:{:02}", track.duration / 60, track.duration % 60);
            println!(
                "║ {:>3}. {} - {} [{}]",
                i + 1,
                truncate(&track.artist, 20),
                truncate(&track.title, 25),
                duration
            );
            println!(
                "║      ID: {} | URL: {}",
                track.audio_id(),
                truncate(&track.url, 30)
            );
            println!("╠────────────────────────────────────────────────────────╣");
        }

        println!("╚════════════════════════════════════════════════════════╝\n");
    }

    Ok(())
}

/// Обрезать строку до указанной длины
fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len.saturating_sub(3)).collect();
        format!("{}...", truncated)
    }
}
