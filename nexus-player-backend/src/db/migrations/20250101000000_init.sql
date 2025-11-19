-- Первая миграция: базовая схема для библиотеки треков
-- SQLx автоматически применяет миграции в порядке имени файла

-- ============================================================================
-- Таблица обложек/изображений (artwork)
-- Используется и треками, и плейлистами, и альбомами
-- ============================================================================
CREATE TABLE IF NOT EXISTS artworks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    -- Изображение может быть либо локальным (data), либо внешним (url)
    data BLOB,              -- Локальное изображение (binary data)
    url TEXT,               -- URL внешнего изображения

    -- Метаинформация
    mime_type TEXT,         -- 'image/jpeg', 'image/png', 'image/webp'
    width INTEGER,          -- Ширина в пикселях
    height INTEGER,         -- Высота в пикселях

    created_at INTEGER NOT NULL,

    -- Constraint: хотя бы одно из полей должно быть заполнено
    CHECK ((data IS NOT NULL) OR (url IS NOT NULL))
);

-- Индекс для поиска по URL (для deduplicate)
CREATE INDEX IF NOT EXISTS idx_artworks_url ON artworks(url);

-- ============================================================================
-- Таблица треков
-- ============================================================================
CREATE TABLE IF NOT EXISTS tracks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    -- === Метаданные (Metadata struct в Rust) ===
    title TEXT NOT NULL,
    artist TEXT,            -- Исполнитель (creator в общей терминологии)
    description TEXT,
    artwork_id INTEGER REFERENCES artworks(id) ON DELETE SET NULL,

    -- === Дополнительные метаданные ===
    album TEXT,
    album_artist TEXT,      -- Может отличаться от artist (сборники)
    year INTEGER,
    genre TEXT,
    track_number INTEGER,   -- Номер трека в альбоме

    -- === Источник трека (TrackSource enum) ===
    source_type TEXT NOT NULL CHECK (source_type IN ('local', 'http', 'external')),

    -- Local source
    source_local_path TEXT,

    -- HTTP source
    source_http_url TEXT,

    -- External source (YouTube, SoundCloud, Bandcamp, etc.)
    source_external_system TEXT,  -- 'youtube', 'soundcloud', 'bandcamp'
    source_external_id TEXT,      -- ID трека в системе

    -- === Кеширование (для external/http треков) ===
    cached_local_path TEXT,                                                  -- Путь к кешированному файлу
    cache_status TEXT DEFAULT 'none' CHECK (cache_status IN ('none', 'downloading', 'cached', 'failed')),
    cache_downloaded_at INTEGER,                                             -- Когда был кеширован

    -- === Технические параметры аудио ===
    file_format TEXT,       -- 'mp3', 'flac', 'ogg', 'opus', 'aac'
    duration_secs REAL,     -- Длительность в секундах
    sample_rate INTEGER,    -- Частота дискретизации (44100, 48000, etc.)
    channels INTEGER,       -- Количество каналов (1=mono, 2=stereo)
    bitrate INTEGER,        -- Битрейт в kbps

    -- === Статистика использования ===
    play_count INTEGER NOT NULL DEFAULT 0,
    last_played INTEGER,    -- Unix timestamp последнего воспроизведения

    -- === Timestamps ===
    added_at INTEGER NOT NULL,      -- Когда добавлен в библиотеку
    updated_at INTEGER NOT NULL     -- Последнее обновление метаданных
);

-- ============================================================================
-- Таблица плейлистов
-- ============================================================================
CREATE TABLE IF NOT EXISTS playlists (
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    -- === Метаданные (Metadata struct в Rust) ===
    title TEXT NOT NULL,
    description TEXT,
    creator TEXT,           -- Кто создал (для внешних плейлистов)
    artwork_id INTEGER REFERENCES artworks(id) ON DELETE SET NULL,

    -- === Timestamps ===
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- ============================================================================
-- Связь Many-to-Many между плейлистами и треками
-- ============================================================================
CREATE TABLE IF NOT EXISTS playlist_tracks (
    playlist_id INTEGER NOT NULL,
    track_id INTEGER NOT NULL,

    position INTEGER NOT NULL,      -- Порядок трека в плейлисте (0, 1, 2, ...)
    added_at INTEGER NOT NULL,      -- Когда трек добавлен в плейлист

    PRIMARY KEY (playlist_id, track_id),
    FOREIGN KEY (playlist_id) REFERENCES playlists(id) ON DELETE CASCADE,
    FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE
);

-- ============================================================================
-- Индексы для быстрого поиска
-- ============================================================================

-- Поиск треков по метаданным
CREATE INDEX IF NOT EXISTS idx_tracks_title ON tracks(title);
CREATE INDEX IF NOT EXISTS idx_tracks_artist ON tracks(artist);
CREATE INDEX IF NOT EXISTS idx_tracks_album ON tracks(album);
CREATE INDEX IF NOT EXISTS idx_tracks_genre ON tracks(genre);

-- Поиск по источнику
CREATE INDEX IF NOT EXISTS idx_tracks_source_type ON tracks(source_type);
CREATE INDEX IF NOT EXISTS idx_tracks_source_external ON tracks(source_external_system, source_external_id);

-- Поиск по статистике
CREATE INDEX IF NOT EXISTS idx_tracks_added_at ON tracks(added_at);
CREATE INDEX IF NOT EXISTS idx_tracks_play_count ON tracks(play_count DESC);
CREATE INDEX IF NOT EXISTS idx_tracks_last_played ON tracks(last_played DESC);

-- Индексы для плейлистов
CREATE INDEX IF NOT EXISTS idx_playlist_tracks_playlist ON playlist_tracks(playlist_id);
CREATE INDEX IF NOT EXISTS idx_playlist_tracks_position ON playlist_tracks(playlist_id, position);
