use thiserror::Error;

pub type Result<T> = anyhow::Result<T>;

#[derive(Error, Debug)]
pub enum AudioError {
    #[error("Не удалось декодировать аудио: {0}")]
    DecodingError(String),

    #[error("Не удалось инициализировать аудио выход: {0}")]
    OutputError(String),

    #[error("Неподдерживаемый формат аудио")]
    UnsupportedFormat,

    #[error("Не удалось построить аудио поток: {0}")]
    StreamBuildError(String),

    #[error("Ошибка ресемплинга: {0}")]
    ResamplingError(String),

    #[error("Аудио поток недоступен")]
    StreamUnavailable,

    #[error("Данные ещё не доступны")]
    InProgress,

    #[error("Ошибка чтения аудио файла: {0}")]
    IoError(#[from] std::io::Error),
}
