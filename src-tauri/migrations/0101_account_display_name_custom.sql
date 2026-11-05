-- Account identity: user-chosen display name flag.
--
-- The accounts surface distinguishes accounts by email first and display name
-- second. Services that cannot reveal an identity (Apple Music) or a provider
-- outage leave the generic fallback ("Qobuz User") on several rows, so the
-- user can rename an account by hand. This flag marks that decision: the
-- login upsert and the automatic identity refresh never overwrite a custom
-- name, while an email discovered later is still stored (identity, not label).

ALTER TABLE accounts ADD COLUMN display_name_custom INTEGER NOT NULL DEFAULT 0;
