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
 
 
 
