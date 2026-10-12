-- 0095: Qobuz streaming entitlement, per account.
--
-- A Qobuz account can authenticate perfectly and still be on a plan without
-- streaming. For those accounts `track/getFileUrl` answers with a valid URL —
-- for a 30-second preview instead of the full track. The download path used to
-- take the first account whose credentials resolved, so an unentitled account
-- could be picked even when another account on the same machine had a
-- subscription.
--
-- Entitlement is stored per account and read from the `credential` block of
-- `user/getUserData` (`lossless_streaming`, `hires_streaming`,
-- `hires_purchases_streaming`), the same signal the Python bridge already uses
-- to gate quality selection.
--
-- Three states matter, and the middle one is the compatibility case:
--   1  = this account can stream
--   0  = authenticated but NOT entitled to streaming
--  -1 = unknown, never determined. Accounts created before this migration and
--       accounts whose provider response lacked the credential block stay -1.
--       A -1 account is NEVER excluded: the safe fallback for "we don't know" is
--       to try it, not to silently drop a working account.

ALTER TABLE accounts ADD COLUMN streaming_entitled INTEGER NOT NULL DEFAULT -1;

ALTER TABLE accounts ADD COLUMN entitlement_checked_at TEXT;
