-- 0109_player_queue.sql — #131 (P00, parte de #130): el backend pasa a ser
-- dueño de la cola, el historial, la pista actual, la posición y los modos de
-- reproducción. Dos tablas nuevas (ninguna columna de tablas existentes):
--
--   player_queue_entries  entradas con identidad propia (entry_id UUID): la
--                         cola pendiente, el historial y la pista actual viven
--                         como filas diferenciadas por `kind`
--   player_state          fila única (id = 1) con posición y modos
--
-- Nota del issue: la cola vieja de localStorage NO se migra — se descarta
-- (la UI elimina syncify.playerState.v1 en su limpieza). Las entradas son
-- tracks de biblioteca; las entradas efímeras de proveedor llegan con sus
-- sub-issues de #130 y no crean Track al encolarse (esas columnas vendrán
-- entonces, nunca editando esta migración aplicada).

CREATE TABLE player_queue_entries (
    entry_id TEXT PRIMARY KEY,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    kind TEXT NOT NULL DEFAULT 'pending'
        CHECK(kind IN ('pending', 'history', 'current')),
    position INTEGER NOT NULL CHECK(position >= 0),
    enqueued_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- La cola pendiente se recorre por orden; el historial se añade en orden de
-- reproducción y se recorta por el extremo más antiguo (límite 100, en el
-- comando).
CREATE INDEX idx_player_queue_entries_kind_position
    ON player_queue_entries(kind, position);

-- Invariante del agregado: como mucho una fila con kind 'current' (el índice
-- único parcial convierte la violación en error de constraint, no en silencio).
CREATE UNIQUE INDEX idx_player_queue_entries_single_current
    ON player_queue_entries(kind) WHERE kind = 'current';

CREATE TABLE player_state (
    id INTEGER PRIMARY KEY CHECK(id = 1),
    position_sec REAL NOT NULL DEFAULT 0 CHECK(position_sec >= 0),
    shuffle INTEGER NOT NULL DEFAULT 0 CHECK(shuffle IN (0, 1)),
    repeat TEXT NOT NULL DEFAULT 'off' CHECK(repeat IN ('off', 'all', 'one')),
    volume REAL NOT NULL DEFAULT 1 CHECK(volume >= 0 AND volume <= 1),
    muted INTEGER NOT NULL DEFAULT 0 CHECK(muted IN (0, 1)),
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO player_state (id) VALUES (1);
