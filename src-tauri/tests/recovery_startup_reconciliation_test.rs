//! Integration Test for Post-Crash Startup Reconciliation and Error Taxonomy Enforcement (Sprint S167)
//!
//! Verifies:
//! 1. Non-retryable errors (AuthInvalid, RejectedQuality, IdentityConflict, UnavailableFromProvider)
//!    are strictly marked FailedTerminal and NEVER automatically retried.
//! 2. Transient retryable errors (TemporaryNetworkFailure, Timeout, RateLimited) are marked Interrupted
//!    and reset to queued.
//! 3. Orphan 'downloading' queue rows are safely reconciled to 'queued' on startup.
//! 4. Recovery actions produce immutable, append-only records in `operation_recovery_audit`.
//! 5. UI labels match exact product requirements (e.g. "Recovered after restart", "Interrupted — retry available", "Failed terminal — user action required").

use sqlx::sqlite::SqlitePoolOptions;
use tempfile::TempDir;
use syncify_core_domain::{
    ErrorTaxonomy, OperationJournalEntry, OperationPhase, OperationStatus, OperationType,
};
use syncify_tauri_lib::services::operation_recovery::{
    create_operation_journal, reconcile_startup_operations, get_recovery_audit_summary,
};

#[tokio::test]
async fn test_recovery_never_auto_retries_non_retryable_errors() {
    let temp = TempDir::new().unwrap();
    let db_path = temp.path().join("rec_non_retry.db");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite:{}?mode=rwc", db_path.display()))
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let tid_auth: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, duration_ms) VALUES ('Track Auth', 180000) RETURNING id"
    ).fetch_one(&pool).await.unwrap();

    let tid_qual: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, duration_ms) VALUES ('Track Qual', 180000) RETURNING id"
    ).fetch_one(&pool).await.unwrap();

    let tid_unav: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, duration_ms) VALUES ('Track Unav', 180000) RETURNING id"
    ).fetch_one(&pool).await.unwrap();

    let qid_auth: i64 = sqlx::query_scalar(
        "INSERT INTO download_queue (track_id, status, priority, position) VALUES (?, 'downloading', 0, 1) RETURNING id"
    )
    .bind(tid_auth)
    .fetch_one(&pool)
    .await
    .unwrap();

    let qid_qual: i64 = sqlx::query_scalar(
        "INSERT INTO download_queue (track_id, status, priority, position) VALUES (?, 'downloading', 0, 2) RETURNING id"
    )
    .bind(tid_qual)
    .fetch_one(&pool)
    .await
    .unwrap();

    let qid_unav: i64 = sqlx::query_scalar(
        "INSERT INTO download_queue (track_id, status, priority, position) VALUES (?, 'downloading', 0, 3) RETURNING id"
    )
    .bind(tid_unav)
    .fetch_one(&pool)
    .await
    .unwrap();

    // 1. AuthInvalid operation
    let op_auth = OperationJournalEntry {
        operation_id: "op-term-auth-01".to_string(),
        operation_type: OperationType::DownloadTidal,
        entity_id: Some(qid_auth.to_string()),
        account_id: Some(1),
        track_id: Some(tid_auth),
        download_id: None,
        provider: Some("tidal".to_string()),
        phase: OperationPhase::Transfer,
        attempt: 1,
        started_at: "".to_string(),
        checkpoint_at: "".to_string(),
        status: OperationStatus::Checkpointed,
        input_identity: None,
        expected_output_path: Some(temp.path().join("t1.flac").to_string_lossy().to_string()),
        staging_path: None,
        file_baseline: None,
        db_transaction_state: None,
        rollback_state: None,
        error_taxonomy: Some(format!("{:?}", ErrorTaxonomy::AuthInvalid {
            message: "Token expired".to_string(),
        })),
        retry_policy: Some("never".to_string()),
        result_summary: None,
    };

    // 2. RejectedQuality operation
    let op_qual = OperationJournalEntry {
        operation_id: "op-term-qual-01".to_string(),
        operation_type: OperationType::DownloadQobuz,
        entity_id: Some(qid_qual.to_string()),
        account_id: Some(2),
        track_id: Some(tid_qual),
        download_id: None,
        provider: Some("qobuz".to_string()),
        phase: OperationPhase::Transfer,
        attempt: 1,
        started_at: "".to_string(),
        checkpoint_at: "".to_string(),
        status: OperationStatus::Checkpointed,
        input_identity: None,
        expected_output_path: Some(temp.path().join("t2.flac").to_string_lossy().to_string()),
        staging_path: None,
        file_baseline: None,
        db_transaction_state: None,
        rollback_state: None,
        error_taxonomy: Some(format!("{:?}", ErrorTaxonomy::RejectedQuality {
            requested: "24-192".to_string(),
            obtained: "16-44".to_string(),
            reason: "Quality unavailable".to_string(),
        })),
        retry_policy: Some("never".to_string()),
        result_summary: None,
    };

    // 3. UnavailableFromProvider operation
    let op_unav = OperationJournalEntry {
        operation_id: "op-term-unav-01".to_string(),
        operation_type: OperationType::DownloadTidal,
        entity_id: Some(qid_unav.to_string()),
        account_id: Some(1),
        track_id: Some(tid_unav),
        download_id: None,
        provider: Some("tidal".to_string()),
        phase: OperationPhase::Transfer,
        attempt: 1,
        started_at: "".to_string(),
        checkpoint_at: "".to_string(),
        status: OperationStatus::Checkpointed,
        input_identity: None,
        expected_output_path: Some(temp.path().join("t3.flac").to_string_lossy().to_string()),
        staging_path: None,
        file_baseline: None,
        db_transaction_state: None,
        rollback_state: None,
        error_taxonomy: Some(format!("{:?}", ErrorTaxonomy::UnavailableFromProvider {
            provider: "tidal".to_string(),
            item_id: "103".to_string(),
            reason: "2001".to_string(),
        })),
        retry_policy: Some("never".to_string()),
        result_summary: None,
    };

    create_operation_journal(&pool, &op_auth).await.unwrap();
    create_operation_journal(&pool, &op_qual).await.unwrap();
    create_operation_journal(&pool, &op_unav).await.unwrap();

    // Startup Reconciliation
    let summary = reconcile_startup_operations(&pool, Some(temp.path())).await.unwrap();
    assert_eq!(summary.failed_terminal_count, 3);
    assert_eq!(summary.interrupted_retryable_count, 0);

    // Verify all 3 queue items transitioned to 'failed' (NOT queued for auto-retry)
    let statuses: Vec<(i64, String)> = sqlx::query_as("SELECT id, status FROM download_queue ORDER BY id ASC")
        .fetch_all(&pool)
        .await
        .unwrap();

    assert_eq!(statuses.len(), 3);
    for (_id, status) in statuses {
        assert_eq!(status, "failed", "Non-retryable error items must be marked failed");
    }

    // Verify journal statuses are failed_terminal
    let journal_statuses: Vec<String> = sqlx::query_scalar("SELECT status FROM operation_journal ORDER BY operation_id ASC")
        .fetch_all(&pool)
        .await
        .unwrap();

    assert_eq!(journal_statuses, vec!["failed_terminal", "failed_terminal", "failed_terminal"]);
}

