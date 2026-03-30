-- Migration 0031_add_credentials_invalid.sql
-- Adds safe invalidation flag instead of deleting stale accounts

ALTER TABLE accounts ADD COLUMN credentials_invalid INTEGER DEFAULT 0;
