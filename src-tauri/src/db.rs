//! Database module for SQLite connection and queries
// RECOMPILE_TIMESTAMP: 2026-04-26 15:10

use sqlx::{sqlite::SqlitePoolOptions, Executor, Pool, Sqlite};
use std::path::PathBuf;
use tauri::Manager;

/// Database connection pool
pub type DbPool = Pool<Sqlite>;

/// Initialize the database connection pool
pub async fn init_db(app_handle: &tauri::AppHandle) -> Result<DbPool, sqlx::Error> {
    let db_path = get_db_path(app_handle).await;
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    tracing::info!("Connecting to database: {}", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(10) // Increase pool size for concurrent operations
        .acquire_timeout(std::time::Duration::from_secs(10)) // Timeout on pool acquire
        // Enable foreign key enforcement on EVERY connection
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                conn.execute("PRAGMA foreign_keys = ON;").await?;
                conn.execute("PRAGMA journal_mode = WAL;").await?; // Better concurrency
                conn.execute("PRAGMA busy_timeout = 30000;").await?; // 30 second timeout for parallel imports
                conn.execute("PRAGMA wal_autocheckpoint = 1000;").await?; // Checkpoint every 1000 pages
                tracing::debug!("SQLite pragmas enabled");
                Ok(())
            })
        })
        .connect(&db_url)
        .await?;

    // Run migrations if needed
    sqlx::migrate!("./migrations").run(&pool).await?;

    tracing::info!("Database initialized successfully");
    Ok(pool)
}

/// Classifies SQLite "database is locked" failures (SQLITE_BUSY family, code 5) out of
/// stringified `sqlx::Error`s.
///
/// S195(c) — why this error class can STILL surface even though every pooled connection
/// runs `PRAGMA journal_mode = WAL` + `PRAGMA busy_timeout = 30000` (see `init_db`):
/// SQLite does NOT invoke the busy handler for `SQLITE_BUSY_SNAPSHOT`. That is exactly
/// what a DEFERRED transaction (`sqlx` `db.begin()`) gets when it reads first and writes
/// later while another writer commits in between — its read snapshot can no longer be
/// upgraded, so the statement fails immediately regardless of busy_timeout.
/// During a library import this races for real: the background `EnrichmentWorker`
/// (upserts into `enrichment_progress`, `UPDATE tracks SET enrichment_status = ...`)
/// and other pool writers interleave with import-time catalog upserts
/// (`enrich_and_persist_sync_track`: `BEGIN` → `SELECT artists` → `INSERT artists ...`).
/// A failed transaction rolls back completely, so retrying the WHOLE operation after a
/// short backoff is safe and removes the entire error class; see
/// `commands::service::enrich_persist_with_locked_retry`.
pub fn is_sqlite_locked_error(err: &str) -> bool {
    err.contains("database is locked")
        || err.contains("database table is locked")
        || err.contains("SQLITE_BUSY")
        || err.contains("(code: 5)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s195_classifies_sqlite_locked_errors() {
        // Exact production signature from the S195 owner report (1 failure / ~8,974):
        assert!(is_sqlite_locked_error(
            "error returned from database: (code: 5) database is locked"
        ));
        assert!(is_sqlite_locked_error("database table is locked"));
        assert!(is_sqlite_locked_error("SQLITE_BUSY: pool busy"));

        // Non-locked failures must NOT be retried by callers of this classifier.
        assert!(!is_sqlite_locked_error(
            "error returned from database: (code: 2067) UNIQUE constraint failed: artists.name"
        ));
        assert!(!is_sqlite_locked_error("Failed to insert artist 'X': column null"));
    }
}

/// Get the database file path, creating directories and migrating if necessary
pub async fn get_db_path(app_handle: &tauri::AppHandle) -> PathBuf {
    let db_dir = app_handle
        .path()
        .app_local_data_dir()
        .expect("No app local data dir available");
        
    tokio::fs::create_dir_all(&db_dir)
        .await
        .ok();
        
    let new_db_path = db_dir.join("syncify.db");
    
    // Migration logic from legacy CWD/exe-based path to OS-native app data path
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()));
        
    let old_db_path = exe_dir.as_ref().map(|d| d.join("data").join("syncify.db"));
    
    if let Some(old_path) = old_db_path {
        if old_path.exists() && !new_db_path.exists() {
            tracing::info!("Migrating pre-existing legacy database to OS app data directory...");
            if let Err(e) = tokio::fs::rename(&old_path, &new_db_path).await {
                tracing::error!("Failed to migrate database (old path: {}): {}", old_path.display(), e);
            } else {
                tracing::info!("Database successfully migrated to {}", new_db_path.display());
                
                // Attempt to move WAL and SHM files if they exist
                if let Some(parent) = exe_dir {
                    let old_wal = parent.join("data").join("syncify.db-wal");
                    let old_shm = parent.join("data").join("syncify.db-shm");
                    let new_wal = db_dir.join("syncify.db-wal");
                    let new_shm = db_dir.join("syncify.db-shm");
                    let _ = tokio::fs::rename(&old_wal, &new_wal).await;
                    let _ = tokio::fs::rename(&old_shm, &new_shm).await;
                }
            }
        }
    }
    
    new_db_path
}
 
 
 
