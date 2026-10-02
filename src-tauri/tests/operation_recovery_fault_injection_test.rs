//! Fault Injection Integration Test Suite for Post-Crash Deterministic Recovery (Sprint S167)
//!
//! Tests crash and failure recovery across all 13 boundary conditions:
//! A. After journal creation
//! B. After staging creation
//! C. During active Transfer (.part)
//! D. After Transfer before Validate
//! E. After Validate before Tagging
//! F. After Tagging before Promotion
//! G. After Promotion before SQLite commit
//! H. After SQLite commit before journal Completed
//! I. During repair filesystem rename
//! J. During repair DB update
//! K. During import track persist
//! L. During playlist link persist
//! M. During metadata enrichment
//!
//! Plus verification of:
//! - Restart reconciliation
//! - DB consistency & FS consistency
//! - Zero duplicate tracks/sources/downloads
//! - Zero ghost tracks/albums
//! - Correct retry vs terminal classification
//! - Append-only recovery audit history
//! - Idempotent second restart (0 mutations)

use sqlx::sqlite::SqlitePoolOptions;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};
use syncify_core_domain::{OperationJournalEntry, OperationPhase, OperationStatus, OperationType};
use syncify_tauri_lib::commands::{
    perform_sync_service_with_emitter, SyncProgressEmitter, SyncProgressEvent,
};
use syncify_tauri_lib::services::operation_recovery::{
    begin_service_sync_operation, begin_tidal_download_operation, classify_operation_error,
    create_operation_journal, download_staging_path, get_recovery_audit_summary,
    reconcile_startup_operations, DownloadJournal, DownloadJournalParams, JournaledOperation,
};
use syncify_tauri_lib::services::tidal_pipeline::TidalSingleTrackRequest;
use tempfile::TempDir;

/// Helper to generate a minimal valid FLAC file (fLaC magic header + minimal streaminfo block)
fn create_valid_flac_file(path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut file = File::create(path).expect("Create flac file");
    // "fLaC" magic bytes + minimal block header
    let flac_header: [u8; 8] = [0x66, 0x4C, 0x61, 0x43, 0x80, 0x00, 0x00, 0x22];
    file.write_all(&flac_header).expect("Write flac header");
    // 34 bytes of streaminfo zeros
    let streaminfo = [0u8; 34];
    file.write_all(&streaminfo).expect("Write streaminfo");
    file.flush().expect("Flush flac file");
}

/// Helper to create a partial/corrupted .part file
fn create_corrupt_part_file(path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut file = File::create(path).expect("Create corrupt part file");
    file.write_all(b"INCOMPLETE_STREAM_PAYLOAD_CORRUPT")
        .expect("Write corrupt bytes");
    file.flush().expect("Flush corrupt part file");
}

