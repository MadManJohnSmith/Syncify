//! Syncify Tauri Library
//!
//! Library crate for the Tauri application.

pub mod commands;
pub mod crypto;
pub mod db;
pub mod download;
pub mod downloader;
pub mod import_cache;
pub mod models;
pub mod services;
pub mod worker;

use db::DbPool;
use std::sync::Arc;
use tokio::sync::Mutex;
use worker::DownloadWorkerState;

/// Lock for serializing album/artist creation across parallel imports
/// This is fast (microseconds) compared to database locks (seconds)
pub type AlbumCreationLock = Arc<Mutex<()>>;

/// Application state shared across commands
pub struct AppState {
    pub db: DbPool,
    pub worker_state: DownloadWorkerState,
    pub album_lock: AlbumCreationLock,
}