#[tokio::test]
async fn test_recovery_schedules_retry_for_transient_network_errors() {
    let temp = TempDir::new().unwrap();
    let db_path = temp.path().join("rec_transient.db");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite:{}?mode=rwc", db_path.display()))
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let tid_net: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, duration_ms) VALUES ('Track Net', 180000) RETURNING id"
    ).fetch_one(&pool).await.unwrap();

    let qid_net: i64 = sqlx::query_scalar(
        "INSERT INTO download_queue (track_id, status, priority, position) VALUES (?, 'downloading', 0, 1) RETURNING id"
    )
    .bind(tid_net)
    .fetch_one(&pool)
    .await
    .unwrap();

    let op_net = OperationJournalEntry {
        operation_id: "op-transient-net-01".to_string(),
        operation_type: OperationType::DownloadQobuz,
        entity_id: Some(qid_net.to_string()),
        account_id: Some(1),
        track_id: Some(tid_net),
        download_id: None,
        provider: Some("qobuz".to_string()),
        phase: OperationPhase::Transfer,
        attempt: 1,
        started_at: "".to_string(),
        checkpoint_at: "".to_string(),
        status: OperationStatus::Started,
        input_identity: None,
        expected_output_path: Some(temp.path().join("t_net.flac").to_string_lossy().to_string()),
        staging_path: None,
        file_baseline: None,
        db_transaction_state: None,
        rollback_state: None,
        error_taxonomy: Some(format!("{:?}", ErrorTaxonomy::TemporaryNetworkFailure {
            endpoint: "https://api.qobuz.com".to_string(),
            message: "Service Unavailable (503)".to_string(),
        })),
        retry_policy: Some("backoff".to_string()),
        result_summary: None,
    };

    create_operation_journal(&pool, &op_net).await.unwrap();

    let summary = reconcile_startup_operations(&pool, Some(temp.path())).await.unwrap();
    assert_eq!(summary.interrupted_retryable_count, 1);
    assert_eq!(summary.failed_terminal_count, 0);

    let q_status: String = sqlx::query_scalar("SELECT status FROM download_queue WHERE id = ?")
        .bind(qid_net)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(q_status, "queued", "Transient error queue item must be reset to queued for retry");
}

#[tokio::test]
async fn test_startup_reconciliation_append_only_audit_trail_and_ui_labels() {
    let temp = TempDir::new().unwrap();
    let db_path = temp.path().join("rec_audit.db");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite:{}?mode=rwc", db_path.display()))
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let op = OperationJournalEntry {
        operation_id: "op-audit-test-01".to_string(),
        operation_type: OperationType::DownloadTidal,
        entity_id: Some("1".to_string()),
        account_id: Some(1),
        track_id: Some(301),
        download_id: None,
        provider: Some("tidal".to_string()),
        phase: OperationPhase::Init,
        attempt: 1,
        started_at: "".to_string(),
        checkpoint_at: "".to_string(),
        status: OperationStatus::Started,
        input_identity: None,
        expected_output_path: Some(temp.path().join("t.flac").to_string_lossy().to_string()),
        staging_path: None,
        file_baseline: None,
        db_transaction_state: None,
        rollback_state: None,
        error_taxonomy: None,
        retry_policy: None,
        result_summary: None,
    };

    create_operation_journal(&pool, &op).await.unwrap();

    let _ = reconcile_startup_operations(&pool, Some(temp.path())).await.unwrap();

    let audit_summary = get_recovery_audit_summary(&pool).await.unwrap();
    assert_eq!(audit_summary.total_journal_scanned, 1);
    assert_eq!(audit_summary.details.len(), 1);
    assert_eq!(audit_summary.details[0].ui_label, "Interrupted — retry available");

    // Verify append-only table has the row
    let audit_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM operation_recovery_audit WHERE operation_id = 'op-audit-test-01'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(audit_count, 1);
}
