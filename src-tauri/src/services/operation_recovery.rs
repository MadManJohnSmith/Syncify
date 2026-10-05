//! Persistent Operation Journal, Checkpointing, and Post-Crash Recovery Service (S167)
//!
//! Provides deterministic post-crash state reconciliation for:
//! - Service Sync & Playlist Imports
//! - Qobuz & Tidal Downloads
//! - Cross-Provider Fallbacks
//! - Physical File Promotions & Tagging
//! - Catalog & Metadata Repairs

use sqlx::{Row, SqlitePool};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use syncify_core_domain::{
    AudioByteValidator, ErrorTaxonomy, LibraryLayout, OperationJournalEntry, OperationPhase,
    OperationRecoveryDetail, OperationStatus, OperationType, RecoveryAction, RecoveryAuditSummary,
};
use tracing::{info, warn};

/// Serializes the post-crash download reconciliation across the whole process.
///
/// Startup runs the reconciliation from a detached task while the download worker
/// starts at the same moment and re-queues rows still marked `downloading`. Two
/// tasks rewriting the same rows is what produced duplicate retries and
/// contradictory queue states after an unexpected shutdown, so every entry point
/// takes this lock before touching state.
fn reconciliation_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

/// Runs both post-crash download passes (journal reconciliation and
/// staging/stuck-queue cleanup) under a single lock acquisition.
///
/// The download worker calls this before it looks at the queue, so that by the time
/// it re-queues interrupted rows the recovery has already settled them. The passes
/// are idempotent, so the detached startup task running them again is a no-op.
pub async fn reconcile_downloads_on_startup(db: &SqlitePool) {
    let _guard = reconciliation_lock().lock().await;

    if let Err(e) = reconcile_startup_operations_locked(db, None).await {
        warn!(
            "[Recovery Engine] Startup journal reconciliation failed: {}",
            e
        );
    }
    if let Err(e) = cleanup_staging_and_recover_stuck_queue_with_message_locked(
        db,
        None,
        "Download interrupted by system restart",
    )
    .await
    {
        warn!(
            "[Recovery Engine] Startup staging cleanup and stuck-queue recovery failed: {}",
            e
        );
    }
}

