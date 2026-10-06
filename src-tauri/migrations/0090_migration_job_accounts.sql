-- Record the selected accounts on migration jobs; historical jobs remain NULL.
-- Account removal keeps job history through ON DELETE SET NULL.
ALTER TABLE migration_jobs ADD COLUMN source_account_id INTEGER REFERENCES accounts(id) ON DELETE SET NULL;
ALTER TABLE migration_jobs ADD COLUMN destination_account_id INTEGER REFERENCES accounts(id) ON DELETE SET NULL;
