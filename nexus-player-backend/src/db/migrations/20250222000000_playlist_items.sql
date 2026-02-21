-- Replace playlist_tracks_v2 with playlist_items_v2 to support nested playlists
-- item_type: 'track' or 'playlist'

CREATE TABLE IF NOT EXISTS playlist_items_v2 (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    playlist_id INTEGER NOT NULL,
    item_type TEXT NOT NULL CHECK (item_type IN ('track', 'playlist')),
    item_id INTEGER NOT NULL,
    position INTEGER NOT NULL,
    added_at INTEGER NOT NULL,
    FOREIGN KEY (playlist_id) REFERENCES playlists_v2(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_playlist_items_v2_unique
    ON playlist_items_v2(playlist_id, item_type, item_id);
CREATE INDEX IF NOT EXISTS idx_playlist_items_v2_position
    ON playlist_items_v2(playlist_id, position);

-- Migrate existing data
INSERT INTO playlist_items_v2 (playlist_id, item_type, item_id, position, added_at)
SELECT playlist_id, 'track', track_id, position, added_at FROM playlist_tracks_v2;

DROP TABLE playlist_tracks_v2;
