-- Migration 0102: Download jobs, delivery entitlements and preferred accounts (D00, spec #111)
-- Esquema aditivo de descargas: solo tablas nuevas; ninguna tabla existente se
-- toca aquí (las columnas de download_queue/downloads van en 0103).
-- Decisiones de ADR-0005: evidencia por cuenta con caducidad, preferencia de
-- cuenta blanda, sin secretos ni comportamiento nuevo en este ticket.

CREATE TABLE download_jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL CHECK(kind IN ('favorites', 'album', 'artist', 'playlist', 'selection', 'url', 'single')),
    label TEXT,
    status TEXT NOT NULL DEFAULT 'running' CHECK(status IN ('running', 'completed', 'partial', 'cancelled', 'failed')),
    source_service TEXT,
    destination_service TEXT,
    preferred_account_id INTEGER REFERENCES accounts(id) ON DELETE SET NULL,
    quality_preference TEXT CHECK(quality_preference IN ('hires', 'lossless', 'high', 'any') OR quality_preference IS NULL),
    allow_fallback INTEGER NOT NULL DEFAULT 1 CHECK(allow_fallback IN (0, 1)),
    filter_json TEXT,
    requested_count INTEGER NOT NULL DEFAULT 0 CHECK(requested_count >= 0),
    estimated_count INTEGER NOT NULL DEFAULT 0 CHECK(estimated_count >= 0),
    completed_count INTEGER NOT NULL DEFAULT 0 CHECK(completed_count >= 0),
    failed_count INTEGER NOT NULL DEFAULT 0 CHECK(failed_count >= 0),
    skipped_count INTEGER NOT NULL DEFAULT 0 CHECK(skipped_count >= 0),
    skip_breakdown_json TEXT,
    error TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    started_at TEXT,
    completed_at TEXT
);

CREATE INDEX idx_download_jobs_status ON download_jobs(status);
CREATE INDEX idx_download_jobs_kind ON download_jobs(kind);

CREATE TABLE download_entitlements (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    service_name TEXT NOT NULL,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    state TEXT NOT NULL CHECK(state IN ('granted', 'denied', 'unknown', 'stale')),
    quality TEXT,
    source TEXT CHECK(source IN ('delivered', 'probe', 'sync') OR source IS NULL),
    last_delivered_at TEXT,
    checked_at TEXT,
    expires_at TEXT,
    error_class TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(track_id, service_name, account_id)
);

CREATE INDEX idx_download_entitlements_track_service ON download_entitlements(track_id, service_name);
CREATE INDEX idx_download_entitlements_account_expiry ON download_entitlements(account_id, expires_at);

CREATE TABLE service_preferred_accounts (
    service_name TEXT PRIMARY KEY,
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
