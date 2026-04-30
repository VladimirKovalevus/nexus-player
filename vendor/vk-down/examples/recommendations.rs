use anyhow::Result;
use dotenv::dotenv;
use std::env;
use vk_api::api::VkClient;
use vk_api::models::{Playlist, Track};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    let token = env::var("VK_ACCESS_TOKEN").expect("VK_ACCESS_TOKEN must be set");
    let client = VkClient::new(token);

    // 1. Поиск альбомов по запросу "veturheim"
    println!("--- Поиск альбомов по запросу 'veturheim' ---");
    let (albums, total): (Vec<Playlist>, u32) = client.search_albums("veturheim", 5, 0).await?;
    println!("Найдено всего: {}", total);
    for album in albums {
        println!(
            "Альбом: {} (ID: {}, Owner: {})",
            album.title, album.id, album.owner_id
        );
    }
    println!();

    // 2. Общие рекомендации треков
    println!("--- Общие рекомендации треков ---");
    let general_recs: Vec<Track> = client.get_recommendations(5, 0, None).await?;
    for (i, track) in general_recs.iter().enumerate() {
        println!("{}. {} - {}", i + 1, track.artist, track.title);
    }
    println!();

    // 3. Рекомендации на основе первого трека из библиотеки
    println!("--- Рекомендации на основе первого трека в библиотеке ---");
    let my_audios: Vec<Track> = client.get_audio(None, 1, 0).await?;
    if let Some(first_track) = my_audios.first() {
        println!(
            "Ваш первый трек: {} - {}",
            first_track.artist, first_track.title
        );

        let target_audio = Some(first_track.audio_id());
        let recommendations: Vec<Track> = client.get_recommendations(10, 0, target_audio).await?;

        println!("Подобранные рекомендации на основе этого трека (10 шт):");
        for (i, track) in recommendations.iter().enumerate() {
            println!("{}. {} - {}", i + 1, track.artist, track.title);
        }
    } else {
        println!("У вас нет треков в библиотеке для подбора рекомендаций.");
    }

    Ok(())
}