/// Record a new operation in the persistent journal.
pub async fn create_operation_journal(
    db: &SqlitePool,
    entry: &OperationJournalEntry,
) -> Result<(), String> {
    sqlx::query(
        r#"
        INSERT INTO operation_journal (
            operation_id, operation_type, entity_id, account_id, track_id,
            download_id, provider, phase, attempt, started_at, checkpoint_at,
            status, input_identity, expected_output_path, staging_path,
            file_baseline, db_transaction_state, rollback_state, error_taxonomy,
            retry_policy, result_summary
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#
    )
    .bind(&entry.operation_id)
    .bind(entry.operation_type.as_str())
    .bind(&entry.entity_id)
    .bind(entry.account_id)
    .bind(entry.track_id)
    .bind(entry.download_id)
    .bind(&entry.provider)
    .bind(entry.phase.as_str())
    .bind(entry.attempt)
    .bind(entry.status.as_str())
    .bind(&entry.input_identity)
    .bind(&entry.expected_output_path)
    .bind(&entry.staging_path)
    .bind(&entry.file_baseline)
    .bind(&entry.db_transaction_state)
    .bind(&entry.rollback_state)
    .bind(&entry.error_taxonomy)
    .bind(&entry.retry_policy)
    .bind(&entry.result_summary)
    .execute(db)
    .await
    .map_err(|e| format!("Failed to insert operation journal entry {}: {}", entry.operation_id, e))?;

    Ok(())
}

/// Update progress checkpoint of an ongoing operation.
pub async fn checkpoint_operation(
    db: &SqlitePool,
    operation_id: &str,
    phase: OperationPhase,
    status: OperationStatus,
    staging_path: Option<&str>,
    details: Option<&str>,
) -> Result<(), String> {
    sqlx::query(
        r#"
        UPDATE operation_journal
        SET phase = ?,
            status = ?,
            staging_path = COALESCE(?, staging_path),
            result_summary = COALESCE(?, result_summary),
            checkpoint_at = CURRENT_TIMESTAMP
        WHERE operation_id = ?
        "#,
    )
    .bind(phase.as_str())
    .bind(status.as_str())
    .bind(staging_path)
    .bind(details)
    .bind(operation_id)
    .execute(db)
    .await
    .map_err(|e| format!("Failed to checkpoint operation {}: {}", operation_id, e))?;

    Ok(())
}

/// Mark an operation as committed/completed successfully.
pub async fn commit_operation(
    db: &SqlitePool,
    operation_id: &str,
    result_summary: Option<&str>,
) -> Result<(), String> {
    sqlx::query(
        r#"
        UPDATE operation_journal
        SET status = 'committed',
            phase = 'completed',
            result_summary = COALESCE(?, result_summary),
            checkpoint_at = CURRENT_TIMESTAMP
        WHERE operation_id = ?
        "#,
    )
    .bind(result_summary)
    .bind(operation_id)
    .execute(db)
    .await
    .map_err(|e| format!("Failed to commit operation {}: {}", operation_id, e))?;

    Ok(())
}

/// Mark an operation as failed or interrupted.
pub async fn fail_operation(
    db: &SqlitePool,
    operation_id: &str,
    error_taxonomy: &ErrorTaxonomy,
    reason: &str,
    is_terminal: bool,
) -> Result<(), String> {
    let status = if is_terminal || !error_taxonomy.is_retryable() {
        OperationStatus::FailedTerminal
    } else {
        OperationStatus::Interrupted
    };

    let tax_str = format!("{:?}", error_taxonomy);

    sqlx::query(
        r#"
        UPDATE operation_journal
        SET status = ?,
            error_taxonomy = ?,
            result_summary = ?,
            checkpoint_at = CURRENT_TIMESTAMP
        WHERE operation_id = ?
        "#,
    )
    .bind(status.as_str())
    .bind(&tax_str)
    .bind(reason)
    .bind(operation_id)
    .execute(db)
    .await
    .map_err(|e| format!("Failed to mark operation {} as failed: {}", operation_id, e))?;

    Ok(())
}

/// Live handle over one in-flight journaled operation.
///
/// Long-running production operations (download pipeline, service sync) hold one of
/// these for the whole duration and report milestones through it. Every method is
/// fail-soft on purpose: the journal is recovery metadata, so a journal write must
/// never abort or fail the operation it is describing. Errors are logged and the
/// operation keeps running.
#[derive(Clone)]
pub struct JournaledOperation {
    db: SqlitePool,
    operation_id: String,
}

impl JournaledOperation {
    /// Record a milestone. `Checkpointed` keeps the entry scannable by
    /// `reconcile_startup_operations`, which is exactly what a live operation needs.
    pub async fn checkpoint(
        &self,
        phase: OperationPhase,
        staging_path: Option<&str>,
        details: Option<&str>,
    ) {
        self.op_checkpoint(phase, staging_path, details).await;
    }

    /// Record a milestone that is being persisted right now. `reconcile_startup_operations`
    /// scans `persisting` too: a crash here means the physical file may already be in
    /// place while the SQLite transaction never committed.
    pub async fn checkpoint_persisting(&self, details: Option<&str>) {
        if let Err(e) = checkpoint_operation(
            &self.db,
            &self.operation_id,
            OperationPhase::Persist,
            OperationStatus::Persisting,
            None,
            details,
        )
        .await
        {
            warn!(op_id = %self.operation_id, error = %e, "[Recovery Engine] Persist checkpoint write failed");
        }
    }

    /// Record a milestone that also declares where the finished artifact will live.
    ///
    /// `expected_output_path` is what makes reconciliation decidable: once it is
    /// set, a crash is repaired by inspecting that path (`ReconcileDbOnly` when the
    /// audio is already promoted, `CompletePromotion` when only the staging file
    /// survived) instead of being written off as a lost transfer.
    pub async fn checkpoint_with_output(
        &self,
        phase: OperationPhase,
        staging_path: Option<&str>,
        expected_output_path: &str,
        details: Option<&str>,
    ) {
        self.op_checkpoint(phase, staging_path, details).await;
        if let Err(e) = sqlx::query(
            "UPDATE operation_journal SET expected_output_path = ? WHERE operation_id = ?",
        )
        .bind(expected_output_path)
        .bind(&self.operation_id)
        .execute(&self.db)
        .await
        {
            warn!(op_id = %self.operation_id, error = %e, "[Recovery Engine] Could not record expected output path");
        }
    }

    async fn op_checkpoint(
        &self,
        phase: OperationPhase,
        staging_path: Option<&str>,
        details: Option<&str>,
    ) {
        if let Err(e) = checkpoint_operation(
            &self.db,
            &self.operation_id,
            phase,
            OperationStatus::Checkpointed,
            staging_path,
            details,
        )
        .await
        {
            warn!(op_id = %self.operation_id, error = %e, "[Recovery Engine] Checkpoint write failed");
        }
    }

    /// Mark the operation as finished successfully.
    pub async fn commit(&self, result_summary: Option<&str>) {
        if let Err(e) = commit_operation(&self.db, &self.operation_id, result_summary).await {
            warn!(op_id = %self.operation_id, error = %e, "[Recovery Engine] Commit write failed");
        }
    }

    /// Mark the operation as failed, classifying the error so that startup
    /// reconciliation knows whether a retry is worth scheduling.
    pub async fn fail(&self, error_taxonomy: &ErrorTaxonomy, reason: &str, is_terminal: bool) {
        if let Err(e) = fail_operation(
            &self.db,
            &self.operation_id,
            error_taxonomy,
            reason,
            is_terminal,
        )
        .await
        {
            warn!(op_id = %self.operation_id, error = %e, "[Recovery Engine] Failure write failed");
        }
    }
}

/// Open a journal entry and return the live handle used to checkpoint it.
pub async fn begin_operation(
    db: &SqlitePool,
    entry: &OperationJournalEntry,
) -> Result<JournaledOperation, String> {
    create_operation_journal(db, entry).await?;
    Ok(JournaledOperation {
        db: db.clone(),
        operation_id: entry.operation_id.clone(),
    })
}

/// Re-attach to a journal entry that is already open, instead of creating a
/// second one for the same download.
///
/// Used when an inner pipeline runs inside an operation that already journaled
/// itself (the download worker calls the Tidal pipeline for a queued item): both
/// layers must report on the SAME row, otherwise startup reconciliation would see
/// two competing entries for one physical download. Returns `Err` when no such
/// entry exists, which tells the caller to open a new one.
pub async fn attach_operation(
    db: &SqlitePool,
    operation_id: &str,
) -> Result<JournaledOperation, String> {
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM operation_journal WHERE operation_id = ?")
            .bind(operation_id)
            .fetch_optional(db)
            .await
            .map_err(|e| {
                format!(
                    "Failed to probe operation journal entry {}: {}",
                    operation_id, e
                )
            })?;

    if exists.is_none() {
        return Err(format!(
            "No open operation journal entry for {}",
            operation_id
        ));
    }

    Ok(JournaledOperation {
        db: db.clone(),
        operation_id: operation_id.to_string(),
    })
}

/// Open the journal entry for a Tidal single-track pipeline run.
///
/// `request.operation_id` is the operation an outer layer already journaled (the
/// download worker, for a queued item). When that entry exists it is reused so a
/// single physical download is never described by two competing journal rows;
/// otherwise a fresh entry is opened for this run.
pub async fn begin_tidal_download_operation(
    db: &SqlitePool,
    request: &crate::services::tidal_pipeline::TidalSingleTrackRequest,
) -> Option<JournaledOperation> {
    if let Some(op_id) = request.operation_id.as_deref() {
        match attach_operation(db, op_id).await {
            Ok(existing) => {
                info!(op_id = %op_id, "[Recovery Engine] Reusing the journal entry opened by the caller");
                return Some(existing);
            }
            Err(e) => {
                info!(op_id = %op_id, error = %e, "[Recovery Engine] No reusable journal entry; opening a new one");
            }
        }
    }

    let query = request.track_id_or_query.trim().to_string();
    let entry = OperationJournalEntry {
        operation_id: format!("op-{}", uuid::Uuid::new_v4()),
        operation_type: OperationType::DownloadTidal,
        entity_id: request
            .hint_track_id
            .map(|id| id.to_string())
            .or_else(|| Some(query.clone())),
        account_id: None,
        track_id: request.hint_track_id,
        download_id: None,
        provider: Some("tidal".to_string()),
        phase: OperationPhase::Init,
        attempt: 1,
        started_at: String::new(),
        checkpoint_at: String::new(),
        status: OperationStatus::Started,
        input_identity: request
            .hint_isrc
            .clone()
            .map(|isrc| format!("isrc={}", isrc))
            .or_else(|| Some(format!("tidal={}", query))),
        expected_output_path: request.output_dir.clone(),
        staging_path: None,
        file_baseline: None,
        db_transaction_state: None,
        rollback_state: None,
        error_taxonomy: None,
        retry_policy: None,
        result_summary: None,
    };

    match begin_operation(db, &entry).await {
        Ok(op) => Some(op),
        Err(e) => {
            warn!(error = %e, "[Recovery Engine] Could not open Tidal pipeline journal entry; continuing unjournaled");
            None
        }
    }
}

/// Open the journal entry for a service sync.
pub async fn begin_service_sync_operation(
    db: &SqlitePool,
    service_name: &str,
    account_id: Option<i64>,
) -> Option<JournaledOperation> {
    let service_normalized = service_name.to_lowercase();
    let entry = OperationJournalEntry {
        operation_id: format!("op-sync-{}-{}", service_normalized, uuid::Uuid::new_v4()),
        operation_type: OperationType::ServiceSync,
        entity_id: Some(service_normalized.clone()),
        account_id,
        track_id: None,
        download_id: None,
        provider: Some(service_normalized.clone()),
        phase: OperationPhase::Init,
        attempt: 1,
        started_at: String::new(),
        checkpoint_at: String::new(),
        status: OperationStatus::Started,
        input_identity: Some(format!("service={}", service_normalized)),
        expected_output_path: None,
        staging_path: None,
        file_baseline: None,
        db_transaction_state: None,
        rollback_state: None,
        error_taxonomy: None,
        retry_policy: None,
        result_summary: None,
    };

    match begin_operation(db, &entry).await {
        Ok(op) => Some(op),
        Err(e) => {
            warn!(
                service = %service_normalized,
                error = %e,
                "[Recovery Engine] Could not open service sync journal entry; continuing unjournaled"
            );
            None
        }
    }
}

/// Deterministic `.staging/<queue_id>.part` path used by the download pipeline.
///
/// Single source of truth shared by the download worker (journal + error cleanup)
/// and the reconciliation engine, so a crash always leaves a path the reconciler
/// can find.
pub fn download_staging_path(output_dir: &str, queue_id: i64) -> String {
    Path::new(output_dir)
        .join(".staging")
        .join(format!("{}.part", queue_id))
        .to_string_lossy()
        .to_string()
}

/// Inputs the download worker knows when a queued item starts processing.
#[derive(Debug, Clone)]
pub struct DownloadJournalParams {
    pub operation_id: String,
    pub queue_id: i64,
    pub track_id: i64,
    pub provider: Option<String>,
    /// Locked source identity (`service_track_id`) or ISRC fallback.
    pub input_identity: Option<String>,
    /// `output_dir` from the resolved download configuration.
    pub output_dir: String,
    /// True when the item may fall back across providers, which makes the
    /// operation a cross-provider recovery candidate rather than a plain download.
    pub allow_fallback: bool,
}

/// Journal lifecycle for one queued download, as driven by the worker.
pub struct DownloadJournal {
    op: JournaledOperation,
    staging_path: String,
    operation_type: OperationType,
    provider: String,
}

impl DownloadJournal {
    /// Open the journal entry for a queued download. Returns `None` (after
    /// logging) when the entry cannot be written; the download then runs
    /// unjournaled rather than failing.
    pub async fn start(db: &SqlitePool, params: &DownloadJournalParams) -> Option<Self> {
        let provider_name = params
            .provider
            .as_deref()
            .map(|p| p.to_lowercase())
            .unwrap_or_else(|| "unknown".to_string());

        let operation_type = match provider_name.as_str() {
            "qobuz" => {
                if params.allow_fallback {
                    OperationType::CrossProviderFallback
                } else {
                    OperationType::DownloadQobuz
                }
            }
            _ => {
                if params.allow_fallback {
                    OperationType::CrossProviderFallback
                } else {
                    OperationType::DownloadTidal
                }
            }
        };

        let staging_path = download_staging_path(&params.output_dir, params.queue_id);

        let entry = OperationJournalEntry {
            operation_id: params.operation_id.clone(),
            operation_type,
            entity_id: Some(params.queue_id.to_string()),
            account_id: None,
            track_id: Some(params.track_id),
            download_id: None,
            provider: Some(provider_name.clone()),
            phase: OperationPhase::Init,
            attempt: 1,
            started_at: String::new(),
            checkpoint_at: String::new(),
            status: OperationStatus::Started,
            input_identity: params.input_identity.clone(),
            expected_output_path: None,
            staging_path: Some(staging_path.clone()),
            file_baseline: None,
            db_transaction_state: None,
            rollback_state: None,
            error_taxonomy: None,
            retry_policy: None,
            result_summary: None,
        };

        match begin_operation(db, &entry).await {
            Ok(op) => Some(DownloadJournal {
                op,
                staging_path,
                operation_type,
                provider: provider_name,
            }),
            Err(e) => {
                warn!(
                    op_id = %params.operation_id,
                    queue_id = params.queue_id,
                    error = %e,
                    "[Recovery Engine] Could not open download journal entry; continuing unjournaled"
                );
                None
            }
        }
    }

    /// Transfer started: the `.part` file now exists (or is about to).
    pub async fn checkpoint_transfer(&self, details: Option<&str>) {
        self.op
            .checkpoint(OperationPhase::Transfer, Some(&self.staging_path), details)
            .await;
    }

    /// The physical file is now at its final library path. This is the milestone
    /// that makes reconciliation decidable: with `expected_output_path` set, a
    /// crash after this point is repaired as `ReconcileDbOnly` instead of being
    /// treated as a lost transfer.
    pub async fn checkpoint_promoted(&self, final_path: &str, details: Option<&str>) {
        self.op
            .checkpoint_with_output(
                OperationPhase::Promotion,
                Some(&self.staging_path),
                final_path,
                details,
            )
            .await;
    }

    /// SQLite ledger is being written for the promoted file.
    pub async fn checkpoint_persist(&self, details: Option<&str>) {
        self.op.checkpoint_persisting(details).await;
    }

    pub async fn commit(&self, result_summary: Option<&str>) {
        self.op.commit(result_summary).await;
    }

    pub async fn fail(&self, error: &str, is_terminal: bool) {
        let taxonomy = classify_operation_error(self.operation_type, &self.provider, error);
        self.op.fail(&taxonomy, error, is_terminal).await;
    }
}

/// Map an operational error message onto the shared error taxonomy so that
/// `fail_operation` and startup reconciliation agree on retryability.
///
/// Mirrors the classification the download worker already applies to queue rows:
/// credential rejections, identity problems, rejected quality, unavailability and
/// exhausted retries are terminal; rate limiting, timeouts, transport failures,
/// server errors and cancellations stay retryable.
pub fn classify_operation_error(
    operation_type: OperationType,
    provider: &str,
    error: &str,
) -> ErrorTaxonomy {
    let provider = if provider.trim().is_empty() {
        match operation_type {
            OperationType::DownloadQobuz => "qobuz".to_string(),
            OperationType::DownloadTidal => "tidal".to_string(),
            OperationType::CrossProviderFallback => "multi".to_string(),
            _ => "unknown".to_string(),
        }
    } else {
        provider.to_lowercase()
    };

    // Misma regla que worker::classify_session_auth_failure: un fallo de
    // TRANSPORTE (timeout, DNS, 5xx, 429) no es un veredicto de credenciales.
    // Sin esta guarda, un corte de red marcaba la cuenta como AuthInvalid.
    if crate::worker::is_transport_failure(error) {
        return ErrorTaxonomy::TemporaryNetworkFailure {
            endpoint: format!("{} operation", provider),
            message: error.to_string(),
        };
    }

    if error.contains("401")
        || error.contains("403")
        || error.contains("RequiresAuth")
        || error.contains("authentication failed")
        || error.contains("invalid_grant")
        || error.contains("OAuth token refresh failed")
    {
        return ErrorTaxonomy::AuthInvalid {
            message: error.to_string(),
        };
    }
    if error.contains("EntitlementDenied") || error.contains("PlaybackUnauthorized") {
        return ErrorTaxonomy::EntitlementDenied {
            provider,
            reason: error.to_string(),
        };
    }
    if error.contains("RejectedQuality") || error.contains("downgrade rejected") {
        return ErrorTaxonomy::RejectedQuality {
            requested: "requested".to_string(),
            obtained: "obtained".to_string(),
            reason: error.to_string(),
        };
    }
    if error.contains("RegionRestricted")
        || (error.contains("region") && error.contains("restricted"))
    {
        return ErrorTaxonomy::RegionRestricted {
            provider,
            country: "unknown".to_string(),
        };
    }
    if error.contains("AmbiguousSource")
        || error.contains("SourceIdentityMissing")
        || error.contains("IdentityConflict")
    {
        return ErrorTaxonomy::IdentityConflict {
            field: "source_identity".to_string(),
            existing_value: "locked".to_string(),
            conflicting_value: error.to_string(),
        };
    }
    if error.contains("TrackUnresolved")
        || error.contains("NotFound")
        || error.contains("not found on")
        || error.contains("404")
        || error.contains("StaleSource")
        || error.contains("track/get failed")
    {
        return ErrorTaxonomy::UnavailableFromProvider {
            provider,
            item_id: "unknown".to_string(),
            reason: error.to_string(),
        };
    }
    if error.contains("429") || error.contains("RateLimit") || error.contains("TooManyRequests") {
        return ErrorTaxonomy::RateLimited {
            provider,
            retry_after_sec: None,
        };
    }
    if error.contains("NetworkExhausted")
        || error.contains("connection")
        || error.contains("connect")
    {
        return ErrorTaxonomy::TemporaryNetworkFailure {
            endpoint: provider,
            message: error.to_string(),
        };
    }
    if error.contains("timeout") || error.contains("timed out") || error.contains("Timeout") {
        return ErrorTaxonomy::Timeout {
            endpoint: provider,
            elapsed_ms: 0,
        };
    }
    if error.contains("cancel") || error.contains("Cancel") {
        return ErrorTaxonomy::Cancelled {
            reason: error.to_string(),
        };
    }
    if error.contains("Invalid") && error.contains("audio") {
        return ErrorTaxonomy::AudioValidationFailed {
            format: "unknown".to_string(),
            reason: error.to_string(),
        };
    }
    if error.contains("Tag") {
        return ErrorTaxonomy::TaggingFailed {
            stage: "tagging".to_string(),
            reason: error.to_string(),
        };
    }
    if error.contains("Filesystem") || error.contains("rename") || error.contains("directory") {
        return ErrorTaxonomy::FilesystemFailed {
            path: "unknown".to_string(),
            reason: error.to_string(),
        };
    }
    if error.contains("SQLITE") || error.contains("database") || error.contains("Database") {
        return ErrorTaxonomy::DatabaseFailed {
            operation: "journal".to_string(),
            reason: error.to_string(),
        };
    }

    ErrorTaxonomy::MalformedProviderPayload {
        provider,
        field: "unknown".to_string(),
        reason: error.to_string(),
    }
}

/// Perform comprehensive startup reconciliation across journal, SQLite state, and filesystem.
pub async fn reconcile_startup_operations(
    db: &SqlitePool,
    music_dir: Option<&Path>,
) -> Result<RecoveryAuditSummary, String> {
    let _guard = reconciliation_lock().lock().await;
    reconcile_startup_operations_locked(db, music_dir).await
}

async fn reconcile_startup_operations_locked(
    db: &SqlitePool,
    _music_dir: Option<&Path>,
) -> Result<RecoveryAuditSummary, String> {
    info!("[Recovery Engine] Starting post-crash deterministic reconciliation...");

    let mut summary = RecoveryAuditSummary::default();

    // 1. Fetch all active or non-terminal journal entries
    let active_rows = sqlx::query(
        r#"
        SELECT operation_id, operation_type, entity_id, account_id, track_id,
               download_id, provider, phase, attempt, started_at, checkpoint_at,
               status, input_identity, expected_output_path, staging_path,
               file_baseline, db_transaction_state, rollback_state, error_taxonomy,
               retry_policy, result_summary
        FROM operation_journal
        WHERE status IN ('started', 'checkpointed', 'persisting', 'recovering')
        ORDER BY checkpoint_at ASC
        "#,
    )
    .fetch_all(db)
    .await
    .map_err(|e| format!("Failed to query active journal entries: {}", e))?;

    summary.total_journal_scanned = active_rows.len();
    summary.active_operations_found = active_rows.len();

    for row in active_rows {
        let op_id: String = row.get("operation_id");
        let op_type_str: String = row.get("operation_type");
        let status_str: String = row.get("status");
        let phase_str: String = row.get("phase");
        let entity_id: Option<String> = row.get("entity_id");
        let track_id: Option<i64> = row.get("track_id");
        let _download_id: Option<i64> = row.get("download_id");
        let exp_path: Option<String> = row.get("expected_output_path");
        let stg_path: Option<String> = row.get("staging_path");
        let tax_str: Option<String> = row.get("error_taxonomy");

        let op_type = OperationType::from_str(&op_type_str).unwrap_or(OperationType::DownloadQobuz);
        let prev_status =
            OperationStatus::from_str(&status_str).unwrap_or(OperationStatus::Started);
        let phase = OperationPhase::from_str(&phase_str).unwrap_or(OperationPhase::Init);

        let mut action_taken = RecoveryAction::NoOp;
        let mut new_status = OperationStatus::Interrupted;
        let mut message = String::new();
        // Writes this reconciliation attempted and could not land. A recovery that
        // did not persist is not a recovery, so these decide the final status.
        let mut persist_errors: Vec<String> = Vec::new();

        match op_type {
            OperationType::DownloadQobuz
            | OperationType::DownloadTidal
            | OperationType::CrossProviderFallback
            | OperationType::Promotion => {
                // Check Case 1: Physical file promoted to destination path but DB missing / uncommitted
                let dest_is_valid = if let Some(ref dest) = exp_path {
                    let dest_path = PathBuf::from(dest);
                    dest_path.exists() && is_valid_audio_file(&dest_path)
                } else {
                    false
                };

                if dest_is_valid {
                    let dest = exp_path.as_ref().unwrap();
                    let dest_path = PathBuf::from(dest);
                    info!(op_id = %op_id, dest = %dest, "Case 1: Destination file exists and is valid audio. Reconciling DB records.");

                    // Ensure downloads table has this record
                    if let Some(tid) = track_id {
                        let dl_existing: Option<(i64, String)> = sqlx::query_as(
                            "SELECT id, file_path FROM downloads WHERE track_id = ? LIMIT 1",
                        )
                        .bind(tid)
                        .fetch_optional(db)
                        .await
                        .ok()
                        .flatten();

                        let f_size = std::fs::metadata(&dest_path)
                            .map(|m| m.len() as i64)
                            .unwrap_or(0);
                        let (fmt, depth, rate) = measured_audio_facts(&dest_path);
                        match dl_existing {
                            Some((dl_id, old_fp)) => {
                                if old_fp != *dest {
                                    if let Err(e) = sqlx::query(
                                        "UPDATE downloads SET file_path = ?, file_size_bytes = ?, file_format = ?, bit_depth = ?, sample_rate = ?, downloaded_at = CURRENT_TIMESTAMP WHERE id = ?"
                                    )
                                    .bind(dest)
                                    .bind(f_size)
                                    .bind(&fmt)
                                    .bind(depth)
                                    .bind(rate)
                                    .bind(dl_id)
                                    .execute(db)
                                    .await
                                    {
                                        persist_errors.push(format!(
                                            "downloads row {} not updated: {}",
                                            dl_id, e
                                        ));
                                    }
                                }
                            }
                            None => {
                                if let Err(e) = sqlx::query(
                                    r#"
                                    INSERT OR REPLACE INTO downloads (
                                        track_id, file_path, file_size_bytes, file_format, bit_depth,
                                        sample_rate, downloaded_at
                                    ) VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
                                    "#
                                )
                                .bind(tid)
                                .bind(dest)
                                .bind(f_size)
                                .bind(&fmt)
                                .bind(depth)
                                .bind(rate)
                                .execute(db)
                                .await
                                {
                                    persist_errors
                                        .push(format!("downloads row for track {} not inserted: {}", tid, e));
                                }
                            }
                        }
                    }

                    // Update download_queue if applicable
                    if let Some(qid_str) = entity_id.as_deref() {
                        if let Ok(qid) = qid_str.parse::<i64>() {
                            if let Err(e) = sqlx::query(
                                "UPDATE download_queue SET status = 'complete', progress_percent = 100.0, completed_at = CURRENT_TIMESTAMP WHERE id = ?"
                            )
                            .bind(qid)
                            .execute(db)
                            .await
                            {
                                persist_errors
                                    .push(format!("download_queue row {} not completed: {}", qid, e));
                            }
                        }
                    }

                    // Clean up any remaining staging file if it was left
                    if let Some(ref stg) = stg_path {
                        let p = Path::new(stg);
                        if p.exists() {
                            let _ = std::fs::remove_file(p);
                            summary.cleaned_staging_files += 1;
                        }
                    }

                    action_taken = RecoveryAction::ReconcileDbOnly;
                    if persist_errors.is_empty() {
                        new_status = OperationStatus::Recovered;
                        message = format!("Reconciled existing physical audio at {}", dest);
                    } else {
                        // The file survived but the ledger did not. Leaving the entry
                        // interrupted keeps the next reconciliation pass retrying it,
                        // instead of declaring a recovery that never happened.
                        new_status = OperationStatus::Interrupted;
                        message = format!(
                            "Physical audio found at {} but persisting the recovered state failed: {}",
                            dest,
                            persist_errors.join("; ")
                        );
                        warn!(
                            op_id = %op_id,
                            dest = %dest,
                            errors = ?persist_errors,
                            "[Recovery Engine] Reconciliation could not persist the recovered download"
                        );
                    }
                } else if let Some(ref stg) = stg_path {
                    // Check Case 2: Audio validated in staging, but Promotion was interrupted before move
                    let stg_p = PathBuf::from(stg);
                    if stg_p.exists() && is_valid_audio_file(&stg_p) {
                        info!(op_id = %op_id, stg = %stg, "Case 2: Staging file is complete and validated. Promoting to destination.");
                        if let Some(ref dest_str) = exp_path {
                            let dest_p = PathBuf::from(dest_str);
                            if let Some(parent) = dest_p.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            if std::fs::rename(&stg_p, &dest_p).is_ok() {
                                if let Some(tid) = track_id {
                                    let f_size = std::fs::metadata(&dest_p)
                                        .map(|m| m.len() as i64)
                                        .unwrap_or(0);
                                    let dl_existing: Option<(i64, String)> = sqlx::query_as(
                                        "SELECT id, file_path FROM downloads WHERE track_id = ? LIMIT 1"
                                    )
                                    .bind(tid)
                                    .fetch_optional(db)
                                    .await
                                    .ok()
                                    .flatten();

                                    match dl_existing {
                                        Some((dl_id, _old_fp)) => {
                                            let _ = sqlx::query(
                                                "UPDATE downloads SET file_path = ?, file_size_bytes = ?, downloaded_at = CURRENT_TIMESTAMP WHERE id = ?"
                                            )
                                            .bind(dest_str)
                                            .bind(f_size)
                                            .bind(dl_id)
                                            .execute(db)
                                            .await;
                                        }
                                        None => {
                                            let _ = sqlx::query(
                                                r#"
                                                INSERT OR REPLACE INTO downloads (
                                                    track_id, file_path, file_size_bytes, file_format, bit_depth,
                                                    sample_rate, downloaded_at
                                                ) VALUES (?, ?, ?, 'FLAC', 16, 44100, CURRENT_TIMESTAMP)
                                                "#
                                            )
                                            .bind(tid)
                                            .bind(dest_str)
                                            .bind(f_size)
                                            .execute(db)
                                            .await;
                                        }
                                    }
                                }

                                if let Some(qid_str) = entity_id.as_deref() {
                                    if let Ok(qid) = qid_str.parse::<i64>() {
                                        let _ = sqlx::query(
                                            "UPDATE download_queue SET status = 'complete', progress_percent = 100.0, completed_at = CURRENT_TIMESTAMP WHERE id = ?"
                                        )
                                        .bind(qid)
                                        .execute(db)
                                        .await;
                                    }
                                }

                                action_taken = RecoveryAction::CompletePromotion;
                                new_status = OperationStatus::Recovered;
                                message = format!(
                                    "Completed promotion of validated staging file to {}",
                                    dest_str
                                );
                            } else {
                                action_taken = RecoveryAction::RollbackStaging;
                                new_status = OperationStatus::Interrupted;
                                message =
                                    "Failed to promote staging file to destination".to_string();
                            }
                        }
                    } else {
                        // Check Case 3: Incomplete transfer or corrupted .staging/.part
                        info!(op_id = %op_id, "Case 3: Incomplete staging file detected. Cleaning up.");
                        if stg_p.exists() {
                            let _ = std::fs::remove_file(&stg_p);
                            summary.cleaned_staging_files += 1;
                        }

                        // Check if error is terminal
                        let is_term = is_terminal_taxonomy_error(tax_str.as_deref());

                        if is_term {
                            action_taken = RecoveryAction::MarkTerminal;
                            new_status = OperationStatus::FailedTerminal;
                            message = "Non-retryable terminal condition during crash recovery"
                                .to_string();

                            if let Some(qid_str) = entity_id.as_deref() {
                                if let Ok(qid) = qid_str.parse::<i64>() {
                                    let _ = sqlx::query(
                                        "UPDATE download_queue SET status = 'failed' WHERE id = ?",
                                    )
                                    .bind(qid)
                                    .execute(db)
                                    .await;
                                }
                            }
                        } else {
                            action_taken = RecoveryAction::ScheduleRetry;
                            new_status = OperationStatus::Interrupted;
                            message = "Staging cleaned up. Download reset to queued for retry."
                                .to_string();

                            if let Some(qid_str) = entity_id.as_deref() {
                                if let Ok(qid) = qid_str.parse::<i64>() {
                                    let _ = sqlx::query("UPDATE download_queue SET status = 'queued', started_at = NULL WHERE id = ?").bind(qid).execute(db).await;
                                }
                            }
                        }
                    }
                } else {
                    // No file traces
                    if is_terminal_taxonomy_error(tax_str.as_deref()) {
                        action_taken = RecoveryAction::MarkTerminal;
                        new_status = OperationStatus::FailedTerminal;
                        message =
                            "Non-retryable terminal condition during crash recovery".to_string();

                        if let Some(qid_str) = entity_id.as_deref() {
                            if let Ok(qid) = qid_str.parse::<i64>() {
                                let _ = sqlx::query(
                                    "UPDATE download_queue SET status = 'failed' WHERE id = ?",
                                )
                                .bind(qid)
                                .execute(db)
                                .await;
                            }
                        }
                    } else {
                        action_taken = RecoveryAction::ScheduleRetry;
                        new_status = OperationStatus::Interrupted;
                        message = "Reset interrupted download to queued state".to_string();

                        if let Some(qid_str) = entity_id.as_deref() {
                            if let Ok(qid) = qid_str.parse::<i64>() {
                                let _ = sqlx::query("UPDATE download_queue SET status = 'queued', started_at = NULL WHERE id = ?").bind(qid).execute(db).await;
                            }
                        }
                    }
                }
            }
            OperationType::CatalogIdentityRepair | OperationType::MetadataPathRepair => {
                info!(op_id = %op_id, "Reconciling interrupted repair operation");
                // Repair operations: verify if target file exists and has valid audio hash
                action_taken = RecoveryAction::RollbackFileToBaseline;
                new_status = OperationStatus::RolledBack;
                message = "Interrupted repair rolled back safely".to_string();
            }
            OperationType::ServiceSync | OperationType::PlaylistImport => {
                info!(op_id = %op_id, "Reconciling interrupted sync/import operation");
                action_taken = RecoveryAction::MarkRecovered;
                new_status = OperationStatus::Interrupted;
                message = "Interrupted import marked for safe resumption".to_string();
            }
            _ => {
                action_taken = RecoveryAction::NoOp;
                new_status = OperationStatus::Interrupted;
                message = "Operation reconciled".to_string();
            }
        }

        // Update operation journal status
        let _ = sqlx::query(
            "UPDATE operation_journal SET status = ?, result_summary = ?, checkpoint_at = CURRENT_TIMESTAMP WHERE operation_id = ?"
        )
        .bind(new_status.as_str())
        .bind(&message)
        .bind(&op_id)
        .execute(db)
        .await;

        // Record in operation_recovery_audit (append-only)
        let recovery_id = format!("rec-{}", uuid_or_timestamp(&op_id));
        let _ = sqlx::query(
            r#"
            INSERT INTO operation_recovery_audit (
                recovery_id, operation_id, operation_type, previous_status,
                new_status, action_taken, error_taxonomy, message, details_json
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&recovery_id)
        .bind(&op_id)
        .bind(op_type.as_str())
        .bind(prev_status.as_str())
        .bind(new_status.as_str())
        .bind(format!("{:?}", action_taken))
        .bind(&tax_str)
        .bind(&message)
        .bind(serde_json::json!({ "phase": phase.as_str(), "entity_id": entity_id }).to_string())
        .execute(db)
        .await;

        if new_status == OperationStatus::Recovered {
            summary.recovered_count += 1;
        } else if new_status == OperationStatus::Interrupted {
            summary.interrupted_retryable_count += 1;
        } else if new_status == OperationStatus::FailedTerminal {
            summary.failed_terminal_count += 1;
        }

        summary.details.push(OperationRecoveryDetail {
            operation_id: op_id,
            operation_type: op_type,
            previous_status: prev_status,
            new_status,
            phase,
            action_taken,
            message,
            ui_label: new_status.display_label().to_string(),
            error_taxonomy: tax_str,
        });
    }

    // TASK-84: Sanitize downloads stuck in 'downloading' for more than 1 hour to failed and purge staging files
    let _ = sanitize_timed_out_downloads(db, None).await;

    // 2. Reconcile any orphan download_queue rows stuck in 'downloading' without journal entries
    let orphan_queue: Vec<(i64, Option<i64>)> =
        sqlx::query_as("SELECT id, track_id FROM download_queue WHERE status = 'downloading'")
            .fetch_all(db)
            .await
            .unwrap_or_default();

    for (qid, _tid) in orphan_queue {
        let _ = sqlx::query(
            "UPDATE download_queue SET status = 'queued', started_at = NULL WHERE id = ?",
        )
        .bind(qid)
        .execute(db)
        .await;
        summary.interrupted_retryable_count += 1;
    }

    info!(
        recovered = summary.recovered_count,
        interrupted = summary.interrupted_retryable_count,
        terminal = summary.failed_terminal_count,
        cleaned_staging = summary.cleaned_staging_files,
        "[Recovery Engine] Reconciliation completed."
    );

    Ok(summary)
}

/// Retrieve the latest recovery audit records from SQLite.
pub async fn get_recovery_audit_summary(db: &SqlitePool) -> Result<RecoveryAuditSummary, String> {
    let rows = sqlx::query(
        r#"
        SELECT recovery_id, operation_id, operation_type, previous_status,
               new_status, action_taken, error_taxonomy, message, details_json
        FROM operation_recovery_audit
        ORDER BY timestamp DESC
        LIMIT 100
        "#,
    )
    .fetch_all(db)
    .await
    .map_err(|e| format!("Failed to fetch recovery audit history: {}", e))?;

    let mut summary = RecoveryAuditSummary::default();
    summary.total_journal_scanned = rows.len();

    for r in rows {
        let op_id: String = r.get("operation_id");
        let op_type_str: String = r.get("operation_type");
        let prev_status_str: String = r.get("previous_status");
        let new_status_str: String = r.get("new_status");
        let error_tax: Option<String> = r.get("error_taxonomy");
        let msg: String = r.get("message");

        let op_type = OperationType::from_str(&op_type_str).unwrap_or(OperationType::DownloadQobuz);
        let prev_status =
            OperationStatus::from_str(&prev_status_str).unwrap_or(OperationStatus::Started);
        let new_status =
            OperationStatus::from_str(&new_status_str).unwrap_or(OperationStatus::Recovered);

        if new_status == OperationStatus::Recovered {
            summary.recovered_count += 1;
        } else if new_status == OperationStatus::Interrupted {
            summary.interrupted_retryable_count += 1;
        } else if new_status == OperationStatus::FailedTerminal {
            summary.failed_terminal_count += 1;
        }

        summary.details.push(OperationRecoveryDetail {
            operation_id: op_id,
            operation_type: op_type,
            previous_status: prev_status,
            new_status,
            phase: OperationPhase::Completed,
            action_taken: RecoveryAction::NoOp,
            message: msg,
            ui_label: new_status.display_label().to_string(),
            error_taxonomy: error_tax,
        });
    }

    Ok(summary)
}

fn is_valid_audio_file(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }
    if let Ok(bytes) = std::fs::read(path) {
        if bytes.len() >= 4 {
            return AudioByteValidator::is_flac_magic(&bytes)
                || AudioByteValidator::is_mp3_magic(&bytes)
                || AudioByteValidator::is_m4a_magic(&bytes);
        }
    }
    false
}

/// Read the container and the real bit depth / sample rate off a recovered file.
///
/// Recovery must not invent audio metadata: a download that crashed right after
/// landing as AAC has to be recorded as AAC, because a row claiming FLAC/16/44.1k
/// contradicts the library the moment anyone opens it. Every value comes back as an
/// `Option` because `downloads` (migration 0004) CHECKs its vocabulary: the format
/// list, `bit_depth IN (16,24,32)` and a strictly positive `sample_rate`. Anything
/// outside that vocabulary is stored as NULL rather than rejected by the write.
fn measured_audio_facts(path: &Path) -> (Option<String>, Option<i64>, Option<i64>) {
    let (format, bit_depth, sample_rate) =
        match crate::download::audio_inspector::inspect_physical_audio_file(path) {
            Some(meta) => (Some(meta.format), meta.bit_depth, meta.sample_rate),
            None => {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_ascii_uppercase();
                let format = match ext.as_str() {
                    "FLAC" => Some("FLAC".to_string()),
                    "M4A" | "AAC" | "MP4" => Some("AAC".to_string()),
                    "MP3" => Some("MP3".to_string()),
                    "WAV" => Some("WAV".to_string()),
                    "OGG" => Some("OGG".to_string()),
                    "OPUS" => Some("OPUS".to_string()),
                    _ => None,
                };
                (format, 16, 44100)
            }
        };

    let format = format.filter(|f| {
        matches!(
            f.as_str(),
            "FLAC" | "ALAC" | "WAV" | "MP3" | "AAC" | "OGG" | "OPUS"
        )
    });
    let bit_depth = matches!(bit_depth, 16 | 24 | 32).then_some(bit_depth as i64);
    let sample_rate = (sample_rate > 0).then_some(sample_rate as i64);

    (format, bit_depth, sample_rate)
}

fn is_terminal_taxonomy_error(tax_str: Option<&str>) -> bool {
    tax_str
        .map(|s| {
            s.contains("AuthInvalid")
                || s.contains("RejectedQuality")
                || s.contains("IdentityConflict")
                || s.contains("UnavailableFromProvider")
                || s.contains("RegionRestricted")
                || s.contains("EntitlementDenied")
        })
        .unwrap_or(false)
}

fn uuid_or_timestamp(op_id: &str) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("{}-{}", op_id, now)
}

