-- Migration 0103: job and account columns on the persistent queue and ledger (D00, spec #111)
-- Columnas aditivas sobre download_queue y downloads; las filas legacy quedan
-- con job_id y account_id NULL. El CHECK de status de download_queue no cambia.

ALTER TABLE download_queue ADD COLUMN job_id INTEGER REFERENCES download_jobs(id) ON DELETE SET NULL;
ALTER TABLE download_queue ADD COLUMN next_attempt_at TEXT;
ALTER TABLE download_queue ADD COLUMN failure_class TEXT;
ALTER TABLE download_queue ADD COLUMN resolved_account_id INTEGER REFERENCES accounts(id) ON DELETE SET NULL;

ALTER TABLE downloads ADD COLUMN account_id INTEGER REFERENCES accounts(id) ON DELETE SET NULL;

CREATE INDEX idx_download_queue_job ON download_queue(job_id);
CREATE INDEX idx_download_queue_next_attempt ON download_queue(next_attempt_at);
CREATE INDEX idx_downloads_account ON downloads(account_id);
