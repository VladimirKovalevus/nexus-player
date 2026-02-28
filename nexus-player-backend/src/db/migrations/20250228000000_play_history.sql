CREATE TABLE IF NOT EXISTS play_history (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    title       TEXT    NOT NULL,
    artist      TEXT,
    source_type TEXT    NOT NULL,
    source_data TEXT    NOT NULL,
    artwork_url TEXT,
    played_at   INTEGER NOT NULL
);
CREATE INDEX idx_play_history_played_at ON play_history (played_at DESC);
