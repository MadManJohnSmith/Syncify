//! Syncify Tauri Commands - organized as formal Rust submodules

pub mod types;
pub use types::*;

pub mod progress;
pub use progress::*;

pub mod accounts;
pub use accounts::*;

pub mod auth;
pub use auth::*;

pub mod backup;
pub use backup::*;

pub mod dashboard;
pub use dashboard::*;

pub mod download;
pub use download::*;

pub mod enrichment;
pub use enrichment::*;

pub mod favorites;
pub use favorites::*;

pub mod integrity;
pub use integrity::*;

pub mod library;
pub use library::*;

pub mod logging;
pub use logging::*;

pub mod lyrics;
pub use lyrics::*;

pub mod metadata;
pub use metadata::*;

pub mod migration;
pub use migration::*;

pub mod notifications;
pub use notifications::*;

pub mod playback;
pub use playback::*;

pub mod playlists;
pub use playlists::*;

pub mod queue;
pub use queue::*;

pub mod search;
pub use search::*;

pub mod service;
pub use service::*;

pub mod settings;
pub use settings::*;

pub mod storage;
pub use storage::*;

pub mod tags;
pub use tags::*;

pub mod tempo;
pub use tempo::*;

pub mod tools;
pub use tools::*;

pub mod url_import;
pub use url_import::*;

pub(crate) use crate::db::DbPool;
pub(crate) use crate::import_cache::ImportCache;
pub(crate) use crate::services::{
    qobuz::QOBUZ_APP_ID, ImportResult, QobuzClient, SpotifyClient, SpotifyConfig, SPOTIFY_SCOPES,
};
pub(crate) use crate::AppState;
pub(crate) use futures_util::StreamExt;
pub(crate) use serde::{Deserialize, Serialize};
pub(crate) use std::path::PathBuf;
#[allow(unused_imports)]
pub(crate) use syncify_core_domain::quality::{
    classify_audio_tier, AudioTier, QualityDecision, QualityDecisionKind, QualityPolicy,
};
pub(crate) use sysinfo::Disks;
pub(crate) use tauri::{Emitter, State};
pub(crate) use walkdir::WalkDir;
