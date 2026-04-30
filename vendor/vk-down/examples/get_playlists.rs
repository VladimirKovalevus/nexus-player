use anyhow::Result;
use vk_api::{auth, VkClient};

#[tokio::main]
async fn main() -> Result<()> {
    println!("╔════════════════════════════════════════════════════════╗");
    println!("║       VK Music - Получение плейлистов                  ║");
    println!("╚════════════════════════════════════════════════════════╝\n");

    // Шаг 1: Авторизация
    let token = auth::interactive_auth()?;

    // Шаг 2: Создание клиента
    let client = VkClient::new(token);

    // Шаг 3: Получение информации о текущем пользователе
    let me = client.get_me().await?;
    println!("Пользователь: {} (ID: {})\n", me.full_name(), me.id);

    // Шаг 4: Получение всех плейлистов
    println!("Загружаем плейлисты...\n");

    let playlists = client.get_all_playlists(Some(me.id)).await?;

    // Шаг 5: Вывод результатов
    if playlists.is_empty() {
        println!("У вас нет плейлистов.");
    } else {
        println!("Найдено плейлистов: {}\n", playlists.len());
        println!("╔════════════════════════════════════════════════════════╗");
        println!("║                   Ваши плейлисты                       ║");
        println!("╠════════════════════════════════════════════════════════╣");

        for (i, playlist) in playlists.into_iter().enumerate() {
            println!(
                "║ {:>3}. {} ({} треков)",
                i + 1,
                truncate(&playlist.title, 35),
                playlist.count
            );

            if !playlist.description.is_empty() {
                println!("║      {}", truncate(&playlist.description, 50));
            }

            println!("║      ID: {}", playlist.full_id());
            println!("╠────────────────────────────────────────────────────────╣");
            if playlist.id == 63188272 {
                println!(
                    "PIZDA {:?}",
                    client.get_playlist_tracks(&playlist, 10, 0).await
                );
            }
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
