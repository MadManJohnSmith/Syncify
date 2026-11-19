//! Syncify Tauri Commands - organized as formal Rust submodules

pub mod editions;
pub use editions::*;
pub mod types;
pub use types::*;

pub mod progress;
pub use progress::*;

pub mod accounts;
pub use accounts::*;

pub mod account_login;
pub use account_login::*;

pub mod account_profiles;

pub mod auth;
pub use auth::*;

pub mod backup;
pub use backup::*;

pub mod dashboard;
pub use dashboard::*;

pub mod download;
pub use download::*;

pub mod download_jobs;
pub use download_jobs::*;

pub mod enrichment;
pub use enrichment::*;

pub mod favorites;
pub use favorites::*;

pub mod integrity;
pub use integrity::*;

pub mod library;
pub use library::*;

/// L01 (issue #98): consulta de biblioteca v2, módulo propio extraído de
/// `library.rs`. La serie L02-L12 del spec #96 extiende ESTE módulo.
pub mod library_query;

/// L05 (issue #102): selección por ids / «todo el filtro», vista previa y
/// runner de acciones por lotes en bloques de 500 con eventos agregados.
pub mod library_actions;
pub use library_actions::*;

pub mod logging;
pub use logging::*;

pub mod mb_dump;
pub use mb_dump::*;

pub mod lyrics;
pub use lyrics::*;

pub mod metadata;
pub use metadata::*;

pub mod migration;
pub use migration::*;

pub mod notifications;
pub use notifications::*;

// El módulo playback ya no expone comandos IPC (#132, P01): el crédito de
// escucha lo llama el ticker vía `perform_record_local_play` (ruta completa),
// así que sin glob re-export que quede huérfano en el binario.
pub mod playback;

pub mod playback_queue;
pub use playback_queue::*;

pub mod player;
pub use player::*;

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

pub mod sync_run;
pub use sync_run::*;

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
