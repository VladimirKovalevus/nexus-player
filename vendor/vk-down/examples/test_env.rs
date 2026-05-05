use vk_api::auth;

fn main() {
    println!("Тестирование загрузки токена из .env файла...\n");

    match auth::load_token_from_env() {
        Some(token) => {
            println!("✓ Токен успешно загружен из .env:");
            println!("  {}", token);
        }
        None => {
            println!("✗ Токен не найден в .env файле");
            println!("  Создайте файл .env с содержимым:");
            println!("  VK_ACCESS_TOKEN=ваш_токен");
        }
    }
}
