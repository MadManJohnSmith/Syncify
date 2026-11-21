//! Comandos IPC del contrato de sincronización (issue #65, ADR-0006): disparo
//! manual por cuenta (`start_sync_run`), cancelación (`cancel_sync_run`) y
//! consulta para la ficha de cuenta (`get_sync_run`, `list_sync_runs`).
//!
//! Este ticket cubre Qobuz con scope `favorites`; el resto de proveedores y
//! colecciones llegan con sus tickets y responden `Unsupported` — la UI no
//! promete colecciones que el contrato aún no recorre.

use crate::commands::AppState;
use crate::sync::job::{self, RunMeta};
use crate::sync::runs::{self, SyncRunRecord};
use crate::sync::source::SyncScope;
use crate::sync::QobuzLibrarySource;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

/// Respuesta de `start_sync_run`: el id de la corrida ya persistida en estado
/// `running` (criterio 1 del issue #65).
#[derive(Debug, Serialize)]
pub struct StartSyncRunResponse {
    pub run_id: i64,
    pub status: String,
}

/// Respuesta de `cancel_sync_run`.
#[derive(Debug, Serialize)]
pub struct CancelSyncRunResponse {
    /// `true` si la petición de cancelación quedó registrada.
    pub cancel_requested: bool,
    /// Estado de la fila tras la petición (`running` si el job atenderá la
    /// bandera en el límite de página; `cancelled` si la corrida estaba
    /// huérfana y se cerró aquí).
    pub status: String,
}

async fn service_of_account(
    db: &sqlx::SqlitePool,
    account_id: i64,
) -> Result<(String, i64), String> {
    sqlx::query_as(
        "SELECT s.name, s.id FROM accounts a JOIN services s ON s.id = a.service_id WHERE a.id = ?",
    )
    .bind(account_id)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("sync run lookup error: {}", e))?
    .ok_or_else(|| format!("SyncRunNotFound: account {} does not exist", account_id))
}

#[tauri::command]
pub async fn start_sync_run(
    state: State<'_, AppState>,
    app: AppHandle,
    account_id: i64,
    scope: String,
) -> Result<StartSyncRunResponse, String> {
    let db = state.db.clone();
    let (service_name, service_id) = service_of_account(&db, account_id).await?;

    // #85 ciclo de vida: una cuenta desactivada o archivada no sincroniza,
    // tampoco por el contrato de corridas; el error nombra el estado para que
    // la ficha pueda ofrecer reactivar.
    crate::commands::accounts::ensure_account_can_sync(&db, account_id).await?;

    // Este ticket es el corte vertical de Qobuz + favoritos; el resto de
    // proveedores y colecciones siguen por el camino antiguo hasta sus tickets.
    if service_name != "qobuz" {
        return Err(format!(
            "Unsupported: SyncRun for {} arrives with its own ticket ({}; this is the Qobuz cut)",
            service_name, service_name
        ));
    }
    let scope_parsed = SyncScope::parse(&scope.trim())
        .ok_or_else(|| format!("Unsupported: unknown sync scope '{}'", scope.trim()))?;
    if scope_parsed != SyncScope::Favorites {
        return Err(format!(
            "Unsupported: sync scope '{}' is not part of the Qobuz cut yet (issue #65)",
            scope.trim()
        ));
    }

    if runs::has_running_run(&db, account_id).await? {
        return Err(
            "SyncRunBusy: a sync run is already active for this account; cancel it or wait"
                .to_string(),
        );
    }

    // Perfil resuelto (issue #69): el modo de la corrida sale del perfil
    // efectivo. El toggle de participación NO se consulta aquí: «sync ahora»
    // siempre puede forzar una corrida aunque la cuenta esté pausada
    // (ADR-0006, decisión 4).
    let profile = crate::sync::profile::load_sync_profile(&db, account_id).await?;
    let run_id = runs::insert_running_run(
        &db,
        account_id,
        service_id,
        scope_parsed,
        profile.mode.as_str(),
    )
    .await?;

    let meta = RunMeta {
        run_id,
        account_id,
        service: service_name.clone(),
        scope: scope_parsed.as_str().to_string(),
    };
    let cancel = job::register_live_run(run_id);
    let db_for_job = db.clone();

    tauri::async_runtime::spawn(async move {
        // Solo favoritos: el alcance del corte. Sin red en tests vía el
        // proveedor falso o la costura SYNCIFY_QOBUZ_API_BASE (PR #298).
        let prefs = crate::commands::ImportPreferences {
            service_name: service_name.clone(),
            favorite_tracks: true,
            favorite_albums: false,
            favorite_artists: false,
            playlists: false,
            purchases: false,
            library_history: false,
            // Las apariciones son evidencia de playlists: su ticket llega con
            // el corte de playlists (seam 1 del spec).
            include_appearances: false,
            force_retry_unavailable: false,
        };

        let result =
            execute_qobuz_favorites_run(&db_for_job, &meta, service_id, &prefs, &app, cancel).await;

        if let Err(error) = result {
            // El job ya cerró su fila con `failed`; aquí solo queda el trazo
            // del error de entorno que impidió llegar a la corrida.
            tracing::error!("sync run {} failed: {}", meta.run_id, error);
        }
    });

    Ok(StartSyncRunResponse {
        run_id,
        status: "running".to_string(),
    })
}