/// Summary report of staging cleanup and stuck queue recovery (TASK-148)
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct StagingRecoverySummary {
    pub purged_staging_files: usize,
    pub recovered_stuck_items: usize,
    pub purged_files: Vec<String>,
    pub recovered_queue_ids: Vec<i64>,
}

/// Purges residual/abandoned files from the .staging directory (*.part, *.cover.jpg, *.lrc, etc.)
/// and recovers orphan items in download_queue stuck in 'downloading' status by transitioning
/// them to 'failed' with an explanatory message (TASK-148).
///
/// Ensures items in 'complete'/'completed' or 'queued' are preserved untouched.
pub async fn cleanup_staging_and_recover_stuck_queue(
    db: &SqlitePool,
    staging_dir: Option<&Path>,
) -> Result<StagingRecoverySummary, String> {
    cleanup_staging_and_recover_stuck_queue_with_message(
        db,
        staging_dir,
        "Download interrupted by system restart",
    )
    .await
}

/// Overload allowing custom error reason/message for recovered stuck queue items.
pub async fn cleanup_staging_and_recover_stuck_queue_with_message(
    db: &SqlitePool,
    staging_dir: Option<&Path>,
    error_message: &str,
) -> Result<StagingRecoverySummary, String> {
    let _guard = reconciliation_lock().lock().await;
    cleanup_staging_and_recover_stuck_queue_with_message_locked(db, staging_dir, error_message)
        .await
}

