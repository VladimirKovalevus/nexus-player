CREATE TABLE local_music_sources (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE,
    is_default INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);

CREATE TABLE local_media_index (
    track_id INTEGER PRIMARY KEY NOT NULL,
    source_key TEXT NOT NULL UNIQUE,
    source_id INTEGER,
    file_size INTEGER,
    modified_at INTEGER,
    last_seen_scan INTEGER NOT NULL,
    unavailable INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY(track_id) REFERENCES tracks_v2(id) ON DELETE CASCADE,
    FOREIGN KEY(source_id) REFERENCES local_music_sources(id) ON DELETE SET NULL
);

CREATE INDEX idx_local_media_source ON local_media_index(source_id);
CREATE INDEX idx_local_media_seen ON local_media_index(last_seen_scan);
