//! Syncify Tauri Commands - organized via include!() macro

pub mod types;
pub use types::*;

pub mod progress;
pub use progress::*;

use crate::import_cache::ImportCache;
use crate::services::{ImportResult, SpotifyClient, SpotifyConfig, SPOTIFY_SCOPES, QobuzClient, qobuz::QOBUZ_APP_ID};
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[allow(unused_imports)]
use syncify_core_domain::quality::{QualityDecision, QualityDecisionKind, QualityPolicy};
use sysinfo::Disks;
use tauri::{Emitter, State};
use walkdir::WalkDir;
use futures_util::StreamExt;

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
include!("integrity.rs");
include!("backup.rs");
include!("playlists.rs");
include!("search.rs");
include!("notifications.rs");
include!("logging.rs");

