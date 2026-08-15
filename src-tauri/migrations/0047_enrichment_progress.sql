-- Migration 0047: Enrichment Progress Tracking
-- Tracks per-track and per-service metadata enrichment state, retries, and errors

CREATE TABLE IF NOT EXISTS enrichment_progress (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    service TEXT NOT NULL CHECK(service IN ('musicbrainz', 'spotify', 'lastfm', 'all')),
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'in_progress', 'completed', 'failed', 'rate_limited')),
    retry_count INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    last_attempt TEXT,
    completed_at TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(track_id, service)
);

CREATE INDEX IF NOT EXISTS idx_enrichment_progress_status ON enrichment_progress(status, service);
CREATE INDEX IF NOT EXISTS idx_enrichment_progress_track ON enrichment_progress(track_id);
