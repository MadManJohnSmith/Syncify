//! Syncify Tauri Application
//!
//! Main entry point for the Tauri desktop application.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod crypto;
mod db;
mod download;
mod downloader;
pub mod enrichment_worker;
mod import_cache;
mod models;
mod services;
mod worker;

use db::DbPool;
use std::sync::Arc;
use tauri::Manager;
use worker::DownloadWorkerState;
pub use enrichment_worker::{EnrichmentWorker, EnrichmentWorkerState};

/// Lock for serializing album/artist creation across parallel imports
/// This is fast (microseconds) compared to database locks (seconds)
pub type AlbumCreationLock = Arc<tokio::sync::Mutex<()>>;

pub use crate::commands::ImportLock;

/// Application state shared across commands
pub struct AppState {
    pub db: DbPool,
    pub worker_state: DownloadWorkerState,
    pub album_lock: AlbumCreationLock,
    pub enrichment_state: EnrichmentWorkerState,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
struct EnrichmentFlags {
    enable_musicbrainz: bool,
    enable_lastfm: bool,
    enable_acoustid: bool,
}

impl Default for EnrichmentFlags {
    fn default() -> Self {
        Self {
            enable_musicbrainz: false,
            enable_lastfm: false,
            enable_acoustid: false,
        }
    }
}

#[allow(dead_code)]
fn is_enrichment_provider_enabled(flags: &EnrichmentFlags, provider: &str) -> bool {
    match provider {
        "musicbrainz" => flags.enable_musicbrainz,
        "lastfm" => flags.enable_lastfm,
        "acoustid" => flags.enable_acoustid,
        _ => false,
    }
}

#[allow(dead_code)]
async fn load_enrichment_flags(db: &DbPool) -> EnrichmentFlags {
    let row: Option<(i64, i64, i64)> = sqlx::query_as(
        "SELECT enable_musicbrainz, enable_lastfm, enable_acoustid FROM metadata_preferences WHERE id = 1",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    if let Some((musicbrainz, lastfm, acoustid)) = row {
        EnrichmentFlags {
            enable_musicbrainz: musicbrainz != 0,
            enable_lastfm: lastfm != 0,
            enable_acoustid: acoustid != 0,
        }
    } else {
        EnrichmentFlags::default()
    }
}

#[cfg(test)]
mod tests {
    use super::{is_enrichment_provider_enabled, EnrichmentFlags};

    #[test]
    fn test_enrichment_respects_flags() {
        let flags = EnrichmentFlags {
            enable_musicbrainz: false,
            enable_lastfm: true,
            enable_acoustid: false,
        };

        assert!(!is_enrichment_provider_enabled(&flags, "musicbrainz"));
        assert!(is_enrichment_provider_enabled(&flags, "lastfm"));
        assert!(!is_enrichment_provider_enabled(&flags, "acoustid"));
        assert!(!is_enrichment_provider_enabled(&flags, "unknown"));
    }
}

fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    tracing::info!("Syncify starting...");

    // Load environment variables from .env file
    if let Err(e) = dotenvy::dotenv() {
        tracing::warn!("No .env file found: {}", e);
    }

    // Create async runtime for database initialization
    let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");

    // Create worker state (active by default for background queue execution)
    let worker_state = DownloadWorkerState::new(2); // 2 concurrent downloads
    let worker_state_clone = worker_state.clone();


    // Create album creation lock for parallel imports
    let album_lock: AlbumCreationLock = Arc::new(tokio::sync::Mutex::new(()));
    let import_lock = crate::commands::ImportLock(tokio::sync::Mutex::new(()));

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            // Initialize database using AppHandle inside setup
            let init_handle = app.handle().clone();
            let db_pool = rt.block_on(async {
                db::init_db(&init_handle)
                    .await
                    .expect("Failed to initialize database")
            });
            tracing::info!("Database connected");
            let db_pool_clone = db_pool.clone();
            let enrichment_state = EnrichmentWorkerState::new();
            let enrichment_state_clone = enrichment_state.clone();

            // Manage app state after successful init
            app.manage(AppState {
                db: db_pool.clone(),
                worker_state,
                album_lock,
                enrichment_state,
            });
            app.manage(import_lock);

            // PAUSE MusicBrainz enrichment as requested (S78) - Commented out to restore user persistence
            /*
            let db_pause = db_pool.clone();
            rt.block_on(async move {
                let _ = sqlx::query("UPDATE metadata_preferences SET enable_musicbrainz = 0 WHERE id = 1")
                    .execute(&db_pause)
                    .await;
            });
            tracing::info!("Background enrichment paused (MusicBrainz disabled)");
            */

            // ═══════════════════════════════════════════════════════
            // KEYCHAIN CRYPTO INITIALIZATION (Sprint 01)
            // Must complete SYNCHRONOUSLY before any command that
            // touches credentials can execute.
            // ═══════════════════════════════════════════════════════
            match crate::crypto::init_keychain_crypto() {
                Ok(()) => {
                    tracing::info!("Keychain crypto initialized successfully");
                }
                Err(e) => {
                    tracing::error!("FATAL: Keychain initialization failed: {}", e);
                    return Err(Box::from(format!(
                        "Keychain initialization failed: {}. \
                         Syncify requires OS Keychain access to protect credentials.",
                        e
                    )));
                }
            }

            // ═══════════════════════════════════════════════════════
            // PYTHON DEPENDENCIES CHECK (Sprint 34)
            // ═══════════════════════════════════════════════════════
            let startup_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tracing::info!("Checking Python dependencies...");
                let python_cmd = commands::get_python_executable();
                
                // Log if .venv is missing as requested in S73
                if !python_cmd.contains(".venv") {
                    let project_root = commands::get_project_root();
                    let expected_venv = if cfg!(windows) {
                        project_root.join(".venv").join("Scripts").join("python.exe")
                    } else {
                        project_root.join(".venv").join("bin").join("python")
                    };
                    tracing::warn!("Python venv not found at {:?}. Python features disabled.", expected_venv);
                }

                // Run the check with a 3-second timeout
                let check_result = tokio::time::timeout(std::time::Duration::from_secs(3), async {
                    tokio::process::Command::new(&python_cmd)
                        .arg("-c")
                        .arg("import spotipy, acoustid, fuzzywuzzy")
                        .output()
                        .await
                })
                .await;

                match check_result {
                    Ok(Ok(output)) => {
                        if !output.status.success() {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            tracing::warn!("Python dependencies missing or error: {}", stderr);
                            use tauri::Emitter;
                            let _ = startup_handle.emit(
                                "python_deps_missing",
                                serde_json::json!({
                                    "message": "Missing required Python packages (spotipy, pyacoustid, etc). Please pip install -r scripts/requirements.txt",
                                }),
                            );
                        } else {
                            tracing::info!("Python dependencies checked successfully");
                        }
                    }
                    Ok(Err(e)) => {
                        tracing::warn!("Failed to execute python dependency check: {}", e);
                        use tauri::Emitter;
                        let _ = startup_handle.emit(
                            "python_deps_missing",
                            serde_json::json!({
                                "message": format!("Failed to run python check: {}. Is python in your PATH?", e),
                            }),
                        );
                    }
                    Err(_) => {
                        tracing::warn!("Python dependency check timed out after 3 seconds");
                        use tauri::Emitter;
                        let _ = startup_handle.emit(
                            "python_deps_missing",
                            serde_json::json!({
                                "message": "Python dependency check timed out. Your Python environment might be slow or misconfigured.",
                            }),
                        );
                    }
                }
            });

            // ═══════════════════════════════════════════════════════
            // LEGACY CREDENTIAL MIGRATION (Sprint 01)
            // Launched as async task — NON-BLOCKING.
            // init_keychain_crypto() already completed synchronously,
            // so get_key() returns the new keychain-backed key before
            // any credential command can execute. The migration runs
            // in background using decrypt_with_key() for the legacy key.
            // ═══════════════════════════════════════════════════════
            let db_for_migration = db_pool_clone.clone();
            let migration_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                match crate::crypto::migrate_legacy_credentials(&db_for_migration).await {
                    Ok((migrated, failed_ids)) => {
                        if migrated > 0 {
                            tracing::info!(
                                "Migrated {} credentials from legacy encryption",
                                migrated
                            );
                        }
                        if !failed_ids.is_empty() {
                            tracing::warn!(
                                "Failed to migrate {} credentials (account IDs: {:?}). \
                                 These accounts require re-authentication.",
                                failed_ids.len(),
                                failed_ids
                            );
                            use tauri::Emitter;
                            let _ = migration_handle.emit(
                                "credential_migration_partial",
                                serde_json::json!({
                                    "failed_count": failed_ids.len(),
                                    "failed_ids": failed_ids,
                                    "message": "Some accounts could not be migrated \
                                                and require re-authentication."
                                }),
                            );
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Legacy credential migration failed: {}", e);
                    }
                }

                // ═══════════════════════════════════════════════════════
                // STALE CREDENTIAL PURGE
                // Detect and remove accounts whose credentials were
                // encrypted with a different machine's keychain key.
                // ═══════════════════════════════════════════════════════
                let rows: Vec<(i64, String, String)> = sqlx::query_as(
                    r#"SELECT a.id, s.name, a.credentials_json
                       FROM accounts a
                       JOIN services s ON s.id = a.service_id
                       WHERE a.credentials_json IS NOT NULL"#,
                )
                .fetch_all(&db_for_migration)
                .await
                .unwrap_or_default();

                let mut purged = 0u32;
                let mut purged_names: Vec<String> = Vec::new();
                for (account_id, service_name, ciphertext) in &rows {
                    if crate::crypto::decrypt(ciphertext).is_err() {
                        tracing::warn!(
                            "Startup purge: removing stale account {} ({}) — irrecoverable credentials",
                            account_id,
                            service_name
                        );
                        let _ = sqlx::query("UPDATE accounts SET credentials_invalid = 1 WHERE id = ?")
                            .bind(account_id)
                            .execute(&db_for_migration)
                            .await;
                        purged += 1;
                        purged_names.push(service_name.clone());
                    }
                }
                if purged > 0 {
                    tracing::info!(
                        "Startup purge: removed {} stale accounts ({:?}). Re-authentication required.",
                        purged,
                        purged_names
                    );
                    use tauri::Emitter;
                    let _ = migration_handle.emit(
                        "stale_credentials_purged",
                        serde_json::json!({
                            "purged_count": purged,
                            "services": purged_names,
                            "message": "Some service accounts were encrypted with a different machine's key and have been removed. Please reconnect your accounts."
                        }),
                    );
                }

                // If valid QOBUZ_USER_TOKEN is in environment, ensure active Qobuz account row uses it
                if let Ok(env_token) = std::env::var("QOBUZ_USER_TOKEN") {
                    let trimmed = env_token.trim();
                    if !trimmed.is_empty() && !trimmed.starts_with("eyJ") {
                        let creds = serde_json::json!({
                            "user_auth_token": trimmed,
                            "auth_token": trimmed,
                            "display_name": "Qobuz User",
                        });
                        if let Ok(encrypted) = crate::crypto::encrypt(&creds.to_string()) {
                            let _ = sqlx::query(
                                r#"UPDATE accounts 
                                   SET credentials_json = ?, credentials_invalid = 0, invalid_reason = NULL, last_auth_error = NULL
                                   WHERE service_id = (SELECT id FROM services WHERE name = 'qobuz')
                                     AND (credentials_invalid = 1 OR credentials_json IS NULL OR credentials_json NOT LIKE '%user_auth_token%')"#
                            )
                            .bind(&encrypted)
                            .execute(&db_for_migration)
                            .await;
                        }
                    }
                }
            });

            // Start background download worker with supervisor
            let handle = app.handle().clone();
            let db = db_pool_clone.clone();
            let state = worker_state_clone.clone();

            tauri::async_runtime::spawn(async move {
                let mut restart_count: u32 = 0;
                const MAX_RESTARTS: u32 = 3;

                loop {
                    // Clone at the TOP of each iteration — each inner spawn
                    // takes ownership via move, so fresh clones are needed
                    // every iteration to avoid E0382 (use of moved value).
                    let handle_clone = handle.clone();      // → inner spawn (worker)
                    let supervisor_emit = handle.clone();   // → outer loop (emit events)
                    let db_clone = db.clone();
                    let state_clone = state.clone();

                    let worker_handle = tokio::task::spawn(async move {
                        let worker = worker::DownloadWorker::new(db_clone, state_clone)
                            .with_app_handle(handle_clone);
                        worker.run().await;
                    });

                    match worker_handle.await {
                        Ok(()) => {
                            // Worker exited cleanly (stopped by user)
                            tracing::info!("Download worker exited cleanly");
                            break;
                        }
                        Err(join_error) => {
                            restart_count += 1;
                            tracing::error!(
                                "Download worker panicked (restart {}/{}): {}",
                                restart_count,
                                MAX_RESTARTS,
                                join_error
                            );

                            if restart_count >= MAX_RESTARTS {
                                tracing::error!(
                                    "Download worker exceeded max restarts ({}). \
                                     Worker permanently stopped.",
                                    MAX_RESTARTS
                                );
                                use tauri::Emitter;
                                let _ = supervisor_emit.emit(
                                    "worker_fatal",
                                    serde_json::json!({
                                        "message": "Download worker crashed and could not recover.",
                                        "restart_count": restart_count
                                    }),
                                );
                                break;
                            }

                            // Emit restart event
                            use tauri::Emitter;
                            let _ = supervisor_emit.emit(
                                "worker_restarted",
                                serde_json::json!({
                                    "restart_count": restart_count,
                                    "max_restarts": MAX_RESTARTS
                                }),
                            );

                            // Backoff: 5s first, 30s subsequent
                            let backoff = if restart_count == 1 {
                                std::time::Duration::from_secs(5)
                            } else {
                                std::time::Duration::from_secs(30)
                            };
                            tracing::warn!(
                                "Restarting download worker in {:?}...",
                                backoff
                            );
                            tokio::time::sleep(backoff).await;
                        }
                    }
                }
            });

            tracing::info!("Background download worker started (supervised)");

            // Start background enrichment worker (S97)
            let db_for_enrichment = db_pool_clone.clone();
            let enrichment_handle = app.handle().clone();
            let enrichment_worker_state = enrichment_state_clone.clone();
            tauri::async_runtime::spawn(async move {
                let rate_limiter = std::sync::Arc::new(crate::services::rate_limiter::RateLimiter::new());
                let worker = EnrichmentWorker::new(
                    db_for_enrichment,
                    enrichment_worker_state,
                    rate_limiter,
                )
                .with_app_handle(enrichment_handle);

                worker.run().await;
            });
            
            tracing::info!("Background enrichment worker started");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Library
            commands::get_library,
            commands::get_library_stats,
            commands::get_dashboard_stats,
            commands::get_health_checks,
            commands::get_duplicate_stats,
            commands::get_duplicate_tracks,
            commands::reset_database,
            commands::search_tracks,
            commands::search_library,
            commands::get_artist,
            commands::get_album,
            commands::repair_artist_links,
            commands::get_playlists,
            commands::get_playlist,
            commands::add_to_playlist,
            commands::create_playlist,
            commands::update_playlist,
            commands::delete_playlist,
            commands::remove_from_playlist,
            commands::reorder_playlist_tracks,
            commands::sync_playlists,
            commands::remove_track,
            commands::bulk_remove_tracks,
            commands::toggle_favorite,
            commands::toggle_track_favorite,
            commands::set_track_favorite,
            commands::get_favorite_tracks,
            commands::get_favorites_tracks,
            commands::get_favorites_albums,
            commands::get_favorites_artists,
            commands::sync_favorites,
            commands::toggle_album_favorite,
            commands::toggle_artist_favorite,
            commands::push_favorite_to_service,
            commands::download_favorites,
            commands::run_integrity_audit,
            commands::repair_integrity_issues,
            commands::export_library,
            commands::import_library,
            commands::emit_test_notification,
            commands::show_in_folder,
            commands::get_track_metadata,
            // Downloads & Queue Management (Canonical)
            commands::enqueue_download,
            commands::add_to_queue,
            commands::add_batch_to_queue,
            commands::get_queue,
            commands::get_queue_stats,
            commands::reorder_queue,
            commands::update_queue_priority,
            commands::cancel_download,
            commands::cancel_queue_item,
            commands::retry_queue_item,
            commands::clear_queue,
            commands::remove_from_queue,
            commands::restore_interrupted_downloads,
            commands::get_worker_status,
            commands::pause_downloads,
            commands::resume_downloads,
            commands::start_worker,
            commands::resume_worker,
            commands::pause_worker,
            commands::set_max_concurrent_downloads,
            commands::queue_downloads,
            commands::get_download_queue,
            commands::get_failed_downloads,
            commands::retry_failed_downloads,
            commands::clear_failed_downloads,
            // Spotify
            commands::start_spotify_auth,
            commands::spotify_auth_callback,
            commands::import_spotify_library,
            commands::enrich_album_metadata,
            commands::import_spotify_playlists,
            // Qobuz
            commands::import_qobuz_library,
            commands::import_qobuz_playlists,
            commands::enrich_qobuz_album_metadata,
            // Tidal
            commands::import_tidal_library,
            // Deezer
            commands::import_deezer_library,
            // SoundCloud
            commands::import_soundcloud_library,
            // Apple Music
            commands::import_apple_music_library,
            // Services
            commands::get_service_statuses,
            commands::import_service,
            commands::import_from_url,
            // Python Auth Bridge
            commands::start_auth,
            commands::get_auth_status,
            commands::logout_service,
            commands::start_auth_and_save,
            commands::validate_all_sessions,
            commands::spotify_auth_webview,
            // Lyrics
            commands::resolve_track_lyrics,
            commands::fetch_lyrics,
            commands::get_lyrics,
            commands::get_all_lyrics,
            commands::get_lyrics_stats,
            commands::save_lyrics,
            commands::delete_lyrics,
            commands::search_lyrics,
            commands::fetch_and_save_lyrics,
            commands::batch_fetch_lyrics,
            commands::batch_fetch_lyrics_with_progress,
            commands::fetch_missing_lyrics,
            commands::embed_lyrics,
            commands::batch_embed_lyrics,
            // Downloads
            commands::download_track,
            // Metadata Enrichment
            commands::enrich_metadata,
            commands::enrich_metadata_musicbrainz,
            commands::match_musicbrainz,
            commands::enrich_spotify_audio_features,
            commands::enrich_genre_lastfm,
            commands::enrich_track,
            commands::enrich_before_download,
            commands::start_enrichment_worker,
            commands::pause_enrichment_worker,
            commands::resume_enrichment_worker,
            commands::get_enrichment_status,
            // Fingerprinting
            commands::check_fingerprint_available,
            commands::identify_audio,
            commands::find_audio_duplicates,
            // Conversion (FFmpeg)
            commands::check_ffmpeg_available,
            commands::get_audio_info,
            commands::convert_audio,
            // Local Library Scanner
            commands::scan_local_library,
            commands::get_local_track_metadata,
            // File Organizer
            commands::preview_organization,
            commands::organize_files,
            // Progress-Enabled Commands
            commands::scan_local_library_with_progress,
            commands::batch_download_tracks,
            commands::batch_enrich_metadata,
            // Playlist Commands
            commands::list_playlists,
            commands::get_playlist_tracks,
            commands::export_playlist,
            commands::match_playlist_to_service,
            // Dependency Management
            commands::check_dependencies,
            commands::install_dependency,
            commands::install_all_dependencies,
            commands::ensure_dependency,
            // Queue Management
            commands::enqueue_download,
            commands::add_to_queue,
            commands::add_batch_to_queue,
            commands::get_queue,
            commands::get_queue_stats,
            commands::update_queue_priority,
            commands::reorder_queue,
            commands::cancel_download,
            commands::cancel_queue_item,
            commands::retry_failed,
            commands::retry_queue_item,
            commands::retry_all_failed,
            commands::clear_completed,
            commands::clear_queue,
            commands::remove_from_queue,
            commands::restore_interrupted_downloads,
            // Single-Track Direct Pipeline
            commands::download_tidal_single_track,

            // Worker Control
            commands::get_worker_status,
            commands::pause_downloads,
            commands::resume_downloads,
            commands::start_worker,
            commands::resume_worker,
            commands::pause_worker,
            commands::set_max_concurrent_downloads,

            // Settings
            commands::get_kv_settings,
            commands::save_setting,
            commands::save_settings_batch,
            // Health Check
            commands::run_health_check,
            // Account Management
            commands::get_services,
            commands::get_accounts,
            commands::add_account,
            commands::remove_account,
            commands::get_account_credentials,
            commands::update_account_sync_time,
            commands::toggle_account_active,
            commands::purge_stale_credentials,
            // Sprint 1: Service Preferences & Sync Settings
            commands::get_service_preferences,
            commands::update_service_preference,
            commands::reorder_service_priorities,
            commands::get_sync_settings,
            commands::update_sync_settings,
            commands::get_service_sync_settings,
            commands::update_service_sync_settings,
            // Sprint 2: Downloads + File Settings
            commands::get_quality_preferences,
            commands::update_quality_preference,
            commands::get_folder_settings,
            commands::update_folder_settings,
            commands::preview_folder_path,
            commands::get_duplicate_settings,
            commands::update_duplicate_settings,
            commands::get_audio_processing_settings,
            commands::update_audio_processing_settings,
            // Sprint 3: Lyrics Tab + Settings
            commands::get_lyrics_providers,
            commands::update_lyrics_provider,
            commands::reorder_lyrics_providers,
            commands::get_lyrics_config,
            commands::update_lyrics_config,
            commands::test_lyrics_provider,
            // Sprint 4: Dashboard + Library Detail Views
            commands::get_service_health,
            commands::create_library_snapshot,
            commands::get_library_snapshots,
            commands::get_album_detail,
            commands::get_album_tracks,
            commands::get_artist_detail,
            commands::get_artist_albums,
            commands::get_artist_tracks,
            // Sprint 5: Advanced Settings & Polish
            commands::get_advanced_settings,
            commands::update_advanced_settings,
            commands::get_metadata_preferences,
            commands::update_metadata_preferences,
            commands::vacuum_database,
            commands::get_cache_stats,
            commands::clear_cache,
            commands::run_diagnostics,
            commands::reset_to_defaults,
            // Sprint 6: Migration Tab
            commands::get_migration_history,
            commands::get_migration_details,
            commands::get_migration_items_by_status,
            commands::preview_migration,
            commands::start_migration,
            commands::cancel_migration,
            commands::retry_failed_items,
            commands::delete_migration,
            commands::get_migration_templates,
            commands::save_migration_template,
            commands::delete_migration_template,
            commands::use_migration_template,
            commands::search_destination_track,
            commands::manual_match_item,
            commands::run_migration_audit,
            // Metadata Tab
            commands::update_track_metadata,
            commands::get_metadata_stats,
            commands::get_tracks_needing_metadata,
            commands::apply_musicbrainz_match,
            commands::get_storage_stats,
            commands::get_top_artists,
            commands::get_album,
            commands::get_local_playlist_tracks,
            commands::get_audio_quality_distribution,
            commands::auto_resolve_duplicates,
            // Service Settings (Sprint 12)
            commands::get_app_settings,
            commands::service_save_settings,
            commands::get_default_download_path,
            // Single Track Download Pipeline (Corte 2)
            commands::download_tidal_single_track,
        ])

        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
