use vk_api::{auth, Playlist, VkClient};

#[tokio::main]
async fn main() {
    println!("╔════════════════════════════════════════════════════════╗");
    println!("║       VK Music Downloader - Получение одного трека    ║");
    println!("╚════════════════════════════════════════════════════════╝\n");

    // Шаг 1: Авторизация
    let token = auth::interactive_auth().unwrap();

    // Шаг 2: Создание клиента
    let client = VkClient::new(token);

    // Шаг 3: Получение первого трека из библиотеки пользователя
    println!("Получаем первый трек из вашей библиотеки...\n");

    // let tracks = client.get_audio(None, 1, 0).await.unwrap();
    let tracks = client.get_by_id(vec![
        "-224099112_456246657".to_string(),
        "-102670202_456243864".to_string(),
    ]);
    println!("ADSAS {:?}", tracks.await);
    let input = "191807149_63188272";
    let mut parts = input.split_whitespace();

    // Try to parse the first integer
    let int1: i64 = match parts.next().and_then(|s| s.parse().ok()) {
        Some(n) => n,
        None => {
            return;
        }
    };

    // Try to parse the second integer
    let int2: i64 = match parts.next().and_then(|s| s.parse().ok()) {
        Some(n) => n,
        None => {
            return;
        }
    };

    println!("Parsed integers: {}, {}", int1, int2);
    println!("Sum: {}", int1 + int2);

    let p = Playlist {
        owner_id: int2,
        id: int1,
        title: "".to_string(),
        access_key: "".to_string(),
        count: 0,
        create_time: 0,
        description: "".to_string(),
        photo: None,
        update_time: 0,
    };
    let tracks = client.get_playlist_tracks(&p, 10, 0).await.unwrap();

    // Шаг 4: Вывод информации о треке
    if let Some(track) = tracks.first() {
        println!("╔════════════════════════════════════════════════════════╗");
        println!("║                   Информация о треке                   ║");
        println!("╠════════════════════════════════════════════════════════╣");
        println!("║ Исполнитель: {:<41} ║", truncate(&track.artist, 41));
        println!("║ Название:    {:<41} ║", truncate(&track.title, 41));
        println!(
            "║ Длительность: {:<2}:{:02} мин                              ║",
            track.duration / 60,
            track.duration % 60
        );
        println!("║ ID трека:    {:<41} ║", track.audio_id());
        println!("╠════════════════════════════════════════════════════════╣");
        println!("║                   Прямая ссылка на MP3                 ║");
        println!("╠════════════════════════════════════════════════════════╣");
        println!("║ {:<53} ║", truncate(&track.url, 53));
        println!("╚════════════════════════════════════════════════════════╝\n");

        println!("✓ Трек успешно получен!");
        println!("  Вы можете использовать URL для скачивания или воспроизведения.");
        println!("  Внимание: ссылка временная и действует несколько часов.\n");
    } else {
        println!("⚠ В вашей библиотеке нет треков.");
    }
}

/// Обрезать строку до указанной длины
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
