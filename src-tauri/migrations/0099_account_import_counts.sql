-- Provider-reported pagination totals are per account and collection, not unique tracks.
-- Snapshots remain partial until the last page is observed and persisted.
CREATE TABLE account_import_counts (
    account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    collection TEXT NOT NULL CHECK (collection IN ('favorites', 'albums', 'playlists')),
    reported_total INTEGER CHECK (reported_total >= 0),
    imported_total INTEGER NOT NULL DEFAULT 0 CHECK (imported_total >= 0),
    scanned_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completeness TEXT NOT NULL CHECK (completeness IN ('partial', 'complete', 'error')),
    PRIMARY KEY (account_id, collection)
);
CREATE INDEX idx_account_import_counts_collection ON account_import_counts(collection, account_id);