/// Ejecuta la corrida real de Qobuz + favoritos: resuelve la credencial
/// almacenada, toma los mismos candados de escritor que el orquestador y
/// corre el job genérico sobre el adaptador Qobuz. Cualquier fallo previo a
/// la corrida cierra la fila como `failed` (criterio 3: credencial inválida →
/// failed).
async fn execute_qobuz_favorites_run(
    db: &sqlx::SqlitePool,
    meta: &RunMeta,
    service_id: i64,
    prefs: &crate::commands::ImportPreferences,
    app: &AppHandle,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), String> {
    let started = std::time::Instant::now();

    let fail_run = |db: &sqlx::SqlitePool, message: &str| {
        let db = db.clone();
        let run_id = meta.run_id;
        let account_id = meta.account_id;
        let service = meta.service.clone();
        let scope = meta.scope.clone();
        let elapsed = started.elapsed().as_millis() as u64;
        let message = message.to_string();
        async move {
            crate::sync::job::unregister_live_run(run_id);
            let _ = runs::record_run_error(&db, run_id, "favorites", &message).await;
            let _ = runs::finish_run(&db, run_id, "failed", None, 0, 1).await;
            let _ = runs::prune_runs(&db, account_id, runs::KEEP_RUNS_PER_ACCOUNT).await;
            let _ = app.emit(
                "sync-run-error",
                crate::sync::SyncRunEvent::finished(
                    run_id, account_id, &service, &scope, "failed", 0, 0, None, 1, elapsed, message,
                ),
            );
        }
    };

    let creds_json: Option<(String,)> =
        sqlx::query_as("SELECT credentials_json FROM accounts WHERE id = ?")
            .bind(meta.account_id)
            .fetch_optional(db)
            .await
            .map_err(|e| {
                format!(
                    "Database error loading credentials for account {}: {}",
                    meta.account_id, e
                )
            })?;

    let decrypted = match creds_json {
        Some((c,)) if !c.trim().is_empty() => crate::crypto::decrypt(&c)?,
        _ => {
            let message = format!(
                "RequiresAuth: Credentials missing for account {}",
                meta.account_id
            );
            fail_run(db, &message).await;
            return Err(message);
        }
    };
    let creds: serde_json::Value = match serde_json::from_str(&decrypted) {
        Ok(creds) => creds,
        Err(e) => {
            let message = format!("RequiresAuth: Malformed credentials JSON: {}", e);
            fail_run(db, &message).await;
            return Err(message);
        }
    };

    let token =
        match crate::commands::service::resolve_qobuz_user_auth_token(db, meta.account_id, &creds)
            .await
        {
            Ok(token) => token,
            Err(err_msg) => {
                fail_run(db, &err_msg).await;
                return Err(err_msg);
            }
        };

    let app_id = std::env::var("QOBUZ_APP_ID")
        .unwrap_or_else(|_| crate::services::qobuz::QOBUZ_APP_ID.to_string());
    let app_secret = std::env::var("QOBUZ_APP_SECRET")
        .unwrap_or_else(|_| crate::services::qobuz::QOBUZ_APP_SECRET.to_string());
    let client = crate::services::QobuzClient::new_with_token(app_id, app_secret, token);

    // Mismos candados que el orquestador: un sync del camino antiguo y una
    // corrida SyncRun no pueden escribir la misma cuenta a la vez.
    let _sync_writer_guard =
        match crate::db::SyncWriterLock::acquire(db, &meta.service, meta.account_id).await {
            Ok(guard) => Some(guard),
            Err(error) => {
                let message = if error.starts_with("Ya existe una sincronización de ") {
                    "SyncWriterBusy: another sync is writing this account; retry after it finishes"
                        .to_string()
                } else {
                    error
                };
                fail_run(db, &message).await;
                return Err(message);
            }
        };

    let source = QobuzLibrarySource::new(&client);
    let result =
        job::run_favorites_job(db, meta, service_id, &source, prefs, Some(app), app, cancel).await;
    match result {
        Ok(outcome) => {
            tracing::info!(
                "sync run {} finished: {} (imported {})",
                meta.run_id,
                outcome.status,
                outcome.imported_total
            );
            Ok(())
        }
        Err(error) => Err(error),
    }
}

#[tauri::command]
pub async fn cancel_sync_run(
    state: State<'_, AppState>,
    run_id: i64,
) -> Result<CancelSyncRunResponse, String> {
    let db = state.db.clone();

    // Corrida viva: la bandera se marca y el job atiende en el límite de
    // página conservando lo importado (criterio 2).
    if job::request_cancel(run_id) {
        return Ok(CancelSyncRunResponse {
            cancel_requested: true,
            status: "running".to_string(),
        });
    }

    // Corrida huérfana (cierre brusco): la fila sigue `running` sin nadie que
    // la atienda; se cierra como cancelled aquí mismo.
    let record = runs::get_run(&db, run_id)
        .await?
        .ok_or_else(|| format!("SyncRunNotFound: no sync run with id {}", run_id))?;
    if record.status == "running" {
        runs::finish_run(
            &db,
            run_id,
            "cancelled",
            record.reported_total,
            record.imported_total,
            record.error_count,
        )
        .await?;
        return Ok(CancelSyncRunResponse {
            cancel_requested: true,
            status: "cancelled".to_string(),
        });
    }

    Ok(CancelSyncRunResponse {
        cancel_requested: false,
        status: record.status,
    })
}

#[tauri::command]
pub async fn get_sync_run(
    state: State<'_, AppState>,
    run_id: i64,
) -> Result<SyncRunRecord, String> {
    runs::get_run(&state.db, run_id)
        .await?
        .ok_or_else(|| format!("SyncRunNotFound: no sync run with id {}", run_id))
}

#[tauri::command]
pub async fn list_sync_runs(
    state: State<'_, AppState>,
    account_id: i64,
    limit: Option<u32>,
) -> Result<Vec<SyncRunRecord>, String> {
    let limit = limit
        .unwrap_or(runs::KEEP_RUNS_PER_ACCOUNT as u32)
        .clamp(1, 50) as i64;
    runs::list_runs(&state.db, account_id, limit).await
}
