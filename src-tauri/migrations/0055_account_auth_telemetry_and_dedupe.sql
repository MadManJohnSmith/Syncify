-- 0055_account_auth_telemetry_and_dedupe.sql
-- S143: Add last_auth_error_at and last_auth_checked_at to accounts table

ALTER TABLE accounts ADD COLUMN last_auth_error_at TEXT;
ALTER TABLE accounts ADD COLUMN last_auth_checked_at TEXT;