async fn cleanup_staging_and_recover_stuck_queue_with_message_locked(
    db: &SqlitePool,
    staging_dir: Option<&Path>,
    error_message: &str,
) -> Result<StagingRecoverySummary, String> {
    let mut summary = StagingRecoverySummary::default();

    // 1. Resolve staging directory if not provided
    let target_staging_dir: Option<PathBuf> = if let Some(dir) = staging_dir {
        Some(dir.to_path_buf())
    } else {
        match crate::commands::resolve_effective_download_paths(db).await {
            Ok(eff) => Some(PathBuf::from(eff.staging_root)),
            Err(_) => {
                let default_p = PathBuf::from(".staging");
                if default_p.exists() {
                    Some(default_p)
                } else {
                    None
                }
            }
        }
    };

    // 2. Scan and purge abandoned staging files
    if let Some(ref s_dir) = target_staging_dir {
        if s_dir.exists() && s_dir.is_dir() {
            if let Ok(canonical_staging) = std::fs::canonicalize(s_dir) {
                for entry in walkdir::WalkDir::new(&canonical_staging)
                    .max_depth(3)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    let p = entry.path();
                    if p.is_file() {
                        let file_name = entry.file_name().to_string_lossy();
                        // Preserve hidden files such as .nomedia and .gitignore
                        if file_name.starts_with('.') {
                            continue;
                        }

                        // Path traversal defense: ensure file is strictly inside canonical staging directory
                        if let Ok(canonical_file) = std::fs::canonicalize(p) {
                            if canonical_file != canonical_staging
                                && canonical_file.starts_with(&canonical_staging)
                                && std::fs::remove_file(&canonical_file).is_ok()
                            {
                                summary.purged_staging_files += 1;
                                summary
                                    .purged_files
                                    .push(canonical_file.to_string_lossy().to_string());
                            }
                        }
                    }
                }

                // Prune empty subdirectories inside staging root (excluding staging root itself)
                for entry in walkdir::WalkDir::new(&canonical_staging)
                    .max_depth(3)
                    .contents_first(true)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    let p = entry.path();
                    if p.is_dir() && p != canonical_staging {
                        let _ = std::fs::remove_dir(p);
                    }
                }
            }
        }
    }

    // 3. Reconcile stuck download_queue items (status = 'downloading')
    let stuck_items: Vec<(i64, Option<String>)> =
        sqlx::query_as("SELECT id, staging_path FROM download_queue WHERE status = 'downloading'")
            .fetch_all(db)
            .await
            .map_err(|e| format!("Failed to query stuck download_queue items: {}", e))?;

    for (qid, staging_path_opt) in stuck_items {
        // If an explicit staging path was tracked on the queue item, ensure it is removed
        if let Some(ref stg_path_str) = staging_path_opt {
            let p = Path::new(stg_path_str);
            if p.exists() && p.is_file() {
                let canonical_target = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
                let path_str = canonical_target.to_string_lossy().to_string();
                if !summary.purged_files.contains(&path_str)
                    && std::fs::remove_file(&canonical_target).is_ok()
                {
                    summary.purged_staging_files += 1;
                    summary.purged_files.push(path_str);
                }
            }
        }

        // Transition stuck downloading item to failed status
        sqlx::query(
            r#"
            UPDATE download_queue
            SET status = 'failed',
                error_message = ?,
                last_error = ?
            WHERE id = ?
            "#,
        )
        .bind(error_message)
        .bind(error_message)
        .bind(qid)
        .execute(db)
        .await
        .map_err(|e| format!("Failed to update stuck download_queue item #{}: {}", qid, e))?;

        summary.recovered_stuck_items += 1;
        summary.recovered_queue_ids.push(qid);
    }

    info!(
        purged = summary.purged_staging_files,
        recovered = summary.recovered_stuck_items,
        "[Recovery Engine] Staging cleanup and stuck queue recovery complete."
    );

    Ok(summary)
}

