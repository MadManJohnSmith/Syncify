//! Syncify Tauri Commands - organized via include!() macro

pub mod types;
pub use types::*;

use crate::import_cache::ImportCache;
use crate::services::{ImportResult, SpotifyClient, SpotifyConfig, SPOTIFY_SCOPES, QobuzClient, qobuz::QOBUZ_APP_ID};
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use sysinfo::Disks;
use tauri::{Emitter, State};
use walkdir::WalkDir;

include!("url_import.rs");
include!("library.rs");
include!("download.rs");
include!("service.rs");
include!("auth.rs");
include!("queue.rs");
include!("accounts.rs");
include!("settings.rs");
include!("tools.rs");
include!("dashboard.rs");
include!("migration.rs");
include!("enrichment.rs");
include!("metadata.rs");
include!("lyrics.rs");
include!("storage.rs");
include!("favorites.rs");

