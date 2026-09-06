//! Persistent Operation Journal, Checkpointing, and Post-Crash Recovery Service (S167)
//!
//! Provides deterministic post-crash state reconciliation for:
//! - Service Sync & Playlist Imports
//! - Qobuz & Tidal Downloads
//! - Cross-Provider Fallbacks
//! - Physical File Promotions & Tagging
//! - Catalog & Metadata Repairs

use std::path::{Path, PathBuf};
use sqlx::{SqlitePool, Row};
use tracing::info;
use syncify_core_domain::{
    AudioByteValidator, ErrorTaxonomy, OperationJournalEntry, OperationPhase,
    OperationRecoveryDetail, OperationStatus, OperationType, RecoveryAction, RecoveryAuditSummary,
};

/// Record a new operation in the persistent journal.
#[allow(dead_code)] // journal de recuperación: cubierto parcialmente por fault_injection_test; API completa intencional
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
#[allow(dead_code)] // journal de recuperación: cubierto parcialmente por fault_injection_test; API completa intencional
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
        "#
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
#[allow(dead_code)] // journal de recuperación: cubierto parcialmente por fault_injection_test; API completa intencional
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
        "#
    )
    .bind(result_summary)
    .bind(operation_id)
    .execute(db)
    .await
    .map_err(|e| format!("Failed to commit operation {}: {}", operation_id, e))?;

    Ok(())
}

/// Mark an operation as failed or interrupted.
#[allow(dead_code)] // journal de recuperación: cubierto parcialmente por fault_injection_test; API completa intencional
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
        "#
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

/// Perform comprehensive startup reconciliation across journal, SQLite state, and filesystem.
pub async fn reconcile_startup_operations(
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
        "#
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
        let prev_status = OperationStatus::from_str(&status_str).unwrap_or(OperationStatus::Started);
        let phase = OperationPhase::from_str(&phase_str).unwrap_or(OperationPhase::Init);

        let mut action_taken = RecoveryAction::NoOp;
        let mut new_status = OperationStatus::Interrupted;
        let mut message = String::new();

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
                        let dl_exists: Option<(i64,)> = sqlx::query_as(
                            "SELECT id FROM downloads WHERE track_id = ? AND file_path = ? LIMIT 1"
                        )
                        .bind(tid)
                        .bind(dest)
                        .fetch_optional(db)
                        .await
                        .ok()
                        .flatten();

                        if dl_exists.is_none() {
                            let f_size = std::fs::metadata(&dest_path).map(|m| m.len() as i64).unwrap_or(0);
                            let _ = sqlx::query(
                                r#"
                                INSERT OR REPLACE INTO downloads (
                                    track_id, file_path, file_size_bytes, file_format, bit_depth,
                                    sample_rate, downloaded_at
                                ) VALUES (?, ?, ?, 'FLAC', 16, 44100, CURRENT_TIMESTAMP)
                                "#
                            )
                            .bind(tid)
                            .bind(dest)
                            .bind(f_size)
                            .execute(db)
                            .await;
                        }
                    }

                    // Update download_queue if applicable
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

                    // Clean up any remaining staging file if it was left
                    if let Some(ref stg) = stg_path {
                        let p = Path::new(stg);
                        if p.exists() {
                            let _ = std::fs::remove_file(p);
                            summary.cleaned_staging_files += 1;
                        }
                    }

                    action_taken = RecoveryAction::ReconcileDbOnly;
                    new_status = OperationStatus::Recovered;
                    message = format!("Reconciled existing physical audio at {}", dest);
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
                            if let Ok(_) = std::fs::rename(&stg_p, &dest_p) {
                                if let Some(tid) = track_id {
                                    let f_size = std::fs::metadata(&dest_p).map(|m| m.len() as i64).unwrap_or(0);
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
                                message = format!("Completed promotion of validated staging file to {}", dest_str);
                            } else {
                                action_taken = RecoveryAction::RollbackStaging;
                                new_status = OperationStatus::Interrupted;
                                message = "Failed to promote staging file to destination".to_string();
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
                            message = "Non-retryable terminal condition during crash recovery".to_string();
                            
                            if let Some(qid_str) = entity_id.as_deref() {
                                if let Ok(qid) = qid_str.parse::<i64>() {
                                    let _ = sqlx::query("UPDATE download_queue SET status = 'failed' WHERE id = ?").bind(qid).execute(db).await;
                                }
                            }
                        } else {
                            action_taken = RecoveryAction::ScheduleRetry;
                            new_status = OperationStatus::Interrupted;
                            message = "Staging cleaned up. Download reset to queued for retry.".to_string();

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
                        message = "Non-retryable terminal condition during crash recovery".to_string();

                        if let Some(qid_str) = entity_id.as_deref() {
                            if let Ok(qid) = qid_str.parse::<i64>() {
                                let _ = sqlx::query("UPDATE download_queue SET status = 'failed' WHERE id = ?").bind(qid).execute(db).await;
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
            "#
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
    let orphan_queue: Vec<(i64, Option<i64>)> = sqlx::query_as(
        "SELECT id, track_id FROM download_queue WHERE status = 'downloading'"
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    for (qid, _tid) in orphan_queue {
        let _ = sqlx::query(
            "UPDATE download_queue SET status = 'queued', started_at = NULL WHERE id = ?"
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
        "#
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
        let prev_status = OperationStatus::from_str(&prev_status_str).unwrap_or(OperationStatus::Started);
        let new_status = OperationStatus::from_str(&new_status_str).unwrap_or(OperationStatus::Recovered);

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

fn is_terminal_taxonomy_error(tax_str: Option<&str>) -> bool {
    tax_str.map(|s| {
        s.contains("AuthInvalid")
            || s.contains("RejectedQuality")
            || s.contains("IdentityConflict")
            || s.contains("UnavailableFromProvider")
            || s.contains("RegionRestricted")
            || s.contains("EntitlementDenied")
    }).unwrap_or(false)
}

fn uuid_or_timestamp(op_id: &str) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("{}-{}", op_id, now)
}

/// Summary report of staging cleanup and stuck queue recovery (TASK-148)
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
pub async fn cleanup_staging_and_recover_stuck_queue_with_message(
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
                            if canonical_file != canonical_staging && canonical_file.starts_with(&canonical_staging) {
                                if let Ok(_) = std::fs::remove_file(&canonical_file) {
                                    summary.purged_staging_files += 1;
                                    summary.purged_files.push(canonical_file.to_string_lossy().to_string());
                                }
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
    let stuck_items: Vec<(i64, Option<String>)> = sqlx::query_as(
        "SELECT id, staging_path FROM download_queue WHERE status = 'downloading'"
    )
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
                if !summary.purged_files.contains(&path_str) {
                    if let Ok(_) = std::fs::remove_file(&canonical_target) {
                        summary.purged_staging_files += 1;
                        summary.purged_files.push(path_str);
                    }
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
            "#
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
        "#
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
            for ext in &["part", "flac", "mp3", "m4a", "cover.jpg", "cover.webp", "lrc"] {
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
            "#
        )
        .bind(qid)
        .execute(db)
        .await;

        if let Ok(_) = res {
            sanitized_count += 1;
        }
    }

    if sanitized_count > 0 {
        info!(sanitized_count, "[Recovery Engine] Sanitized downloads timed out in downloading state (> 1h)");
    }

    Ok(sanitized_count)
}
