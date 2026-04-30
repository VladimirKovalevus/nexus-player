use anyhow::Result;
use std::io::{self, Write};
use vk_api::{auth, VkClient};

#[tokio::main]
async fn main() -> Result<()> {
    println!("╔════════════════════════════════════════════════════════╗");
    println!("║     VK - Информация о пользователе (direct auth)       ║");
    println!("╚════════════════════════════════════════════════════════╝\n");

    let token = get_token().await?;
    let client = VkClient::new(token);

    println!("Получаем информацию о пользователе...\n");
    let user = client.get_me().await?;

    println!("╔════════════════════════════════════════════════════════╗");
    println!("║                   Ваш профиль                          ║");
    println!("╠════════════════════════════════════════════════════════╣");
    println!("║ ID:   {:<49} ║", user.id);
    println!("║ Имя:  {:<49} ║", user.full_name());
    println!("╚════════════════════════════════════════════════════════╝\n");

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
