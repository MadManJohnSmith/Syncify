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
pub mod worker;

use db::DbPool;
use std::sync::Arc;
use worker::DownloadWorkerState;
pub use enrichment_worker::EnrichmentWorkerState;

/// Application state shared across commands
pub struct AppState {
    pub db: DbPool,
    pub worker_state: DownloadWorkerState,
    pub enrichment_state: EnrichmentWorkerState,
    pub concurrency_manager: Arc<services::ConcurrencyManager>,
}
