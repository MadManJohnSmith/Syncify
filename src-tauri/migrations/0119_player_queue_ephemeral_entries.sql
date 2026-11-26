-- Migration 0119 (issue #134, ADR-0010 decision 2/3): entradas efímeras.
--
-- Una entrada de cola es exactamente una de estas dos identidades:
--   * biblioteca: track_id;
--   * proveedor:  service_id + service_track_id.
-- Encolar una entrada de proveedor no crea Track ni TrackSource. La promoción
-- la convierte en biblioteca solo después de 30 s reales de escucha.
--
-- `listen_session_id` congela la primera escucha que cruzó ese umbral. Si la
-- identidad no se resuelve, la entrada sigue efímera y `promotion_error` guarda
-- el motivo; un reintento usa el mismo token y `playback_listens` deduplica el
-- crédito aunque la llamada llegue con otra sesión.

CREATE TABLE player_queue_entries_0119_new (
    entry_id TEXT PRIMARY KEY,
    track_id INTEGER REFERENCES tracks(id) ON DELETE CASCADE,
    service_id INTEGER REFERENCES services(id),
    service_track_id TEXT,
    kind TEXT NOT NULL DEFAULT 'pending'
        CHECK(kind IN ('pending', 'history', 'current')),
    position INTEGER NOT NULL CHECK(position >= 0),
    enqueued_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    listen_session_id TEXT,
    promotion_error TEXT,
    CHECK (
        (track_id IS NOT NULL AND service_id IS NULL AND service_track_id IS NULL)
        OR
        (track_id IS NULL AND service_id IS NOT NULL
         AND service_track_id IS NOT NULL AND length(trim(service_track_id)) > 0)
    )
);

INSERT INTO player_queue_entries_0119_new
    (entry_id, track_id, kind, position, enqueued_at)
SELECT entry_id, track_id, kind, position, enqueued_at
FROM player_queue_entries;

DROP TABLE player_queue_entries;

ALTER TABLE player_queue_entries_0119_new RENAME TO player_queue_entries;

CREATE INDEX idx_player_queue_entries_kind_position
    ON player_queue_entries(kind, position);

CREATE UNIQUE INDEX idx_player_queue_entries_single_current
    ON player_queue_entries(kind) WHERE kind = 'current';