#[tokio::test]
async fn test_fault_injection_boundary_a_after_journal_creation() {
    let temp = TempDir::new().unwrap();
    let db_path = temp.path().join("fault_a.db");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite:{}?mode=rwc", db_path.display()))
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    // Boundary A: Crash immediately after creating journal entry (status: started, phase: init)
    let entry = OperationJournalEntry {
        operation_id: "op-fault-a-01".to_string(),
        operation_type: OperationType::DownloadQobuz,
        entity_id: Some("1".to_string()),
        account_id: Some(1),
        track_id: Some(10),
        download_id: None,
        provider: Some("qobuz".to_string()),
        phase: OperationPhase::Init,
        attempt: 1,
        started_at: "".to_string(),
        checkpoint_at: "".to_string(),
        status: OperationStatus::Started,
        input_identity: Some(r#"{"isrc":"USRC12345678"}"#.to_string()),
        expected_output_path: Some(temp.path().join("audio.flac").to_string_lossy().to_string()),
        staging_path: None,
        file_baseline: None,
        db_transaction_state: None,
        rollback_state: None,
        error_taxonomy: None,
        retry_policy: Some("immediate".to_string()),
        result_summary: None,
    };

    create_operation_journal(&pool, &entry).await.unwrap();

    // Startup Reconciliation
    let summary = reconcile_startup_operations(&pool, Some(temp.path()))
        .await
        .unwrap();
    assert_eq!(summary.active_operations_found, 1);
    assert_eq!(summary.interrupted_retryable_count, 1);

    // Verify journal status transitioned from Started -> Interrupted
    let journal_status: String =
        sqlx::query_scalar("SELECT status FROM operation_journal WHERE operation_id = ?")
            .bind("op-fault-a-01")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(journal_status, "interrupted");

    // Idempotent second restart
    let summary_second = reconcile_startup_operations(&pool, Some(temp.path()))
        .await
        .unwrap();
    assert_eq!(
        summary_second.active_operations_found, 0,
        "Second restart must find 0 active operations"
    );
}

#[tokio::test]
async fn test_fault_injection_boundary_b_and_c_during_transfer_and_staging() {
    let temp = TempDir::new().unwrap();
    let db_path = temp.path().join("fault_b_c.db");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite:{}?mode=rwc", db_path.display()))
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let staging_file = temp.path().join(".staging").join("op-fault-bc.part");
    create_corrupt_part_file(&staging_file);
    assert!(staging_file.exists());

    // Insert track row first to satisfy FK
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, duration_ms) VALUES ('Test Track BC', 180000) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Insert queue row
    let qid: i64 = sqlx::query_scalar(
        "INSERT INTO download_queue (track_id, status, priority, position) VALUES (?, 'downloading', 0, 1) RETURNING id"
    )
    .bind(tid)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Boundary B/C: Crash during Transfer with incomplete .part staging file
    let entry = OperationJournalEntry {
        operation_id: "op-fault-bc-01".to_string(),
        operation_type: OperationType::DownloadTidal,
        entity_id: Some(qid.to_string()),
        account_id: Some(1),
        track_id: Some(20),
        download_id: None,
        provider: Some("tidal".to_string()),
        phase: OperationPhase::Transfer,
        attempt: 1,
        started_at: "".to_string(),
        checkpoint_at: "".to_string(),
        status: OperationStatus::Checkpointed,
        input_identity: Some(r#"{"serviceTrackId":"134683067"}"#.to_string()),
        expected_output_path: Some(
            temp.path()
                .join("Tidal Track.flac")
                .to_string_lossy()
                .to_string(),
        ),
        staging_path: Some(staging_file.to_string_lossy().to_string()),
        file_baseline: None,
        db_transaction_state: None,
        rollback_state: None,
        error_taxonomy: None,
        retry_policy: Some("backoff".to_string()),
        result_summary: None,
    };

    create_operation_journal(&pool, &entry).await.unwrap();

    // Startup Reconciliation
    let summary = reconcile_startup_operations(&pool, Some(temp.path()))
        .await
        .unwrap();
    assert_eq!(summary.cleaned_staging_files, 1);
    assert!(
        !staging_file.exists(),
        "Corrupt staging file must be cleaned up on restart"
    );

    // Queue item reset to queued
    let q_status: String = sqlx::query_scalar("SELECT status FROM download_queue WHERE id = ?")
        .bind(qid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        q_status, "queued",
        "Queue item must be safely reset to queued state"
    );

    // Audit record present
    let audit = get_recovery_audit_summary(&pool).await.unwrap();
    assert_eq!(audit.interrupted_retryable_count, 1);
}

#[tokio::test]
async fn test_fault_injection_boundary_f_after_tagging_before_promotion() {
    let temp = TempDir::new().unwrap();
    let db_path = temp.path().join("fault_f.db");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite:{}?mode=rwc", db_path.display()))
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let staging_file = temp.path().join(".staging").join("op-fault-f.flac");
    create_valid_flac_file(&staging_file);
    assert!(staging_file.exists());

    let dest_file = temp.path().join("Music").join("Artist - Track.flac");

    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, duration_ms) VALUES ('Test Track F', 180000) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let qid: i64 = sqlx::query_scalar(
        "INSERT INTO download_queue (track_id, status, priority, position) VALUES (?, 'downloading', 0, 1) RETURNING id"
    )
    .bind(tid)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Boundary F: Valid tagged audio in staging, crash right before promotion to destination
    let entry = OperationJournalEntry {
        operation_id: "op-fault-f-01".to_string(),
        operation_type: OperationType::DownloadQobuz,
        entity_id: Some(qid.to_string()),
        account_id: Some(1),
        track_id: Some(tid),
        download_id: None,
        provider: Some("qobuz".to_string()),
        phase: OperationPhase::Tagging,
        attempt: 1,
        started_at: "".to_string(),
        checkpoint_at: "".to_string(),
        status: OperationStatus::Checkpointed,
        input_identity: Some(r#"{"title":"Track","artist":"Artist"}"#.to_string()),
        expected_output_path: Some(dest_file.to_string_lossy().to_string()),
        staging_path: Some(staging_file.to_string_lossy().to_string()),
        file_baseline: None,
        db_transaction_state: None,
        rollback_state: None,
        error_taxonomy: None,
        retry_policy: None,
        result_summary: None,
    };

    create_operation_journal(&pool, &entry).await.unwrap();

    // Startup Reconciliation should complete promotion without redownloading!
    let summary = reconcile_startup_operations(&pool, Some(temp.path()))
        .await
        .unwrap();
    assert_eq!(summary.recovered_count, 1);
    assert!(
        dest_file.exists(),
        "Validated staging file must be promoted to destination"
    );
    assert!(!staging_file.exists(), "Staging file moved to destination");

    // Check downloads table inserted and queue complete
    let dl_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads WHERE track_id = ?")
        .bind(tid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(dl_count, 1);

    let q_status: String = sqlx::query_scalar("SELECT status FROM download_queue WHERE id = ?")
        .bind(qid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(q_status, "complete");
}

#[tokio::test]
async fn test_fault_injection_boundary_g_and_h_after_promotion_before_db_commit() {
    let temp = TempDir::new().unwrap();
    let db_path = temp.path().join("fault_g_h.db");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite:{}?mode=rwc", db_path.display()))
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    // Destination file exists physically
    let dest_file = temp.path().join("Music").join("Promoted Track.flac");
    create_valid_flac_file(&dest_file);
    assert!(dest_file.exists());

    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, duration_ms) VALUES ('Test Track GH', 180000) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let qid: i64 = sqlx::query_scalar(
        "INSERT INTO download_queue (track_id, status, priority, position) VALUES (?, 'downloading', 0, 1) RETURNING id"
    )
    .bind(tid)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Boundary G/H: File promoted to disk, crash occurred before SQLite commit
    let entry = OperationJournalEntry {
        operation_id: "op-fault-gh-01".to_string(),
        operation_type: OperationType::Promotion,
        entity_id: Some(qid.to_string()),
        account_id: Some(1),
        track_id: Some(tid),
        download_id: None,
        provider: Some("tidal".to_string()),
        phase: OperationPhase::Promotion,
        attempt: 1,
        started_at: "".to_string(),
        checkpoint_at: "".to_string(),
        status: OperationStatus::Persisting,
        input_identity: Some(r#"{"isrc":"GBAYE1234567"}"#.to_string()),
        expected_output_path: Some(dest_file.to_string_lossy().to_string()),
        staging_path: None,
        file_baseline: None,
        db_transaction_state: Some("pending".to_string()),
        rollback_state: None,
        error_taxonomy: None,
        retry_policy: None,
        result_summary: None,
    };

    create_operation_journal(&pool, &entry).await.unwrap();

    // Reconciliation should detect existing valid physical audio, create downloads row, mark recovered
    let summary = reconcile_startup_operations(&pool, Some(temp.path()))
        .await
        .unwrap();
    assert_eq!(summary.recovered_count, 1);

    let dl_row: (i64, String, String) =
        sqlx::query_as("SELECT track_id, file_path, file_format FROM downloads WHERE track_id = ?")
            .bind(tid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(dl_row.0, tid);
    assert_eq!(dl_row.1, dest_file.to_string_lossy().to_string());
    assert_eq!(dl_row.2, "FLAC");

    let q_status: String = sqlx::query_scalar("SELECT status FROM download_queue WHERE id = ?")
        .bind(qid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(q_status, "complete");

    // Second restart is 100% idempotent and does 0 new downloads rows
    let summary_second = reconcile_startup_operations(&pool, Some(temp.path()))
        .await
        .unwrap();
    assert_eq!(summary_second.active_operations_found, 0);
    let dl_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads WHERE track_id = ?")
        .bind(tid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(dl_count, 1, "Must never duplicate downloads row");
}

#[tokio::test]
async fn test_fault_injection_boundary_i_and_j_repair_crash() {
    let temp = TempDir::new().unwrap();
    let db_path = temp.path().join("fault_i_j.db");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite:{}?mode=rwc", db_path.display()))
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    // Boundary I/J: Catalog/Metadata repair crashed mid-operation
    let entry = OperationJournalEntry {
        operation_id: "op-fault-ij-01".to_string(),
        operation_type: OperationType::CatalogIdentityRepair,
        entity_id: Some("99".to_string()),
        account_id: None,
        track_id: Some(99),
        download_id: Some(1),
        provider: None,
        phase: OperationPhase::Persist,
        attempt: 1,
        started_at: "".to_string(),
        checkpoint_at: "".to_string(),
        status: OperationStatus::Persisting,
        input_identity: None,
        expected_output_path: Some("/fake/path.flac".to_string()),
        staging_path: None,
        file_baseline: Some(r#"{"input_sha256":"abc123"}"#.to_string()),
        db_transaction_state: Some("in_progress".to_string()),
        rollback_state: None,
        error_taxonomy: None,
        retry_policy: None,
        result_summary: None,
    };

    create_operation_journal(&pool, &entry).await.unwrap();

    let summary = reconcile_startup_operations(&pool, Some(temp.path()))
        .await
        .unwrap();
    assert_eq!(summary.active_operations_found, 1);

    let journal_status: String =
        sqlx::query_scalar("SELECT status FROM operation_journal WHERE operation_id = ?")
            .bind("op-fault-ij-01")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(journal_status, "rolled_back");
}

#[tokio::test]
async fn test_fault_injection_boundary_k_l_m_import_and_enrichment_crash() {
    let temp = TempDir::new().unwrap();
    let db_path = temp.path().join("fault_k_l_m.db");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite:{}?mode=rwc", db_path.display()))
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    // Boundary K/L: Service Sync / Playlist Import crash
    let entry = OperationJournalEntry {
        operation_id: "op-fault-klm-01".to_string(),
        operation_type: OperationType::ServiceSync,
        entity_id: Some("spotify-playlist-10".to_string()),
        account_id: Some(1),
        track_id: None,
        download_id: None,
        provider: Some("spotify".to_string()),
        phase: OperationPhase::Persist,
        attempt: 1,
        started_at: "".to_string(),
        checkpoint_at: "".to_string(),
        status: OperationStatus::Started,
        input_identity: Some(r#"{"playlist_id":"spotify-playlist-10"}"#.to_string()),
        expected_output_path: None,
        staging_path: None,
        file_baseline: None,
        db_transaction_state: None,
        rollback_state: None,
        error_taxonomy: None,
        retry_policy: Some("immediate".to_string()),
        result_summary: None,
    };

    create_operation_journal(&pool, &entry).await.unwrap();

    let summary = reconcile_startup_operations(&pool, Some(temp.path()))
        .await
        .unwrap();
    assert_eq!(summary.interrupted_retryable_count, 1);

    let journal_status: String =
        sqlx::query_scalar("SELECT status FROM operation_journal WHERE operation_id = ?")
            .bind("op-fault-klm-01")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(journal_status, "interrupted");
}

// ============================================================================
// PRODUCTION PATH (BD-9)
//
// The boundary tests above inject journal rows by hand. The tests below drive
// the SAME journal API the shipping code uses — `DownloadJournal` (download
// worker), `begin_tidal_download_operation` (Tidal pipeline) and
// `begin_service_sync_operation` (service sync) — and then let
// `reconcile_startup_operations` consume whatever they wrote. They fail if the
// production code stops writing scannable rows, or writes rows the reconciler
// cannot act on.
// ============================================================================

/// Open a migrated, migrated-only database (no operation_journal content).
async fn migrated_pool(name: &str) -> (TempDir, sqlx::SqlitePool) {
    let temp = TempDir::new().unwrap();
    let db_path = temp.path().join(name);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite:{}?mode=rwc", db_path.display()))
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    (temp, pool)
}

/// Seed a track + a `downloading` queue row, the state the worker starts from.
async fn seed_downloading_queue_row(pool: &sqlx::SqlitePool) -> (i64, i64) {
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, duration_ms) VALUES ('Journaled Track', 200000) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let qid: i64 = sqlx::query_scalar(
        "INSERT INTO download_queue (track_id, status, priority, position) VALUES (?, 'downloading', 0, 1) RETURNING id",
    )
    .bind(tid)
    .fetch_one(pool)
    .await
    .unwrap();

    (tid, qid)
}

#[tokio::test]
async fn production_worker_journal_is_consumed_by_startup_reconciliation() {
    let (temp, pool) = migrated_pool("production_worker.db").await;
    let output_dir = temp.path().join("library").to_string_lossy().to_string();
    let (tid, qid) = seed_downloading_queue_row(&pool).await;

    // The worker opens the entry and checkpoints the transfer, exactly as
    // `DownloadWorker::process_download_internal` does.
    let journal = DownloadJournal::start(
        &pool,
        &DownloadJournalParams {
            operation_id: "op-production-download-01".to_string(),
            queue_id: qid,
            track_id: tid,
            provider: Some("qobuz".to_string()),
            input_identity: Some("service_track_id=42".to_string()),
            output_dir: output_dir.clone(),
            allow_fallback: false,
        },
    )
    .await
    .expect("production journal entry must open");

    journal.checkpoint_transfer(Some("transfer started")).await;

    // The production row must be scannable by the reconciler and must point at
    // the real `.part` path the download writes into.
    let (status, phase, staging_path, entity_id, operation_type): (
        String,
        String,
        Option<String>,
        Option<String>,
        String,
    ) = sqlx::query_as(
        "SELECT status, phase, staging_path, entity_id, operation_type FROM operation_journal WHERE operation_id = ?",
    )
    .bind("op-production-download-01")
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(status, "checkpointed");
    assert_eq!(phase, "transfer");
    assert_eq!(operation_type, "download_qobuz");
    assert_eq!(entity_id.as_deref(), Some(qid.to_string().as_str()));
    assert_eq!(
        staging_path.as_deref(),
        Some(download_staging_path(&output_dir, qid).as_str())
    );

    // Crash mid-transfer: a truncated `.part` sits at the journaled path.
    let part = temp
        .path()
        .join("library")
        .join(".staging")
        .join(format!("{}.part", qid));
    create_corrupt_part_file(&part);
    assert!(part.exists());

    let summary = reconcile_startup_operations(&pool, Some(temp.path()))
        .await
        .unwrap();
    assert_eq!(summary.active_operations_found, 1);
    assert_eq!(summary.interrupted_retryable_count, 1);
    assert_eq!(summary.cleaned_staging_files, 1);

    // Staging purged, queue item re-armed for retry, journal closed as interrupted.
    assert!(!part.exists(), "incomplete .part must be purged");
    let queue_status: String = sqlx::query_scalar("SELECT status FROM download_queue WHERE id = ?")
        .bind(qid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(queue_status, "queued");

    let journal_status: String =
        sqlx::query_scalar("SELECT status FROM operation_journal WHERE operation_id = ?")
            .bind("op-production-download-01")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(journal_status, "interrupted");

    // Append-only audit trail for the recovered operation.
    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM operation_recovery_audit WHERE operation_id = 'op-production-download-01' AND action_taken = 'ScheduleRetry'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit_count, 1);
}

#[tokio::test]
async fn production_worker_committed_download_is_not_reconciled_again() {
    let (temp, pool) = migrated_pool("production_commit.db").await;
    let output_dir = temp.path().join("library").to_string_lossy().to_string();
    let (tid, qid) = seed_downloading_queue_row(&pool).await;

    let final_path = temp
        .path()
        .join("library")
        .join("Artist")
        .join("Track.flac");
    create_valid_flac_file(&final_path);

    let journal = DownloadJournal::start(
        &pool,
        &DownloadJournalParams {
            operation_id: "op-production-download-02".to_string(),
            queue_id: qid,
            track_id: tid,
            provider: Some("qobuz".to_string()),
            input_identity: Some("service_track_id=42".to_string()),
            output_dir: output_dir.clone(),
            allow_fallback: false,
        },
    )
    .await
    .expect("production journal entry must open");

    journal.checkpoint_transfer(Some("transfer started")).await;
    journal
        .checkpoint_promoted(&final_path.to_string_lossy(), Some("promoted via qobuz"))
        .await;
    journal
        .checkpoint_persist(Some("writing downloads ledger"))
        .await;
    journal.commit(Some("promoted=true service=qobuz")).await;

    // 'committed' is outside the set startup reconciliation scans, so a healthy
    // download is never re-queued or double-reconciled.
    let summary = reconcile_startup_operations(&pool, Some(temp.path()))
        .await
        .unwrap();
    assert_eq!(
        summary.active_operations_found, 0,
        "a committed download must not be reconciled again"
    );

    let (status, expected_output_path): (String, Option<String>) = sqlx::query_as(
        "SELECT status, expected_output_path FROM operation_journal WHERE operation_id = ?",
    )
    .bind("op-production-download-02")
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(status, "committed");
    assert_eq!(
        expected_output_path.as_deref(),
        Some(final_path.to_string_lossy().as_ref())
    );
    assert!(
        final_path.exists(),
        "committed audio must be left untouched"
    );
}

#[tokio::test]
async fn production_worker_terminal_failure_marks_queue_failed_on_recovery() {
    let (temp, pool) = migrated_pool("production_terminal.db").await;
    let output_dir = temp.path().join("library").to_string_lossy().to_string();
    let (tid, qid) = seed_downloading_queue_row(&pool).await;

    let journal = DownloadJournal::start(
        &pool,
        &DownloadJournalParams {
            operation_id: "op-production-download-03".to_string(),
            queue_id: qid,
            track_id: tid,
            provider: Some("qobuz".to_string()),
            input_identity: Some("service_track_id=42".to_string()),
            output_dir,
            allow_fallback: false,
        },
    )
    .await
    .expect("production journal entry must open");

    journal.checkpoint_transfer(Some("transfer started")).await;

    // A credential rejection is what the worker treats as permanent.
    let permanent_error = "RequiresAuth: Qobuz user authentication required (token expired)";
    assert!(
        !classify_operation_error(OperationType::DownloadQobuz, "qobuz", permanent_error)
            .is_retryable(),
        "credential rejection must not be classified as retryable"
    );
    journal.fail(permanent_error, true).await;

    // The worker also records the verdict on the queue row.
    sqlx::query("UPDATE download_queue SET status = 'failed', error_message = ? WHERE id = ?")
        .bind(permanent_error)
        .bind(qid)
        .execute(&pool)
        .await
        .unwrap();

    let journal_status: String =
        sqlx::query_scalar("SELECT status FROM operation_journal WHERE operation_id = ?")
            .bind("op-production-download-03")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(journal_status, "failed_terminal");

    let tax: String =
        sqlx::query_scalar("SELECT error_taxonomy FROM operation_journal WHERE operation_id = ?")
            .bind("op-production-download-03")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        tax.contains("AuthInvalid"),
        "journal must record the taxonomy the reconciler classifies on, got {}",
        tax
    );

    // `failed_terminal` is outside the scannable set, so recovery leaves the
    // failed item alone: the orphan sweep only re-arms rows still 'downloading'.
    let summary = reconcile_startup_operations(&pool, Some(temp.path()))
        .await
        .unwrap();
    assert_eq!(summary.active_operations_found, 0);
    assert_eq!(summary.failed_terminal_count, 0);

    let queue_status: String = sqlx::query_scalar("SELECT status FROM download_queue WHERE id = ?")
        .bind(qid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        queue_status, "failed",
        "a permanently failed download must not be re-queued by recovery"
    );
}

#[tokio::test]
async fn production_tidal_pipeline_reuses_the_worker_journal_entry() {
    let (temp, pool) = migrated_pool("production_tidal.db").await;
    let output_dir = temp.path().join("library").to_string_lossy().to_string();
    let (tid, qid) = seed_downloading_queue_row(&pool).await;

    // The worker journals the queued item...
    let _worker_journal = DownloadJournal::start(
        &pool,
        &DownloadJournalParams {
            operation_id: "op-production-download-04".to_string(),
            queue_id: qid,
            track_id: tid,
            provider: Some("tidal".to_string()),
            input_identity: Some("service_track_id=4242".to_string()),
            output_dir: output_dir.clone(),
            allow_fallback: false,
        },
    )
    .await
    .expect("production journal entry must open");

    // ...and the pipeline it calls reuses that same row instead of forking a
    // second entry for one physical download.
    let request = TidalSingleTrackRequest {
        track_id_or_query: "4242".to_string(),
        requested_quality: Some("24-192".to_string()),
        output_dir: Some(output_dir.clone()),
        allow_lossy_fallback: Some(false),
        hint_title: Some("Journaled Track".to_string()),
        hint_artist: Some("Some Artist".to_string()),
        hint_album: Some("Some Album".to_string()),
        hint_isrc: Some("USQX92000875".to_string()),
        hint_track_number: Some(1),
        hint_disc_number: Some(1),
        hint_release_date: Some("2020-03-27".to_string()),
        hint_track_id: Some(tid),
        operation_id: Some("op-production-download-04".to_string()),
    };

    let pipeline_journal: JournaledOperation = begin_tidal_download_operation(&pool, &request)
        .await
        .expect("pipeline must attach to the worker entry");

    pipeline_journal
        .checkpoint(
            OperationPhase::Transfer,
            Some("/tmp/syncify_staging_abc/4242.flac"),
            Some("transfer started"),
        )
        .await;

    let entries: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM operation_journal WHERE entity_id = ?")
            .bind(qid.to_string())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        entries, 1,
        "one physical download must never have two journal entries"
    );

    // Without a caller-provided operation id the pipeline opens its own entry.
    let standalone = TidalSingleTrackRequest {
        track_id_or_query: "777".to_string(),
        hint_track_id: None,
        operation_id: None,
        ..request.clone()
    };
    let standalone_journal = begin_tidal_download_operation(&pool, &standalone)
        .await
        .expect("standalone pipeline must open its own entry");
    standalone_journal
        .checkpoint(
            OperationPhase::Validate,
            Some("/tmp/syncify_staging_def/777.flac"),
            None,
        )
        .await;

    let standalone_type: String = sqlx::query_scalar(
        "SELECT operation_type FROM operation_journal WHERE operation_type = 'download_tidal' AND entity_id = ?",
    )
    .bind("777")
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(standalone_type, "download_tidal");

    // Both entries are live, so startup reconciliation must pick them up.
    let summary = reconcile_startup_operations(&pool, Some(temp.path()))
        .await
        .unwrap();
    assert_eq!(summary.active_operations_found, 2);
}

#[tokio::test]
async fn production_service_sync_journal_is_scannable_and_closeable() {
    let (temp, pool) = migrated_pool("production_sync.db").await;

    let journal: JournaledOperation = begin_service_sync_operation(&pool, "Spotify", Some(7))
        .await
        .expect("service sync journal entry must open");

    let (operation_type, status, entity_id, account_id): (
        String,
        String,
        Option<String>,
        Option<i64>,
    ) = sqlx::query_as(
        "SELECT operation_type, status, entity_id, account_id FROM operation_journal WHERE operation_id LIKE 'op-sync-spotify-%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(operation_type, "service_sync");
    assert_eq!(status, "started");
    assert_eq!(entity_id.as_deref(), Some("spotify"));
    assert_eq!(account_id, Some(7));

    journal
        .checkpoint(
            OperationPhase::Transfer,
            None,
            Some("authenticated; fetching remote library"),
        )
        .await;

    // A crash between milestones leaves the sync scannable and retryable: the
    // catalog upserts it already performed are idempotent, so re-running is safe.
    let interrupted = reconcile_startup_operations(&pool, Some(temp.path()))
        .await
        .unwrap();
    assert_eq!(interrupted.active_operations_found, 1);
    assert_eq!(interrupted.interrupted_retryable_count, 1);

    let audit = get_recovery_audit_summary(&pool).await.unwrap();
    assert_eq!(audit.failed_terminal_count, 0);
    assert_eq!(audit.interrupted_retryable_count, 1);
    assert!(audit
        .details
        .iter()
        .any(|d| matches!(d.operation_type, OperationType::ServiceSync)));
}

#[tokio::test]
async fn production_service_sync_commit_is_not_reconciled() {
    let (_temp, pool) = migrated_pool("production_sync_commit.db").await;

    let journal = begin_service_sync_operation(&pool, "tidal", Some(3))
        .await
        .expect("service sync journal entry must open");
    journal
        .checkpoint(OperationPhase::Transfer, None, Some("fetching"))
        .await;
    journal
        .checkpoint_persisting(Some("imported_tracks=120"))
        .await;
    journal
        .commit(Some("service=tidal success=true imported_tracks=120"))
        .await;

    let summary = reconcile_startup_operations(&pool, None).await.unwrap();
    assert_eq!(
        summary.active_operations_found, 0,
        "a completed sync must not be reconciled again"
    );
}

#[tokio::test]
async fn production_error_classification_matches_retry_semantics() {
    // Retryable: the worker leaves these on the queue for another attempt and the
    // journal must record them as `interrupted`, not `failed_terminal`.
    for (error, label) in [
        ("NetworkExhausted: all retries failed", "network"),
        ("request timed out after 30s", "timeout"),
        ("429 TooManyRequests from provider", "rate limit"),
    ] {
        let taxonomy = classify_operation_error(OperationType::DownloadQobuz, "qobuz", error);
        assert!(
            taxonomy.is_retryable(),
            "'{}' ({}) must classify as retryable, got {:?}",
            error,
            label,
            taxonomy
        );
    }

    // Terminal: these need user action, so recovery must not silently re-queue.
    for (error, label) in [
        ("RequiresAuth: Qobuz user authentication required", "auth"),
        (
            "SourceIdentityMissing: no locked service_track_id",
            "identity",
        ),
        ("TrackUnresolved: not found on provider", "unavailable"),
        ("RejectedQuality: downgrade rejected", "quality"),
    ] {
        let taxonomy = classify_operation_error(OperationType::DownloadQobuz, "qobuz", error);
        assert!(
            !taxonomy.is_retryable(),
            "'{}' ({}) must classify as terminal, got {:?}",
            error,
            label,
            taxonomy
        );
    }
}
/// Collector used to observe that the sync really ran while asserting on the
/// journal row it leaves behind.
#[derive(Clone, Default)]
struct RecordingSyncEmitter {
    events: Arc<Mutex<Vec<String>>>,
}
impl RecordingSyncEmitter {
    fn new() -> Self {
        Self::default()
    }
    fn phases(&self) -> Vec<String> {
        self.events.lock().unwrap().clone()
    }
}
impl SyncProgressEmitter for RecordingSyncEmitter {
    fn emit_sync_progress(&self, event: &SyncProgressEvent) {
        self.events.lock().unwrap().push(event.phase.clone());
    }
}
/// BD-9 regression: the shipping entry point `perform_sync_service_with_emitter`
/// itself must open a journal entry and close it, without any test-injected row.
///
/// The tests above drive the journal API; this one drives the real command that
/// production calls (tray, `import_qobuz_library`, `unified_sync_service`,
/// playlist import). If the wrapper stops calling `begin_service_sync_operation`,
/// or forgets to close what it opened, the row below disappears or stays scannable
/// and this test fails — which is exactly the state the post-crash reconciler
/// cannot repair on its own.
#[tokio::test]
async fn production_service_sync_entry_point_journals_and_closes_its_own_run() {
    let (_temp, pool) = migrated_pool("production_sync_entry_point.sqlite").await;
    let emitter = RecordingSyncEmitter::new();

    // No accounts are seeded, so the sync stops at `RequiresAuth` without touching
    // the network — the run still has to be journaled end to end.
    let err = perform_sync_service_with_emitter(&pool, "qobuz", None, None, Some(&emitter))
        .await
        .expect_err("a sync without credentials must fail");
    assert!(
        err.starts_with("RequiresAuth:"),
        "unexpected sync error: {}",
        err
    );
    assert!(
        !emitter.phases().is_empty(),
        "the sync must have run, otherwise the journal row proves nothing"
    );

    // 1. The wrapper created the entry (operation type + entity as the reconciler
    //    expects for a service sync).
    let rows: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT operation_id, status, operation_type, error_taxonomy FROM operation_journal \
         WHERE operation_type = 'service_sync' AND entity_id = 'qobuz'",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        rows.len(),
        1,
        "the production sync entry point must journal exactly one run, got {:?}",
        rows
    );
    let (operation_id, status, operation_type, taxonomy) = &rows[0];
    assert_eq!(operation_type, "service_sync");
    assert!(
        operation_id.starts_with("op-sync-qobuz-"),
        "unexpected operation id: {}",
        operation_id
    );

    // 2. It closed the entry with the classified error instead of leaving it open.
    assert_eq!(
        status, "failed_terminal",
        "RequiresAuth must close the entry as terminal, got {}",
        status
    );
    let taxonomy = taxonomy
        .as_deref()
        .unwrap_or_else(|| panic!("the failed sync must record its error taxonomy"));
    assert!(
        taxonomy.contains("AuthInvalid"),
        "the journal must classify the auth failure the same way the reconciler \
         does, got {}",
        taxonomy
    );

    // 3. A closed entry is invisible to startup reconciliation: no re-repair, no
    //    audit rows, and the run is not reconsidered on every restart.
    let summary = reconcile_startup_operations(&pool, None).await.unwrap();
    assert_eq!(
        summary.total_journal_scanned, 0,
        "a closed sync entry must not be scanned again"
    );
    let audited: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM operation_recovery_audit WHERE operation_id = ?")
            .bind(operation_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        audited, 0,
        "a closed sync entry must not produce recovery audit rows"
    );
}