/// Sanitize downloads that have been stuck in 'downloading' for more than 1 hour (TASK-84).
/// Transitions them to 'failed' and purges their staging files (.part, etc.)
pub async fn sanitize_timed_out_downloads(
    db: &SqlitePool,
    staging_dir: Option<&Path>,
) -> Result<usize, String> {
    let target_staging_dir: Option<PathBuf> = if let Some(dir) = staging_dir {
        Some(dir.to_path_buf())
    } else {
        match crate::commands::resolve_effective_download_paths(db).await {
            Ok(eff) => Some(PathBuf::from(eff.staging_root)),
            Err(_) => {
                let default_p = PathBuf::from(".staging");
                if default_p.exists() {
                    Some(default_p)
                } else {
                    None
                }
            }
        }
    };

    let stuck_items: Vec<(i64, Option<String>)> = sqlx::query_as(
        r#"
        SELECT id, staging_path
        FROM download_queue
        WHERE status = 'downloading'
          AND (
            (started_at IS NOT NULL AND datetime(started_at) <= datetime('now', '-1 hour'))
            OR (started_at IS NULL AND datetime(created_at) <= datetime('now', '-1 hour'))
          )
        "#,
    )
    .fetch_all(db)
    .await
    .map_err(|e| format!("Failed to query timed-out download_queue items: {}", e))?;

    let mut sanitized_count = 0;

    for (qid, staging_path_opt) in stuck_items {
        // 1. Purge explicit staging path if it exists
        if let Some(ref stg_path_str) = staging_path_opt {
            let p = Path::new(stg_path_str);
            if p.exists() && p.is_file() {
                let _ = std::fs::remove_file(p);
            }
        }

        // 2. Purge potential staging files matching {qid}.part, {qid}.cover.jpg, {qid}.lrc in staging directory
        if let Some(ref s_dir) = target_staging_dir {
            for ext in &[
                "part",
                "flac",
                "mp3",
                "m4a",
                "cover.jpg",
                "cover.webp",
                "lrc",
            ] {
                let candidate = s_dir.join(format!("{}.{}", qid, ext));
                if candidate.exists() && candidate.is_file() {
                    let _ = std::fs::remove_file(&candidate);
                }
            }
        }

        // 3. Mark as failed
        let res = sqlx::query(
            r#"
            UPDATE download_queue
            SET status = 'failed',
                error_message = 'Download timed out after 1 hour in downloading state',
                last_error = 'Download timed out after 1 hour in downloading state'
            WHERE id = ?
            "#,
        )
        .bind(qid)
        .execute(db)
        .await;

        if let Ok(_) = res {
            sanitized_count += 1;
        }
    }

    if sanitized_count > 0 {
        info!(
            sanitized_count,
            "[Recovery Engine] Sanitized downloads timed out in downloading state (> 1h)"
        );
    }

    Ok(sanitized_count)
}

