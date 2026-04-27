-- 0043_add_is_purchased_and_qobuz_id.sql
ALTER TABLE library_entries ADD COLUMN is_purchased INTEGER NOT NULL DEFAULT 0;
ALTER TABLE tracks ADD COLUMN qobuz_id TEXT;
