-- 0106_sync_runs.sql — ADR-0006 (#64/#65): el trabajo de sincronización pasa a
-- ser un agregado SyncRun con identidad, estado, progreso, cancelación e
-- historial. Tres tablas nuevas (ninguna columna de tablas existentes):
--
--   sync_runs            una fila por corrida (estado de la ejecución)
--   sync_run_collections cobertura por colección (reportado frente a importado,
--                        o desconocida si el proveedor no publica total)
--   sync_run_errors      los huecos registrados (una página que persiste el
--                        fallo se registra y la corrida continúa)
--
-- El estado de la corrida (completed/partial/failed/cancelled) y la cobertura
-- son dos ejes que nunca se mezclan (decisión 5 del ADR-0006).

CREATE TABLE sync_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    service_id INTEGER NOT NULL REFERENCES services(id),
    mode TEXT NOT NULL DEFAULT 'import_only'
        CHECK(mode IN ('import_only', 'mirror_quarantine', 'strict_mirror', 'bidirectional')),
    scope TEXT NOT NULL DEFAULT 'favorites'
        CHECK(scope IN ('favorites', 'playlists', 'albums', 'artists')),
    status TEXT NOT NULL DEFAULT 'running'
        CHECK(status IN ('running', 'completed', 'partial', 'failed', 'cancelled')),
    started_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    finished_at TEXT,
    reported_total INTEGER,
    imported_total INTEGER NOT NULL DEFAULT 0,
    error_count INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_sync_runs_account_started ON sync_runs(account_id, started_at DESC);

CREATE TABLE sync_run_collections (
    run_id INTEGER NOT NULL REFERENCES sync_runs(id) ON DELETE CASCADE,
    collection TEXT NOT NULL
        CHECK(collection IN ('favorites', 'playlists', 'albums', 'artists')),
    reported INTEGER,
    imported INTEGER NOT NULL DEFAULT 0,
    coverage TEXT NOT NULL DEFAULT 'unknown'
        CHECK(coverage IN ('reported', 'imported', 'unknown')),
    PRIMARY KEY (run_id, collection)
);

CREATE TABLE sync_run_errors (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id INTEGER NOT NULL REFERENCES sync_runs(id) ON DELETE CASCADE,
    collection TEXT,
    message TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