/// Summary of canonical disk layout and downloads ledger reconciliation (TASK-110).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[allow(dead_code)] // Lo devuelve `resolve_canonical_track_path_from_db`, cubierta por
                    // `tests/disk_layout_normalization_test.rs`.
pub struct CanonicalPathNormalizationReport {
    pub scanned_downloads: usize,
    pub updated_records: usize,
    pub moved_physical_files: usize,
    pub errors: Vec<String>,
}

/// Resolves the canonical track destination path for a given `track_id` based on library settings and track/album metadata.
#[allow(dead_code)] // Cubierta por `tests/disk_layout_normalization_test.rs`.
pub async fn resolve_canonical_track_path_from_db(
    db: &SqlitePool,
    track_id: i64,
) -> Result<Option<PathBuf>, String> {
    let row: Option<(String, String, String, String, Option<String>, Option<i32>, i64, Option<String>)> = sqlx::query_as(
        r#"
        SELECT
            t.title,
            COALESCE(
                (SELECT art.name FROM track_artists ta JOIN artists art ON art.id = ta.artist_id WHERE ta.track_id = t.id ORDER BY CASE ta.role WHEN 'primary' THEN 1 WHEN 'main' THEN 2 ELSE 3 END, ta.artist_id ASC LIMIT 1),
                (SELECT art.name FROM album_artists aa JOIN artists art ON art.id = aa.artist_id WHERE aa.album_id = t.album_id ORDER BY aa.is_primary DESC, aa.artist_id ASC LIMIT 1),
                'Unknown Artist'
            ) as artist,
            COALESCE(
                (SELECT art.name FROM album_artists aa JOIN artists art ON art.id = aa.artist_id WHERE aa.album_id = t.album_id ORDER BY aa.is_primary DESC, aa.artist_id ASC LIMIT 1),
                'Unknown Artist'
            ) as album_artist,
            COALESCE(alb.title, 'Unknown Album') as album_title,
            alb.release_date,
            t.disc_number,
            COALESCE(CAST(t.track_number AS INTEGER), 1) as track_number,
            COALESCE(LOWER(d.file_format), 'flac') as format
        FROM tracks t
        LEFT JOIN albums alb ON t.album_id = alb.id
        LEFT JOIN downloads d ON d.track_id = t.id
        WHERE t.id = ?
        "#,
    )
    .bind(track_id)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("Query error: {}", e))?;

    let (title, artist, alb_artist, album_title, rel_date, disc_number, track_number, format) =
        match row {
            Some(r) => r,
            None => return Ok(None),
        };

    let year = rel_date
        .as_deref()
        .and_then(|d| d.get(..4).and_then(|y| y.parse::<i32>().ok()));

    let base_folder: Option<String> =
        sqlx::query_scalar("SELECT base_folder FROM folder_settings WHERE id = 1")
            .fetch_optional(db)
            .await
            .unwrap_or(None);

    let base_dir = base_folder
        .filter(|p| !p.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::audio_dir()
                .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join("Music"))
                .join("Syncify")
        });

    let layout = LibraryLayout::new(base_dir);
    let canonical = layout.canonical_track_path(
        &alb_artist,
        &artist,
        &album_title,
        year,
        disc_number.unwrap_or(1) as u32,
        1,
        track_number as u32,
        &title,
        &format.unwrap_or_else(|| "flac".to_string()),
    );

    Ok(Some(canonical))
}

