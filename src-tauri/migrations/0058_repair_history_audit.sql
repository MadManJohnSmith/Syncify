-- Migration 0058: Append-only Repair History and Audit Log (S163)
-- Created: 2026-08-20

CREATE TABLE IF NOT EXISTS repair_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    repair_id TEXT NOT NULL UNIQUE,
    timestamp TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    download_id INTEGER,
    old_track_id INTEGER,
    new_track_id INTEGER,
    old_path TEXT NOT NULL,
    new_path TEXT NOT NULL,
    input_file_hash TEXT NOT NULL,
    output_file_hash TEXT,
    audio_payload_hash_before TEXT,
    audio_payload_hash_after TEXT,
    baseline_validation TEXT NOT NULL,
    actions TEXT NOT NULL,
    rollback_state TEXT,
    provenance TEXT NOT NULL,
    result TEXT NOT NULL,
    details_json TEXT
);

CREATE INDEX IF NOT EXISTS idx_repair_history_timestamp ON repair_history(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_repair_history_download_id ON repair_history(download_id);
