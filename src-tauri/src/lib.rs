#![allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::field_reassign_with_default,
    clippy::needless_update,
    clippy::suspicious_open_options,
    clippy::if_same_then_else,
    clippy::manual_clamp,
    clippy::redundant_pattern_matching,
    clippy::doc_lazy_continuation,
    clippy::should_implement_trait,
    clippy::assertions_on_constants
)]
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
pub mod logs_history;
pub mod models;
pub mod services;
pub mod sync_scheduler;
pub mod system_conditions;
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
