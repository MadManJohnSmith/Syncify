-- 0042: Add qobuz_id to albums table and unique index
ALTER TABLE albums ADD COLUMN qobuz_id TEXT;
CREATE UNIQUE INDEX idx_albums_qobuz_id ON albums(qobuz_id) WHERE qobuz_id IS NOT NULL;
