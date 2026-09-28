//! Syncify Tauri Library
//!
//! Library crate for the Tauri application.

pub mod cmd_utils;
pub mod commands;
pub mod crypto;
pub mod db;
pub mod download;
pub mod enrichment_worker;
pub mod import_cache;
pub mod models;
pub mod services;
pub mod tray;
pub mod worker;

use db::DbPool;
pub use enrichment_worker::EnrichmentWorkerState;
use std::sync::Arc;
use worker::DownloadWorkerState;

/// Application state shared across commands
pub struct AppState {
    pub db: DbPool,
    pub worker_state: DownloadWorkerState,
    pub enrichment_state: EnrichmentWorkerState,
    pub concurrency_manager: Arc<services::ConcurrencyManager>,
}