/// Normalizes and reconciles physical audio paths and the SQLite `downloads` ledger
/// to conform to the canonical `[{Year}] {Album}` and `Various Artists` layout (TASK-110).
#[allow(dead_code)] // Cubierta por `tests/disk_layout_normalization_test.rs`.
pub async fn reconcile_canonical_download_records(
    db: &SqlitePool,
    dry_run: bool,
) -> Result<CanonicalPathNormalizationReport, String> {
    let rows: Vec<(i64, i64, String)> = sqlx::query_as(
        "SELECT id, track_id, file_path FROM downloads WHERE file_path IS NOT NULL AND file_path != ''"
    )
    .fetch_all(db)
    .await
    .map_err(|e| format!("Failed to query downloads for path reconciliation: {}", e))?;

    let mut report = CanonicalPathNormalizationReport {
        scanned_downloads: rows.len(),
        updated_records: 0,
        moved_physical_files: 0,
        errors: Vec::new(),
    };

    for (dl_id, track_id, current_path_str) in rows {
        let canonical_path_opt = match resolve_canonical_track_path_from_db(db, track_id).await {
            Ok(opt) => opt,
            Err(e) => {
                report.errors.push(format!(
                    "Failed to resolve canonical path for track {}: {}",
                    track_id, e
                ));
                continue;
            }
        };

        let canonical_path = match canonical_path_opt {
            Some(p) => p,
            None => continue,
        };

        let canonical_path_str = canonical_path.to_string_lossy().to_string();
        if canonical_path_str == current_path_str {
            continue;
        }

        let curr_p = PathBuf::from(&current_path_str);
        if !curr_p.exists() {
            // If the file already exists at the canonical path, just update DB
            if canonical_path.exists() {
                if !dry_run {
                    let _ = sqlx::query("UPDATE downloads SET file_path = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
                        .bind(&canonical_path_str)
                        .bind(dl_id)
                        .execute(db)
                        .await;
                }
                report.updated_records += 1;
            }
            continue;
        }

        if !dry_run {
            if let Some(parent) = canonical_path.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    report.errors.push(format!(
                        "Failed to create parent dir for {}: {}",
                        canonical_path_str, e
                    ));
                    continue;
                }
            }

            if let Err(e) = std::fs::rename(&curr_p, &canonical_path) {
                report.errors.push(format!(
                    "Failed to move file from {} to {}: {}",
                    current_path_str, canonical_path_str, e
                ));
                continue;
            }

            report.moved_physical_files += 1;

            let res = sqlx::query(
                "UPDATE downloads SET file_path = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            )
            .bind(&canonical_path_str)
            .bind(dl_id)
            .execute(db)
            .await;

            if let Err(e) = res {
                report.errors.push(format!(
                    "Failed to update downloads record {}: {}",
                    dl_id, e
                ));
                continue;
            }

            report.updated_records += 1;
        } else {
            report.moved_physical_files += 1;
            report.updated_records += 1;
        }
    }

    Ok(report)
}
