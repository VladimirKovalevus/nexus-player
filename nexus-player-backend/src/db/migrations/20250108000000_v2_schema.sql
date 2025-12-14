-- V2 Schema Migration: SeaORM with JSON columns
-- Упрощенная схема с JSON для flexibility и меньшим количеством nullable колонок

-- ============================================================================
-- Таблица обложек/изображений V2
-- ============================================================================
CREATE TABLE IF NOT EXISTS artworks_v2 (
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    source_type TEXT NOT NULL CHECK (source_type IN ('data', 'url')),
    source_data TEXT,

    width INTEGER,
    height INTEGER,

    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_artworks_v2_source_type ON artworks_v2(source_type);

-- ============================================================================
-- Таблица треков V2
-- ============================================================================
CREATE TABLE IF NOT EXISTS tracks_v2 (
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    title TEXT NOT NULL,
    artist TEXT,
    description TEXT,
    artwork_id INTEGER REFERENCES artworks_v2(id) ON DELETE SET NULL,

    album TEXT,
    album_artist TEXT,
    year INTEGER,
    genre TEXT,
    track_number INTEGER,

    source_type TEXT NOT NULL CHECK (source_type IN ('local', 'http', 'external', 'cached')),
    source_data TEXT,

    cache_type TEXT NOT NULL DEFAULT 'none' CHECK (cache_type IN ('none', 'downloading', 'cached', 'failed')),
    cache_data TEXT,

    file_format TEXT,
    duration_secs REAL,
    sample_rate INTEGER,
    channels INTEGER,
    bitrate INTEGER,

    play_count INTEGER NOT NULL DEFAULT 0,
    last_played INTEGER,

    added_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Индексы для поиска
CREATE INDEX IF NOT EXISTS idx_tracks_v2_title ON tracks_v2(title);
CREATE INDEX IF NOT EXISTS idx_tracks_v2_artist ON tracks_v2(artist);
CREATE INDEX IF NOT EXISTS idx_tracks_v2_album ON tracks_v2(album);
CREATE INDEX IF NOT EXISTS idx_tracks_v2_genre ON tracks_v2(genre);
CREATE INDEX IF NOT EXISTS idx_tracks_v2_source_type ON tracks_v2(source_type);
CREATE INDEX IF NOT EXISTS idx_tracks_v2_added_at ON tracks_v2(added_at);
CREATE INDEX IF NOT EXISTS idx_tracks_v2_play_count ON tracks_v2(play_count DESC);
CREATE INDEX IF NOT EXISTS idx_tracks_v2_last_played ON tracks_v2(last_played DESC);

-- ============================================================================
-- Таблица плейлистов V2
-- ============================================================================
CREATE TABLE IF NOT EXISTS playlists_v2 (
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    title TEXT NOT NULL,
    description TEXT,
    creator TEXT,
    artwork_id INTEGER REFERENCES artworks_v2(id) ON DELETE SET NULL,

    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- ============================================================================
-- Связь Many-to-Many между плейлистами и треками V2
-- ============================================================================
CREATE TABLE IF NOT EXISTS playlist_tracks_v2 (
    playlist_id INTEGER NOT NULL,
    track_id INTEGER NOT NULL,

    position INTEGER NOT NULL,
    added_at INTEGER NOT NULL,

    PRIMARY KEY (playlist_id, track_id),
    FOREIGN KEY (playlist_id) REFERENCES playlists_v2(id) ON DELETE CASCADE,
    FOREIGN KEY (track_id) REFERENCES tracks_v2(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_playlist_tracks_v2_playlist ON playlist_tracks_v2(playlist_id);
CREATE INDEX IF NOT EXISTS idx_playlist_tracks_v2_position ON playlist_tracks_v2(playlist_id, position);
