//! TASK-7.1 / CR-7: the retry policy is the shared `ErrorTaxonomy`, not a hand-rolled
//! scan of the error text.
//!
//! Covers the three places that make the decision:
//! 1. `download::qobuz::classify_qobuz_http_failure` — what the downloader retries and
//!    how long it waits.
//! 2. `commands::queue::perform_retry_queue_item` / `perform_retry_all_failed` — what
//!    the user may re-queue, and the message they get when they may not.

use sqlx::sqlite::SqlitePoolOptions;
use syncify_core_domain::errors::ErrorTaxonomy;
use syncify_tauri_lib::commands::queue::{perform_retry_all_failed, perform_retry_queue_item};
use syncify_tauri_lib::download::qobuz::classify_qobuz_http_failure;

fn terminal(t: &ErrorTaxonomy) -> bool {
    t.is_terminal() || t.requires_user_action()
}

#[test]
fn qobuz_http_failures_drive_the_retry_decision_from_the_taxonomy() {
    // Rate limiting and server errors are the retryable classes.
    let rate_limited = classify_qobuz_http_failure(reqwest::StatusCode::TOO_MANY_REQUESTS, "42");
    assert!(rate_limited.is_retryable());
    assert_eq!(rate_limited.max_attempts(), 3);
    assert!(
        rate_limited.retry_delay_sec() > 0,
        "a rate limit without Retry-After still gets a positive wait"
    );
    assert!(rate_limited.ui_message().contains("qobuz"));

    for status in [
        reqwest::StatusCode::BAD_GATEWAY,
        reqwest::StatusCode::SERVICE_UNAVAILABLE,
        reqwest::StatusCode::GATEWAY_TIMEOUT,
        reqwest::StatusCode::REQUEST_TIMEOUT,
        reqwest::StatusCode::INTERNAL_SERVER_ERROR,
    ] {
        let t = classify_qobuz_http_failure(status, "42");
        assert!(t.is_retryable(), "HTTP {} must stay retryable", status);
        assert_eq!(t.max_attempts(), 3);
        assert!(
            t.ui_message().contains(&status.to_string()),
            "the status must survive into the message so the worker classifier sees it"
        );
    }

    // Credentials, missing items and other permanent 4xx are terminal.
    for status in [
        reqwest::StatusCode::UNAUTHORIZED,
        reqwest::StatusCode::FORBIDDEN,
        reqwest::StatusCode::NOT_FOUND,
        reqwest::StatusCode::BAD_REQUEST,
        reqwest::StatusCode::GONE,
    ] {
        let t = classify_qobuz_http_failure(status, "42");
        assert!(!t.is_retryable(), "HTTP {} must not be retried", status);
        assert!(terminal(&t), "HTTP {} must be a terminal failure", status);
        assert_eq!(t.max_attempts(), 1);
        assert!(
            t.ui_message().contains(&status.to_string()),
            "the status must survive into the message: {}",
            t.ui_message()
        );
    }

    // An auth rejection is exactly what requires the user to act.
    assert!(
        classify_qobuz_http_failure(reqwest::StatusCode::UNAUTHORIZED, "42")
            .invalidates_credentials()
    );
}

async fn queue_db() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations");
    pool
}

async fn seed_queue_row(
    db: &sqlx::SqlitePool,
    service: &str,
    error_message: &str,
    retry_count: i64,
) -> i64 {
    let track_id: i64 =
        sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Retry Fixture') RETURNING id")
            .fetch_one(db)
            .await
            .expect("track row");
    sqlx::query_scalar(
        r#"INSERT INTO download_queue (track_id, service_name, status, error_message, last_error, retry_count)
           VALUES (?, ?, 'failed', ?, ?, ?) RETURNING id"#,
    )
    .bind(track_id)
    .bind(service)
    .bind(error_message)
    .bind(error_message)
    .bind(retry_count)
    .fetch_one(db)
    .await
    .expect("queue row")
}

#[tokio::test]
async fn retry_queue_item_refuses_terminal_failures_with_the_taxonomy_message() {
    let db = queue_db().await;

    let terminal_row = seed_queue_row(
        &db,
        "qobuz",
        "AmbiguousSource: Multiple matching tracks found without ISRC/MBID",
        1,
    )
    .await;
    let err = perform_retry_queue_item(&db, terminal_row)
        .await
        .expect_err("terminal failure must not be retryable");
    assert!(
        err.starts_with("Cannot auto-retry terminal failure:"),
        "unexpected message: {}",
        err
    );
    let taxonomy = syncify_tauri_lib::services::operation_recovery::classify_operation_error(
        syncify_core_domain::OperationType::DownloadTidal,
        "qobuz",
        "AmbiguousSource: Multiple matching tracks found without ISRC/MBID",
    );
    assert_eq!(
        err,
        format!(
            "Cannot auto-retry terminal failure: {}",
            taxonomy.ui_message()
        )
    );

    let transient_row = seed_queue_row(
        &db,
        "qobuz",
        "NetworkExhausted: Network error after 3 attempts: connection reset",
        1,
    )
    .await;
    // A spent retry budget is still an error the queue may re-attempt explicitly...
    perform_retry_queue_item(&db, transient_row)
        .await
        .expect("transient failure stays retryable");
    let status: String = sqlx::query_scalar("SELECT status FROM download_queue WHERE id = ?")
        .bind(transient_row)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(status, "queued");

    // ...unless the row already exhausted its budget.
    let exhausted = seed_queue_row(
        &db,
        "qobuz",
        "NetworkExhausted: Network error after 3 attempts: connection reset",
        99,
    )
    .await;
    assert!(perform_retry_queue_item(&db, exhausted).await.is_err());
}

#[tokio::test]
async fn retry_all_failed_agrees_with_the_single_item_verdict() {
    let db = queue_db().await;

    let transient =
        seed_queue_row(&db, "qobuz", "Network timeout: connection reset by peer", 1).await;
    let ambiguous = seed_queue_row(
        &db,
        "qobuz",
        "AmbiguousSource: Multiple matching tracks found without ISRC/MBID",
        99,
    )
    .await;
    let expired = seed_queue_row(&db, "qobuz", "RequiresAuth: token expired 401", 1).await;
    let not_found = seed_queue_row(&db, "tidal", "TrackUnresolved: not found on provider", 1).await;

    let retried = perform_retry_all_failed(&db).await.expect("retry all");
    assert_eq!(retried, 1, "only the transient row may be re-queued");

    let rows: Vec<(i64, String, i64)> = sqlx::query_as(
        "SELECT id, status, retry_count FROM download_queue WHERE id IN (?, ?, ?, ?)",
    )
    .bind(transient)
    .bind(ambiguous)
    .bind(expired)
    .bind(not_found)
    .fetch_all(&db)
    .await
    .unwrap();

    let mut by_id: std::collections::HashMap<i64, (String, i64)> = rows
        .into_iter()
        .map(|(id, status, rc)| (id, (status, rc)))
        .collect();
    assert_eq!(by_id.remove(&transient).unwrap(), ("queued".to_string(), 2));
    assert_eq!(
        by_id.remove(&ambiguous).unwrap(),
        ("failed".to_string(), 99)
    );
    assert_eq!(by_id.remove(&expired).unwrap(), ("failed".to_string(), 1));
    assert_eq!(by_id.remove(&not_found).unwrap(), ("failed".to_string(), 1));
}
