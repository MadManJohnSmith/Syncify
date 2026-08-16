-- Migration 0051_account_invalidation_reason.sql
-- Add invalid_reason and last_auth_error to accounts table for S109 session recovery

ALTER TABLE accounts ADD COLUMN invalid_reason TEXT;
ALTER TABLE accounts ADD COLUMN last_auth_error TEXT;

CREATE INDEX IF NOT EXISTS idx_accounts_service_invalid ON accounts(service_id, credentials_invalid);
