ALTER TABLE local_media_index ADD COLUMN position INTEGER;

WITH ranked AS (
    SELECT track_id, ROW_NUMBER() OVER (ORDER BY track_id DESC) - 1 AS new_position
    FROM local_media_index
)
UPDATE local_media_index
SET position = (SELECT new_position FROM ranked WHERE ranked.track_id = local_media_index.track_id);

CREATE INDEX idx_local_media_position ON local_media_index(position);
