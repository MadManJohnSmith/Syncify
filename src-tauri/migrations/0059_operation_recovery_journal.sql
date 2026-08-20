-- Migration 0059: Persistent Operation Journal and Crash Recovery Protocol (S167)
-- Created: 2026-08-20

CREATE TABLE IF NOT EXISTS operation_journal (
    operation_id TEXT PRIMARY KEY,
    operation_type TEXT NOT NULL,
    entity_id TEXT,
    account_id INTEGER,
    track_id INTEGER,
    download_id INTEGER,
    provider TEXT,
    phase TEXT NOT NULL,
    attempt INTEGER NOT NULL DEFAULT 1,
    started_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    checkpoint_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    status TEXT NOT NULL,
    input_identity TEXT,
    expected_output_path TEXT,
    staging_path TEXT,
    file_baseline TEXT,
    db_transaction_state TEXT,
    rollback_state TEXT,
    error_taxonomy TEXT,
    retry_policy TEXT,
    result_summary TEXT
);

CREATE INDEX IF NOT EXISTS idx_op_journal_status ON operation_journal(status);
CREATE INDEX IF NOT EXISTS idx_op_journal_type ON operation_journal(operation_type);
CREATE INDEX IF NOT EXISTS idx_op_journal_checkpoint ON operation_journal(checkpoint_at DESC);
CREATE INDEX IF NOT EXISTS idx_op_journal_entity ON operation_journal(entity_id);

CREATE TABLE IF NOT EXISTS operation_recovery_audit (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    recovery_id TEXT NOT NULL UNIQUE,
    timestamp TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    operation_id TEXT NOT NULL,
    operation_type TEXT NOT NULL,
    previous_status TEXT NOT NULL,
    new_status TEXT NOT NULL,
    action_taken TEXT NOT NULL,
    error_taxonomy TEXT,
    message TEXT NOT NULL,
    details_json TEXT
);

CREATE INDEX IF NOT EXISTS idx_op_recovery_audit_timestamp ON operation_recovery_audit(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_op_recovery_audit_op_id ON operation_recovery_audit(operation_id);
