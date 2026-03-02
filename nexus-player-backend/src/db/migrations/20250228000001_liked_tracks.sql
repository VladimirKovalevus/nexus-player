ALTER TABLE tracks_v2 ADD COLUMN liked INTEGER NOT NULL DEFAULT 0;
ALTER TABLE tracks_v2 ADD COLUMN liked_position INTEGER;

-- Like all existing tracks (they were in Liked Songs)
UPDATE tracks_v2 SET liked = 1;
UPDATE tracks_v2 SET liked_position = (
    SELECT COUNT(*) - 1 FROM tracks_v2 t2 WHERE t2.id <= tracks_v2.id
);

-- Remove stale Liked Songs playlist
DELETE FROM playlist_items_v2
    WHERE playlist_id = (SELECT id FROM playlists_v2 WHERE title = 'Liked Songs');
DELETE FROM playlists_v2 WHERE title = 'Liked Songs';
