use vk_api::auth;

fn main() -> anyhow::Result<()> {
    println!("Тестирование сохранения токена в .env файл...\n");

    let test_token = "new_test_token_67890";

    println!("Сохраняем токен: {}", test_token);
    auth::save_token_to_env(test_token)?;
    println!("✓ Токен сохранен в .env\n");

    println!("Проверяем загрузку сохраненного токена...");
    match auth::load_token_from_env() {
        Some(token) => {
            if token == test_token {
                println!("✓ Токен успешно загружен и совпадает!");
                println!("  Загруженный токен: {}", token);
            } else {
                println!("✗ Токены не совпадают!");
                println!("  Ожидалось: {}", test_token);
                println!("  Получено:  {}", token);
            }
        }
        None => {
            println!("✗ Не удалось загрузить токен из .env");
        }
    }

    Ok(())
}
