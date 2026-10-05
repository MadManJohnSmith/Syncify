//! Syncify Tauri Application
//!
//! Main entry point for the Tauri desktop application.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cmd_utils;
mod commands;
mod crypto;
mod db;
mod download;
pub mod enrichment_worker;
mod import_cache;
mod logs_history;
mod models;
mod services;
mod sync_scheduler;
mod system_conditions;
mod tray;
mod worker;

use db::DbPool;
pub use enrichment_worker::{EnrichmentWorker, EnrichmentWorkerState};
use std::sync::Arc;
use tauri::Manager;
use worker::DownloadWorkerState;

pub use crate::commands::ImportLock;

/// Application state shared across commands
pub struct AppState {
    pub db: DbPool,
    pub worker_state: DownloadWorkerState,
    pub enrichment_state: EnrichmentWorkerState,
    pub concurrency_manager: Arc<services::ConcurrencyManager>,
}

fn main() {
    // Endurecimiento de seguridad TASK-112: umask estricto en sistemas Unix
    crate::crypto::set_secure_process_umask();

    // .env solo en desarrollo (SEC-010): dotenvy buscaría el archivo en el CWD
    // y sus padres, y en release las credenciales llegan por keyring/UI o por
    // el entorno real del sistema. SYNCIFY_ENV también activa la carga de .env
    // en los puentes Python.
    #[cfg(debug_assertions)]
    {
        if std::env::var_os("SYNCIFY_ENV").is_none() {
            std::env::set_var("SYNCIFY_ENV", "development");
        }
        let _ = dotenvy::dotenv();
    }

    // ═══════════════════════════════════════════════════════
    // WEBKIT HARDENING (must run BEFORE the webview is created)
    // ═══════════════════════════════════════════════════════
    // WebKitGTK opens an EGL display for the webview's accelerated
    // compositing and ABORTS the whole process when that fails ("Could not
    // create default EGL display: EGL_BAD_PARAMETER. Aborting..." — verified
    // on the AppImage v0.3.0 on an Iris Xe / KDE Wayland host, where the DMABUF
    // workaround alone was not enough).
    //
    // Two switches are needed, not one: WEBKIT_DISABLE_DMABUF_RENDERER only
    // drops the DMABuf renderer, and compositing still creates the EGL display,
    // so WEBKIT_DISABLE_COMPOSITING_MODE is the one that keeps the webview on
    // cairo and never touches EGL. LIBGL_ALWAYS_SOFTWARE would also stop the
    // abort, but it forces llvmpipe on every machine including the ones whose
    // GPU works fine.
    //
    // They have to be set here, in-process: the WebKit UI process inherits the
    // environment when it is spawned, and asking users to export a variable is
    // not an acceptable install experience.
    #[cfg(target_os = "linux")]
    for var in [
        "WEBKIT_DISABLE_DMABUF_RENDERER",
        "WEBKIT_DISABLE_COMPOSITING_MODE",
    ] {
        if std::env::var_os(var).is_none() {
            std::env::set_var(var, "1");
        }
    }

    // Initialize unified logging system (rotating file in dev, console, in-memory ring buffer)
    let log_config = services::logging::init_logging_system(None, None);

    tracing::info!(
        is_dev = log_config.is_development,
        log_to_file = log_config.log_to_file,
        log_level = %log_config.log_level,
        log_dir = %log_config.log_dir.display(),
        "Syncify starting..."
    );

    // Hasta `init_logging_system` no hay subscriber: un `tracing::info!` en el
    // bloque de arriba se perdía sin llegar al log de arranque.
    #[cfg(target_os = "linux")]
    tracing::info!(
        dmabuf_disabled = std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").as_deref() == Ok("1"),
        compositing_disabled =
            std::env::var("WEBKIT_DISABLE_COMPOSITING_MODE").as_deref() == Ok("1"),
        "WebKit rendering: EGL-independent path configured"
    );

    // Create async runtime for database initialization
    let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");

    // Create worker state (active by default for background queue execution)
    let worker_state = DownloadWorkerState::new(2); // 2 concurrent downloads
    let worker_state_clone = worker_state.clone();

    let import_lock = crate::commands::ImportLock(tokio::sync::Mutex::new(()));
    let concurrency_manager = services::get_global_concurrency_manager();

    tauri::Builder::default()
        // S194/S200: local playback protocol - serves byte ranges of files that
        // resolve_playback_source explicitly granted (downloads-verified).
        // S200: ASYNC registration — the handler does blocking file IO (up to
        // 8 MB per request); the synchronous variant ran it on the MAIN thread
        // (webkit/WebView2 serve custom schemes there) and froze the UI while
        // audio played. The async variant answers via UriSchemeResponder from a
        // plain OS thread.
        .register_asynchronous_uri_scheme_protocol("syncify-media", |_ctx, request, responder| {
            commands::handle_media_protocol_request_async(request, responder)
        })
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        // Ítem 31: autoinicio al arrancar el sistema, gobernado por el ajuste
        // existente `start_on_boot` (tray.rs lo aplica con el ManagerExt del plugin).
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        // Ítem 32: notificaciones nativas del escritorio, respetando la
        // preferencia del usuario (`notifications_enabled` en tray.rs).
        .plugin(tauri_plugin_notification::init())
        .setup(move |app| {
            // Initialize database using AppHandle inside setup
            let init_handle = app.handle().clone();
            services::logging::get_global_log_buffer().set_app_handle(init_handle.clone());
            commands::set_global_app_handle(init_handle.clone());

            // ═══════════════════════════════════════════════════════
            // APPLICATION PROFILE PERMISSIONS HARDENING (TASK-112)
            // Enforce 0700 for dirs and 0600 for files, audit/purge residual localstorage
            // ═══════════════════════════════════════════════════════
            if let Ok(profile_dir) = init_handle.path().app_local_data_dir() {
                match crate::crypto::ensure_secure_profile_permissions(&profile_dir) {
                    Ok(report) => {
                        tracing::info!(
                            dirs_hardened = report.directories_hardened,
                            files_hardened = report.files_hardened,
                            purged_localstorage = report.purged_localstorage_files,
                            "Application profile permissions hardened to 0700/0600"
                        );
                    }
                    Err(e) => {
                        tracing::warn!("Failed to harden application profile permissions: {}", e);
                    }
                }
            }

            let db_pool = rt.block_on(async {
                db::init_db(&init_handle)
                    .await
                    .expect("Failed to initialize database")
            });
            tracing::info!("Database connected");

            // Post-DB initialization check to ensure newly created DB / WAL / SHM files conform to 0600
            if let Ok(profile_dir) = init_handle.path().app_local_data_dir() {
                let _ = crate::crypto::ensure_secure_profile_permissions(&profile_dir);
            }

            let persisted_max_concurrent: usize = rt.block_on(async {
                let val: Option<i64> = sqlx::query_scalar(
                    "SELECT COALESCE(
                        (SELECT max_concurrent_downloads FROM sync_settings WHERE id = 1),
                        (SELECT CAST(value AS INTEGER) FROM settings WHERE key = 'dl_concurrent_downloads'),
                        2
                    )"
                )
                .fetch_optional(&db_pool)
                .await
                .ok()
                .flatten();

                val.map(|v| v.max(1) as usize).unwrap_or(2)
            });

            tracing::info!("Loaded persisted max_concurrent_downloads: {}", persisted_max_concurrent);
            worker_state.set_max_concurrent(persisted_max_concurrent);

            let db_pool_clone = db_pool.clone();
            let enrichment_state = EnrichmentWorkerState::new();
            let enrichment_state_clone = enrichment_state.clone();

            // Manage app state after successful init
            app.manage(AppState {
                db: db_pool.clone(),
                worker_state,
                enrichment_state,
                concurrency_manager: concurrency_manager.clone(),
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
            // CONFIGURE PATH FOR BUNDLED BINARIES (FFmpeg, fpcalc)
            // ═══════════════════════════════════════════════════════
            let project_root = commands::get_project_root();
            let bin_dir = project_root.join("bin");
            let res_bin_dir = project_root.join("resources").join("bin");
            if let Ok(current_path) = std::env::var("PATH") {
                let sep = if cfg!(windows) { ";" } else { ":" };
                let mut prepends = Vec::new();
                if bin_dir.exists() {
                    prepends.push(bin_dir.to_string_lossy().to_string());
                }
                if res_bin_dir.exists() {
                    prepends.push(res_bin_dir.to_string_lossy().to_string());
                }
                if !prepends.is_empty() {
                    let new_path = format!("{}{}{}", prepends.join(sep), sep, current_path);
                    std::env::set_var("PATH", new_path);
                    tracing::info!("Prepended bundled bin directories to PATH: {:?}", prepends);
                }
            }

            // ═══════════════════════════════════════════════════════
            // GSTREAMER PLUGINS (Linux only)
            // ═══════════════════════════════════════════════════════
            // WebKitGTK reproduce el audio del player vía GStreamer. En el
            // AppImage las librerías vienen de webkit2gtk pero los plugins no,
            // así que hay que apuntar GStreamer a los que empaquetamos.
            #[cfg(target_os = "linux")]
            {
                let mut gst_candidates = vec![
                    project_root.join("usr").join("lib").join("gstreamer-1.0"),
                    project_root.join("lib").join("gstreamer-1.0"),
                ];
                // AppImage: el ejecutable vive en `<AppDir>/usr/bin` y los
                // plugins se inyectan en `<AppDir>/usr/lib/gstreamer-1.0`.
                // `project_root` resuelve a `<AppDir>/resources` (es donde
                // encuentra `scripts/`), así que sin estas rutas el
                // GST_PLUGIN_SYSTEM_PATH_1_0 apuntaba a un directorio inexistente
                // y el audio acababa dependiendo de los plugins del anfitrión.
                // Se descarta lo que cuelgue de `/usr` o `/bin`: ahí el
                // directorio es el del sistema y pisar la variable taparía las
                // rutas que GStreamer añade por defecto (p.ej. multiarch).
                if let Ok(exe) = std::env::current_exe() {
                    if let Some(exe_dir) = exe.parent() {
                        if !exe_dir.starts_with("/usr") && !exe_dir.starts_with("/bin") {
                            gst_candidates.push(exe_dir.join("lib").join("gstreamer-1.0"));
                            if let Some(app_usr) = exe_dir.parent() {
                                gst_candidates.push(app_usr.join("lib").join("gstreamer-1.0"));
                            }
                        }
                    }
                }
                // Un directorio vacío también es peor que no tocar nada:
                // GST_PLUGIN_SYSTEM_PATH_1_0 sustituye la lista por defecto.
                let has_plugins = |dir: &std::path::Path| {
                    dir.is_dir()
                        && std::fs::read_dir(dir)
                            .map(|entries| {
                                entries.flatten().any(|entry| {
                                    entry.path().extension().is_some_and(|ext| ext == "so")
                                })
                            })
                            .unwrap_or(false)
                };
                if let Some(gst_dir) = gst_candidates.iter().find(|d| has_plugins(d)) {
                    let gst_str = gst_dir.to_string_lossy().to_string();
                    let existing = std::env::var("GST_PLUGIN_SYSTEM_PATH_1_0")
                        .unwrap_or_default();
                    let new_path = if existing.is_empty() {
                        gst_str
                    } else if existing.split(':').any(|p| p == gst_str) {
                        existing
                    } else {
                        format!("{}:{}", gst_str, existing)
                    };
                    std::env::set_var("GST_PLUGIN_SYSTEM_PATH_1_0", &new_path);
                    tracing::info!(
                        "Bundled GStreamer plugins directory: {}",
                        gst_dir.to_string_lossy()
                    );
                }
            }

            // ═══════════════════════════════════════════════════════
            // NEUTRALIZE PYTHON ENV INJECTED BY LINUXDEPLOY'S APPRUN
            // ═══════════════════════════════════════════════════════
            // AppRun.c:169 de linuxdeploy hace putenv("PYTHONHOME=$APPDIR/usr/")
            // (+ PYTHONPATH=usr/share/pyshared y PYTHONDONTWRITEBYTECODE) aunque
            // el AppImage no empaquete Python, y eso revienta cualquier Python
            // del sistema que la app invoque ("No module named 'encodings'").
            // Este proceso es dueño de su entorno: se limpia siempre; los
            // puentes reciben su PYTHONPATH por comando cuando corresponden.
            {
                let had_python_env = std::env::var_os("PYTHONHOME").is_some()
                    || std::env::var_os("PYTHONPATH").is_some();
                std::env::remove_var("PYTHONHOME");
                std::env::remove_var("PYTHONPATH");
                std::env::remove_var("PYTHONDONTWRITEBYTECODE");
                if had_python_env {
                    tracing::info!("Cleared inherited Python environment variables");
                }
            }

            // ═══════════════════════════════════════════════════════
            // AUTO-DOWNLOAD EXTERNAL DEPENDENCIES IF MISSING (FFmpeg, fpcalc)
            // ═══════════════════════════════════════════════════════
            tauri::async_runtime::spawn(async move {
                let has_ffmpeg = crate::cmd_utils::create_std_command("ffmpeg")
                    .arg("-version")
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false);

                if !has_ffmpeg {
                    tracing::info!("FFmpeg/fpcalc not detected, auto-downloading dependencies via dependency_manager...");
                    match commands::install_all_dependencies().await {
                        Ok(bridge_result) => {
                            if !bridge_result.success {
                                let err_msg = bridge_result.error.unwrap_or_else(|| "Unknown error".to_string());
                                tracing::error!("Failed to auto-download dependencies (integrity or installation error): {}", err_msg);
                            } else {
                                tracing::info!("External dependencies successfully verified and installed");
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to execute dependency_manager: {}", e);
                        }
                    }
                }
            });

            // ═══════════════════════════════════════════════════════
            // PYTHON DEPENDENCIES CHECK (Sprint 34)
            // ═══════════════════════════════════════════════════════
            let startup_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tracing::info!("Checking Python dependencies...");
                let python_cmd = commands::get_python_executable();
                let project_root = commands::get_project_root();

                // El runtime empaquetado (AppImage/DEB/instalador NSIS) y el
                // Python del sistema no fallan igual: al primero no le sirve
                // de nada un `pip install` en la máquina del usuario.
                let bundled_python = crate::cmd_utils::packaged_python_candidates(
                    &crate::cmd_utils::find_scripts_dir(&project_root),
                )
                .iter()
                .any(|candidate| candidate == std::path::Path::new(&python_cmd));
                let recovery_hint = if bundled_python {
                    "The Python runtime bundled with Syncify is incomplete (spotipy, pyacoustid, mutagen or yarl missing). Nothing to install: reinstall the app, and report it from Help if it persists."
                } else {
                    "Missing required Python packages (spotipy, pyacoustid, etc). Install them with: pip install --user --break-system-packages -r requirements.txt (or use a venv)"
                };

                // Log if .venv is missing as requested in S73. Solo es un
                // problema real si get_python_executable cayó al fallback de
                // PATH: un Python empaquetado (`python/bin/python`) o un venv
                // ya son instalaciones válidas y no generan ruido.
                if python_cmd == "python" {
                    let expected_venv = if cfg!(windows) {
                        project_root.join(".venv").join("Scripts").join("python.exe")
                    } else {
                        project_root.join(".venv").join("bin").join("python")
                    };
                    tracing::warn!("Python venv not found at {:?}. Python features disabled.", expected_venv);
                }

                // Run the check with a 5-second timeout. Módulos que los
                // puentes sí importan: fuzzywuzzy ya no es dependencia
                // (requirements.txt trae rapidfuzz vía syncedlyrics) y un
                // import muerto aquí marcaba "deps faltantes" con el bundle
                // completo instalado.
                let check_result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
                    crate::cmd_utils::create_tokio_command(&python_cmd)
                        .arg("-c")
                        .arg("import spotipy, acoustid, mutagen, yarl")
                        .output()
                        .await
                })
                .await;

                match check_result {
                    Ok(Ok(output)) => {
                        if !output.status.success() {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            tracing::warn!("Python dependencies missing: {}. Trying auto-install...", stderr);

                            // requirements.txt en el árbol dev o en los resources
                            // empaquetados (find_scripts_dir cubre `_up_/scripts`).
                            let req_file = [
                                project_root.join("requirements.txt"),
                                project_root.join("scripts").join("requirements.txt"),
                                crate::cmd_utils::find_scripts_dir(&project_root)
                                    .join("requirements.txt"),
                            ]
                            .into_iter()
                            .find(|p| p.exists());
                            let mut auto_fixed = false;
                            if let Some(req_file) = req_file {
                                // Intento 1: pip normal (solo funciona tal cual en un venv).
                                let mut install_result = crate::cmd_utils::create_tokio_command(&python_cmd)
                                    .arg("-m")
                                    .arg("pip")
                                    .arg("install")
                                    .arg("-r")
                                    .arg(&req_file)
                                    .output()
                                    .await;

                                // Intento 2: Pythons "externally-managed" (PEP 668: Arch,
                                // Fedora, Debian 12+) rechazan pip global; `--user
                                // --break-system-packages` instala en el site-packages del
                                // usuario sin tocar el del sistema. pip antiguo (<23) no
                                // conoce el flag y fallará igual → evento al frontend.
                                let first_failed = !install_result
                                    .as_ref()
                                    .is_ok_and(|r| r.status.success());
                                if first_failed {
                                    tracing::info!(
                                        "Plain pip install failed; retrying with --user --break-system-packages (PEP 668)"
                                    );
                                    install_result = crate::cmd_utils::create_tokio_command(&python_cmd)
                                        .arg("-m")
                                        .arg("pip")
                                        .arg("install")
                                        .arg("--user")
                                        .arg("--break-system-packages")
                                        .arg("-r")
                                        .arg(&req_file)
                                        .output()
                                        .await;
                                }

                                match install_result {
                                    Ok(res) if res.status.success() => {
                                        tracing::info!("Successfully auto-installed Python requirements!");
                                        auto_fixed = true;
                                    }
                                    Ok(res) => {
                                        let stderr = String::from_utf8_lossy(&res.stderr);
                                        tracing::warn!(
                                            "pip auto-install failed: {}",
                                            stderr.lines().last().unwrap_or("(sin stderr)")
                                        );
                                    }
                                    Err(e) => tracing::warn!("pip auto-install could not run: {}", e),
                                }
                            }

                            if !auto_fixed {
                                use tauri::Emitter;
                                let _ = startup_handle.emit(
                                    "python_deps_missing",
                                    serde_json::json!({
                                        "message": recovery_hint,
                                    }),
                                );
                            }
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
                                "message": format!("Failed to run python check: {}. {}", e, recovery_hint),
                            }),
                        );
                    }
                    Err(_) => {
                        tracing::warn!("Python dependency check timed out");
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
                let mut restored = Vec::new();
                let mut purged_names: Vec<String> = Vec::new();
                for (account_id, service_name, ciphertext) in &rows {
                    match crate::crypto::decrypt(ciphertext) {
                        // La clave actual descifra: si la cuenta venía marcada
                        // por un arranque donde el keyring no estaba disponible,
                        // la marca era transitoria y la cuenta vuelve a estar viva.
                        Ok(_) => {
                            let _ = sqlx::query(
                                "UPDATE accounts SET credentials_invalid = 0, invalid_reason = NULL WHERE id = ? AND credentials_invalid = 1",
                            )
                            .bind(account_id)
                            .execute(&db_for_migration)
                            .await;
                            restored.push(service_name.clone());
                        }
                        Err(_) => {
                            tracing::warn!(
                                "Startup purge: marking account {} ({}) as needing re-auth — \
                                 credentials could not be decrypted with the current key",
                                account_id,
                                service_name
                            );
                            // Sin `last_auth_error_at`: esto NO es un rechazo del proveedor sino una
                            // credencial irrecuperable (cifrada con otra clave de
                            // keychain). El decay por antigüedad de
                            // `auth_latch_is_stale` no aplica: reintentar no la
                            // reconstruye, solo exigiría un login que el usuario
                            // no puede evitar. Los rechazos de proveedor (Tidal,
                            // worker) sí lo sellan.
                            let _ =
                                sqlx::query("UPDATE accounts SET credentials_invalid = 1 WHERE id = ?")
                                    .bind(account_id)
                                    .execute(&db_for_migration)
                                    .await;
                            purged += 1;
                            purged_names.push(service_name.clone());
                        }
                    }
                }
                if !restored.is_empty() {
                    tracing::info!(
                        "Startup check: {} account(s) recovered after the key became readable \
                         again ({:?}).",
                        restored.len(),
                        restored
                    );
                }
                if purged > 0 {
                    tracing::info!(
                        "Startup purge: marked {} account(s) needing re-auth ({:?}).",
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

            // ═══════════════════════════════════════════════════════
            // POST-CRASH OPERATION RECONCILIATION (Sprint 167)
            // Automatically reconciles un-terminated operations from
            // the persistent journal and download_queue.
            // ═══════════════════════════════════════════════════════
            let db_for_recovery = db_pool_clone.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = crate::services::operation_recovery::reconcile_startup_operations(&db_for_recovery, None).await {
                    tracing::warn!("Post-crash startup reconciliation encountered an error: {}", e);
                }
                if let Err(e) = crate::services::operation_recovery::cleanup_staging_and_recover_stuck_queue(&db_for_recovery, None).await {
                    tracing::warn!("Startup staging cleanup and queue recovery encountered an error: {}", e);
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

            // ═══════════════════════════════════════════════════════
            // ITEM 29: SCHEDULED SYNCHRONIZATION
            // Reads the persisted `sync_settings` interval and fires the same
            // flow as the manual tray sync while the toggle is on.
            // ═══════════════════════════════════════════════════════
            sync_scheduler::start_sync_scheduler(app.handle().clone());

            // ═══════════════════════════════════════════════════════
            // ITEM 28: ENVIRONMENT PAUSES (metered network / low battery)
            // Real D-Bus detection (NetworkManager Metered + UPower) wired to
            // the existing `pause_on_metered` / `pause_on_low_battery` flags.
            // ═══════════════════════════════════════════════════════
            system_conditions::start_environment_watch(app.handle().clone());

            // Initialize system tray (TASK-120), isolating any panic from the
            // appindicator FFI (TASK-154): if libayatana-appindicator3 is
            // missing, libappindicator-sys panics before returning an error.
            // catch_unwind contains it so the app degrades to tray-less mode.
            if let Err(e) = tray::setup_system_tray_isolated(app.handle()) {
                tracing::warn!(
                    "System tray unavailable (libayatana-appindicator3 missing?): tray disabled ({})",
                    e
                );
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if tray::is_close_to_tray_enabled() {
                    let _ = window.hide();
                    api.prevent_close();
                }
            }
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
            commands::sanitize_playlists,
            commands::preview_smart_playlist_count,
            commands::create_smart_playlist,
            commands::remove_track,
            commands::bulk_remove_tracks,
            commands::toggle_favorite,
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
            commands::reconcile_downloads_from_storage,
            commands::export_library,
            commands::import_library,
            commands::emit_test_notification,
            commands::show_in_folder,
            commands::get_track_metadata,
            commands::get_track_sources_availability,
            commands::check_track_availability,
            commands::check_tracks_availability,
            // Downloads & Queue Management (Canonical)
            commands::enqueue_download,
            commands::enqueue_tracks,
            commands::reconcile_queue,
            commands::add_to_queue,
            commands::add_batch_to_queue,
            commands::preflight_download_batch,
            commands::enqueue_eligible_batch,
            commands::get_queue,
            commands::get_queue_stats,
            commands::audit_download_queue,
            commands::reorder_queue,
            commands::update_queue_priority,
            commands::cancel_download,
            commands::cancel_queue_item,
            commands::retry_queue_item,
            commands::retry_failed,
            commands::clear_completed,
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
            commands::import_qobuz_purchases,
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
            commands::refresh_spotify_session,
            commands::validate_all_sessions,
            commands::spotify_auth_webview,
            // Lyrics
            commands::resolve_track_lyrics,
            commands::get_lyrics,
            commands::get_all_lyrics,
            commands::get_lyrics_stats,
            commands::save_lyrics,
            commands::import_lyrics_file,
            commands::probe_track_lyrics,
            commands::harvest_missing_lyrics,
            commands::get_lastfm_api_key_status,
            commands::set_lastfm_api_key,
            commands::delete_lyrics,
            commands::search_lyrics,
            commands::fetch_and_save_lyrics,
            commands::batch_fetch_lyrics,
            commands::batch_fetch_lyrics_with_progress,
            commands::fetch_missing_lyrics,
            commands::embed_lyrics,
            commands::batch_embed_lyrics,
            // S202: karaoke refetch + animated covers
            commands::refetch_karaoke_lyrics,
            commands::cancel_karaoke_refetch,
            commands::sweep_animated_covers,
            commands::cancel_animated_cover_sweep,
            // TASK-111: Sidecar completeness (LRC & covers)
            commands::materialize_missing_lrc_sidecars,
            commands::materialize_missing_covers,
            // S191: tag editor
            commands::read_track_tags,
            commands::write_track_tags,
            // S202 / TASK-7.1 CR-5: country & region tag repair (dry-run + apply)
            commands::plan_country_tag_repair,
            commands::apply_country_tag_repair,
            // S194: local playback of downloaded tracks
            commands::resolve_playback_source,
            // Downloads
            // Metadata Enrichment
            commands::enrich_metadata,
            commands::enrich_metadata_musicbrainz,
            commands::match_musicbrainz,
            commands::resolve_ghost_artists,
            commands::hydrate_stub_albums,
            commands::recalculate_album_total_tracks,
            commands::purge_orphan_empty_albums,
            commands::enrich_genre_lastfm,
            commands::enrich_track,
            commands::start_enrichment_worker,
            commands::pause_enrichment_worker,
            commands::resume_enrichment_worker,
            commands::get_enrichment_status,
            commands::preview_library_enrichment,
            commands::start_library_enrichment,
            commands::cancel_library_enrichment,
            commands::get_library_enrichment_status,
            // Fingerprinting
            commands::check_fingerprint_available,
            commands::identify_audio,
            commands::find_audio_duplicates,
            // Conversion (FFmpeg)
            commands::check_ffmpeg_available,
            // Local Library Scanner
            commands::scan_local_library,
            commands::get_local_track_metadata,
            // Progress-Enabled Commands
            commands::scan_local_library_with_progress,
            // FE-6: detiene el watcher de auto-rescan de una ruta de biblioteca
            commands::stop_library_watcher_command,
            commands::batch_enrich_metadata,
            // Playlist Commands
            commands::get_playlist_tracks,
            commands::fetch_remote_playlist_tracks,
            commands::export_playlist,
            commands::match_playlist_to_service,
            // Dependency Management
            commands::check_dependencies,
            commands::install_dependency,
            commands::install_all_dependencies,
            commands::ensure_dependency,
            // File export (dialog-resolved text writes)
            commands::write_text_file,
            // Single-Track Direct Pipeline
            commands::download_tidal_single_track,

            // Settings
            commands::get_kv_settings,
            // S203: global download quality ceiling
            commands::get_global_max_quality,
            commands::set_global_max_quality,
            commands::save_setting,
            commands::save_settings_batch,
            // Health Check
            commands::run_health_check,
            // Account Management
            commands::get_services,
            commands::get_accounts,
            commands::add_account,
            commands::remove_account,
            commands::update_account_sync_time,
            commands::toggle_account_active,
            commands::purge_stale_credentials,
            commands::get_service_auth_status,
            commands::sync_service,
            // Sprint 1: Service Preferences & Sync Settings
            commands::get_service_preferences,
            commands::update_service_preference,
            commands::reorder_service_priorities,
            commands::get_sync_settings,
            commands::update_sync_settings,
            commands::get_service_sync_settings,
            commands::update_service_sync_settings,
            commands::get_service_import_preferences,
            commands::update_service_import_preferences,
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
            commands::get_loudness_stats,
            commands::get_download_settings,
            commands::save_download_settings,
            commands::get_effective_download_preferences,
            commands::save_effective_download_preferences,
            commands::update_fallback_action,
            commands::force_redownload_tracks,
            commands::clear_download_history,
            // Sprint 3: Lyrics Tab + Settings
            commands::get_lyrics_providers,
            commands::update_lyrics_provider,
            commands::reorder_lyrics_providers,
            commands::get_lyrics_config,
            commands::update_lyrics_config,
            commands::test_lyrics_provider,
            // Sprint 4: Dashboard + Library Detail Views
            commands::reconcile_library_physical_state,
            commands::get_service_health,
            commands::run_batch_health_check,
            commands::create_library_snapshot,
            commands::get_library_snapshots,
            // Sprint 5: Advanced Settings & Polish
            commands::get_advanced_settings,
            commands::update_advanced_settings,
            commands::get_metadata_preferences,
            commands::update_metadata_preferences,
            commands::vacuum_database,
            commands::run_diagnostics,
            commands::reset_to_defaults,
            // Sprint 6: Migration Tab
            commands::get_migration_history,
            commands::get_migration_details,
            commands::get_migration_items_by_status,
            commands::preview_migration,
            commands::get_migration_destinations,
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
            commands::fetch_missing_cover_art,
            commands::get_tidal_repair_dry_run,
            commands::get_repair_history,
            commands::audit_catalog_identity,
            commands::plan_catalog_identity_repair,
            commands::apply_catalog_identity_repair,
            // S143B/S159: retroactive track version disambiguation repair
            commands::plan_disambiguation_repair,
            commands::execute_disambiguation_repair,
            commands::get_recovery_audit_summary,
            commands::trigger_startup_reconciliation,
            commands::get_concurrency_stats_summary,
            commands::get_active_concurrency_locks,
            commands::apply_musicbrainz_match,
            commands::reconcile_musicbrainz_tags,
            commands::get_storage_stats,
            commands::get_top_artists,
            commands::get_top_genres,
            commands::get_local_playlist_tracks,
            // S201: playlist download mode A (verify + M3U export)
            commands::export_playlist_m3u,
            // FE-6: import de playlists desde archivo (.m3u/.m3u8/.csv/.txt)
            commands::import_playlist_from_file,
            commands::get_audio_quality_distribution,
            commands::auto_resolve_duplicates,
            // Service Settings (Sprint 12)
            commands::get_app_settings,
            commands::service_save_settings,
            commands::get_default_download_path,
            commands::get_default_temp_path,
            commands::validate_directory_path,
            // System Logging (Sprint 170)
            commands::get_system_logs,
            commands::clear_system_logs,
            commands::export_system_logs,
            commands::record_system_log,
            commands::get_logging_status,
            // R15: log history read back from the rotating files on disk
            logs_history::read_log_history,
            logs_history::export_log_range,
            // User Reports from Help panel (FE-8)
            commands::save_user_report,
            // Local BPM & Tempo Analysis (Sprint 173)
            commands::analyze_library_bpm,
            commands::cancel_bpm_analysis,
            commands::update_track_bpm_manual,
            // System Tray & Desktop Notifications (TASK-120)
            tray::update_tray_icon,
            tray::update_tray_status,
            tray::update_tray_settings,
            tray::get_tray_settings,
            tray::show_notification,
        ])

        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
