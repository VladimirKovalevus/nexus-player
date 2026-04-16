use thiserror::Error;

#[derive(Error, Debug)]
pub enum StreamError {
    #[error("Не найден подходящий провайдер для источника")]
    NoProviderFound,

    #[error("Неподдерживаемый источник: {0}")]
    InvalidSource(String),

    #[error("Stream недоступен: {0}")]
    NoStreamAvailable(String),

    #[error("HTTP ошибка: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("IO ошибка: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Функция не поддерживается")]
    NotSupported,

    #[error("Ошибка парсинга метаданных: {0}")]
    MetadataError(String),

    #[error("Ошибка аутентификации: {0}")]
    AuthenticationError(String),

    #[error("Превышен таймаут запроса")]
    Timeout,

    #[error("Трек недоступен: {0}")]
    Unavailable(String),

    #[error("Ошибка streaming: {0}")]
    Other(String),
}

pub type StreamResult<T> = Result<T, StreamError>;
