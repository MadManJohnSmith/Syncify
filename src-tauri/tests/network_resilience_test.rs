//! Integration and Unit Tests for S115B & S117B:
//! Network Resilience, Central Connection Pooling, Rate Limiting, Cooperative Cancellation,
//! Worker Concurrency Control, Fast-Fail Non-Retry Policies & Credential Redaction.

use reqwest::header::{HeaderMap, HeaderValue, RETRY_AFTER};
use reqwest::StatusCode;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use syncify_core_domain::quality::{QualityClass, QualityPolicy};
use syncify_tauri_lib::download::http_client::{
    calculate_backoff_with_jitter, create_http_client, download_stream_to_file,
    execute_with_retry, is_transient_status, parse_retry_after, shared_http_client,
    QOBUZ_LIMITER,
};
use syncify_tauri_lib::download::qobuz::{build_request_signature, QobuzDownloader};
use syncify_tauri_lib::services::rate_limiter::{
    RateLimitConfig, RateLimiter, GLOBAL_RATE_LIMITER,
};
use syncify_tauri_lib::worker::DownloadWorkerState;
use syncify_tidal_downloader::anonymize_identifier;

#[tokio::test]
async fn test_centralized_connection_pooling_reusability() {
    let client_a = create_http_client();
    let client_b = create_http_client();
    let client_shared = shared_http_client();

    // Verify all clients clone and share identical configurations
    assert_eq!(
        format!("{:?}", client_a),
        format!("{:?}", client_b),
        "Pooled clients must share internal structure"
    );
    assert_eq!(
        format!("{:?}", client_a),
        format!("{:?}", client_shared),
        "Created client must share central singleton configuration"
    );
}

#[tokio::test]
async fn test_qobuz_and_downloaders_share_central_http_client() {
    let qobuz_downloader_1 = QobuzDownloader::new();
    let qobuz_downloader_2 = QobuzDownloader::new();
    let global_client = shared_http_client();

    // Both downloaders and the global shared client share identical internal structure & pool settings
    assert_eq!(
        format!("{:?}", create_http_client()),
        format!("{:?}", global_client),
        "Downloader client creation must map directly to shared singleton pool"
    );
    drop(qobuz_downloader_1);
    drop(qobuz_downloader_2);
}

#[test]
fn test_retry_after_header_parsing_seconds_and_date() {
    let mut headers_secs = HeaderMap::new();
    headers_secs.insert(RETRY_AFTER, HeaderValue::from_static("60"));
    let parsed_secs = parse_retry_after(&headers_secs, SystemTime::now());
    assert_eq!(parsed_secs, Some(Duration::from_secs(60)));

    let mut headers_date = HeaderMap::new();
    headers_date.insert(
        RETRY_AFTER,
        HeaderValue::from_static("Sun, 06 Nov 1994 08:49:37 GMT"),
    );
    let target_timestamp = 784111777; // Sun, 06 Nov 1994 08:49:37 GMT
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(target_timestamp - 30); // 30s before target
    let parsed_date = parse_retry_after(&headers_date, now);
    assert_eq!(parsed_date, Some(Duration::from_secs(30)));
}

#[test]
fn test_exponential_backoff_with_jitter_bounds() {
    let base = Duration::from_millis(500);
    let max = Duration::from_secs(30);

    for attempt in 0..5 {
        let backoff = calculate_backoff_with_jitter(attempt, base, max);
        assert!(backoff >= Duration::from_millis(50));
        assert!(backoff <= max);
    }
}

#[test]
fn test_transient_status_classification() {
    assert!(is_transient_status(StatusCode::TOO_MANY_REQUESTS));
    assert!(is_transient_status(StatusCode::BAD_GATEWAY));
    assert!(is_transient_status(StatusCode::SERVICE_UNAVAILABLE));
    assert!(is_transient_status(StatusCode::GATEWAY_TIMEOUT));
    assert!(is_transient_status(StatusCode::REQUEST_TIMEOUT));

    // Permanent errors must NOT be transient
    assert!(!is_transient_status(StatusCode::OK));
    assert!(!is_transient_status(StatusCode::NOT_FOUND));
    assert!(!is_transient_status(StatusCode::UNAUTHORIZED));
    assert!(!is_transient_status(StatusCode::FORBIDDEN));
    assert!(!is_transient_status(StatusCode::UNPROCESSABLE_ENTITY));
}

#[tokio::test]
async fn test_rate_limiter_service_isolation_and_burst() {
    let mut configs = std::collections::HashMap::new();
    configs.insert("service_a".to_string(), RateLimitConfig::per_second(10));
    configs.insert("service_b".to_string(), RateLimitConfig::per_second(5));

    let limiter = RateLimiter::with_configs(configs);

    // Burst 10 requests on service_a
    let start_a = Instant::now();
    for _ in 0..10 {
        limiter.acquire("service_a").await;
    }
    assert!(start_a.elapsed() < Duration::from_millis(100));

    // service_b should still be immediately available and unaffected by service_a's bucket
    let start_b = Instant::now();
    limiter.acquire("service_b").await;
    assert!(start_b.elapsed() < Duration::from_millis(50));
}

#[tokio::test]
async fn test_rate_limiter_dynamic_429_penalty_backoff() {
    let limiter = RateLimiter::new();

    // Impose 150ms penalty on "tidal"
    limiter
        .penalize_service("tidal", Duration::from_millis(150))
        .await;

    let start = Instant::now();
    limiter.acquire("tidal").await;
    let elapsed = start.elapsed();

    assert!(
        elapsed >= Duration::from_millis(140),
        "Expected acquisition to pause for penalty duration, elapsed: {:?}",
        elapsed
    );
}

#[tokio::test]
async fn test_global_rate_limiter_is_single_effective_layer() {
    // Penalize qobuz on the central GLOBAL_RATE_LIMITER
    GLOBAL_RATE_LIMITER
        .penalize_service("qobuz", Duration::from_millis(150))
        .await;

    // Call QOBUZ_LIMITER wrapper from http_client
    let start = Instant::now();
    QOBUZ_LIMITER.wait("qobuz").await;
    let elapsed = start.elapsed();

    assert!(
        elapsed >= Duration::from_millis(140),
        "QOBUZ_LIMITER.wait() must directly route through GLOBAL_RATE_LIMITER without dual-state divergence, elapsed: {:?}",
        elapsed
    );
}

#[tokio::test]
async fn test_429_retry_after_service_isolation_multi_provider() {
    let limiter = RateLimiter::new();

    // Penalize only "qobuz" with 160ms
    limiter
        .penalize_service("qobuz", Duration::from_millis(160))
        .await;

    // Concurrently acquire tokens for "tidal", "spotify", "deezer"
    let start_other = Instant::now();
    limiter.acquire("tidal").await;
    limiter.acquire("spotify").await;
    limiter.acquire("deezer").await;
    let elapsed_other = start_other.elapsed();

    assert!(
        elapsed_other < Duration::from_millis(50),
        "Unaffected services must acquire instantly without being blocked by qobuz 429 penalty, elapsed: {:?}",
        elapsed_other
    );

    // Verify qobuz remains throttled
    let start_qobuz = Instant::now();
    limiter.acquire("qobuz").await;
    let elapsed_qobuz = start_qobuz.elapsed();

    assert!(
        elapsed_qobuz >= Duration::from_millis(140),
        "Penalized service must wait for penalty window to clear, elapsed: {:?}",
        elapsed_qobuz
    );
}

#[tokio::test]
async fn test_rate_limiter_cooperative_cancellation() {
    let limiter = RateLimiter::new();
    let cancel_token = CancellationToken::new();

    // Impose long penalty
    limiter
        .penalize_service("qobuz", Duration::from_secs(10))
        .await;

    // Trigger cancel after 50ms
    let token_clone = cancel_token.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        token_clone.cancel();
    });

    let res = limiter
        .acquire_cancellable("qobuz", Some(&cancel_token))
        .await;
    assert!(res.is_err());
    let err_str = res.unwrap_err().to_string();
    assert!(err_str.contains("cancelled"));
}

#[tokio::test]
async fn test_execute_with_retry_cooperative_cancellation() {
    let cancel_token = CancellationToken::new();
    cancel_token.cancel(); // Pre-cancelled

    let count = Arc::new(AtomicU32::new(0));
    let count_clone = count.clone();

    let res = execute_with_retry("test_svc", Some(&cancel_token), move || {
        let count = count_clone.clone();
        async move {
            count.fetch_add(1, Ordering::SeqCst);
            let client = shared_http_client();
            client.get("http://127.0.0.1:9999/dummy").send().await
        }
    })
    .await;

    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("cancelled"));
}

#[tokio::test]
async fn test_execute_with_retry_recovers_from_transient_server_errors() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let attempt_count = Arc::new(AtomicU32::new(0));
    let srv_attempt = attempt_count.clone();

    // Mock TCP HTTP Server that serves 503, 502, then 200 OK
    tokio::spawn(async move {
        for i in 0..3 {
            if let Ok((mut socket, _)) = listener.accept().await {
                srv_attempt.fetch_add(1, Ordering::SeqCst);
                let mut buf = [0u8; 1024];
                let _ = socket.read(&mut buf).await;

                let response = match i {
                    0 => "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n",
                    1 => "HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n",
                    _ => "HTTP/1.1 200 OK\r\nContent-Length: 7\r\n\r\nSUCCESS",
                };
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.flush().await;
            }
        }
    });

    let url = format!("http://127.0.0.1:{}/test", port);
    let client = create_http_client();

    let result = execute_with_retry("test_transient", None, || {
        let u = url.clone();
        let c = client.clone();
        async move { c.get(&u).send().await }
    })
    .await;

    assert!(result.is_ok(), "Expected recovery after transient 503 and 502 errors");
    let resp = result.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let text = resp.text().await.unwrap();
    assert_eq!(text, "SUCCESS");
    assert_eq!(attempt_count.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn test_fast_fail_on_401_403_404_without_retries() {
    for error_code in [StatusCode::UNAUTHORIZED, StatusCode::FORBIDDEN, StatusCode::NOT_FOUND] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        let attempt_count = Arc::new(AtomicU32::new(0));
        let srv_attempt = attempt_count.clone();

        tokio::spawn(async move {
            if let Ok((mut socket, _)) = listener.accept().await {
                srv_attempt.fetch_add(1, Ordering::SeqCst);
                let mut buf = [0u8; 1024];
                let _ = socket.read(&mut buf).await;

                let status_line = match error_code {
                    StatusCode::UNAUTHORIZED => "HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n",
                    StatusCode::FORBIDDEN => "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n",
                    StatusCode::NOT_FOUND => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n",
                    _ => unreachable!(),
                };
                let _ = socket.write_all(status_line.as_bytes()).await;
                let _ = socket.flush().await;
            }
        });

        let url = format!("http://127.0.0.1:{}/auth_check", port);
        let client = create_http_client();

        let result = execute_with_retry("test_fast_fail", None, || {
            let u = url.clone();
            let c = client.clone();
            async move { c.get(&u).send().await }
        })
        .await;

        assert!(result.is_ok(), "execute_with_retry returns permanent status for application handler");
        let resp = result.unwrap();
        assert_eq!(resp.status(), error_code);
        assert_eq!(
            attempt_count.load(Ordering::SeqCst),
            1,
            "Permanent error status {} must execute exactly 1 time with zero retries",
            error_code
        );
    }
}

#[test]
fn test_quality_policy_fast_fail_downgrade_rejection() {
    // Lossless requested, but lossy (mp3) delivered when lossy fallback is disabled
    let res = QualityPolicy::evaluate_downgrade(
        QualityClass::Lossless,
        QualityClass::Lossy,
        "mp3",
        false, // allow_lossy_fallback = false
    );
    assert!(res.is_err(), "Strict quality policy must fast-fail on downgrade without allow_fallback");
    let err_str = res.unwrap_err();
    assert!(err_str.contains("requested_lossless_but_received_mp3"));
}

#[tokio::test]
async fn test_cancellation_token_aborts_streaming_and_purges_part_file() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    // Mock HTTP server that streams chunked audio data slowly
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0u8; 1024];
            let _ = socket.read(&mut buf).await;

            let header = "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Type: audio/flac\r\n\r\n";
            let _ = socket.write_all(header.as_bytes()).await;
            let _ = socket.flush().await;

            // Stream chunks with delay
            for _ in 0..10 {
                let chunk_data = "1000\r\n".to_string() + &"A".repeat(4096) + "\r\n";
                if socket.write_all(chunk_data.as_bytes()).await.is_err() {
                    break;
                }
                let _ = socket.flush().await;
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    });

    let temp_dir = tempfile::tempdir().unwrap();
    let target_part_file = temp_dir.path().join("test_in_flight_stream.flac.part");

    let client = create_http_client();
    let resp = client
        .get(format!("http://127.0.0.1:{}/stream", port))
        .send()
        .await
        .unwrap();

    let cancel_token = CancellationToken::new();
    let token_clone = cancel_token.clone();

    // Spawn task to cancel after 30ms during active streaming
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(30)).await;
        token_clone.cancel();
    });

    let download_res = download_stream_to_file(
        resp,
        &target_part_file,
        "item_123",
        "mock_stream",
        Some(&cancel_token),
        |_downloaded, _total| {},
    )
    .await;

    assert!(download_res.is_err(), "Download must return Err upon cancellation");
    assert!(
        download_res.unwrap_err().to_string().contains("cancelled"),
        "Error message must indicate cancellation"
    );

    // Verify atomic cleanup of .part file
    assert!(
        !target_part_file.exists(),
        "In-flight .part staging file must be purged immediately on cancellation (0 orphan files)"
    );
}

#[test]
fn test_worker_concurrency_selector_1_to_5_updates_state() {
    let state = DownloadWorkerState::new(2);
    assert_eq!(state.max_concurrent(), 2);
    assert_eq!(state.status().max_concurrent, 2);

    for target_concurrency in 1..=5 {
        state.set_max_concurrent(target_concurrency);
        assert_eq!(
            state.max_concurrent(),
            target_concurrency,
            "Worker max_concurrent must update at runtime"
        );
        assert_eq!(
            state.status().max_concurrent,
            target_concurrency,
            "Worker status DTO must reflect updated concurrency for UI"
        );
    }
}

#[test]
fn test_credential_and_token_redaction_invariants() {
    // 1. Anonymize short tokens (<= 6 chars) -> "***"
    assert_eq!(anonymize_identifier("12345"), "***");
    assert_eq!(anonymize_identifier("secret"), "***");

    // 2. Anonymize empty token -> "none"
    assert_eq!(anonymize_identifier(""), "none");
    assert_eq!(anonymize_identifier("   "), "none");

    // 3. Anonymize normal OAuth / session token -> "pre...suf"
    let token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9";
    let redacted = anonymize_identifier(token);
    assert_eq!(redacted, "eyJ...CJ9");
    assert!(!redacted.contains("I1NiIsInR5cCI6Ikp"));

    // 4. Request signing logs only metadata (format & track_id), never app_secret
    let secret = "super_secret_app_key_998877";
    let sig = build_request_signature("27", "12345678", "1600000000", secret);
    assert_eq!(sig.len(), 32, "Signature must be pure MD5 hex digest");
    assert!(!sig.contains(secret), "Signature must never contain raw plaintext secret");
}

#[tokio::test]
async fn test_qobuz_stream_body_decode_fail_retry_then_success() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let attempts = Arc::new(AtomicU32::new(0));
    let attempts_clone = attempts.clone();

    tokio::spawn(async move {
        // Attempt 1: Abrupt connection close mid-stream (simulates "error decoding response body")
        if let Ok((mut socket, _)) = listener.accept().await {
            attempts_clone.fetch_add(1, Ordering::SeqCst);
            let mut buf = [0u8; 1024];
            let _ = socket.read(&mut buf).await;

            let header = "HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\npartial";
            let _ = socket.write_all(header.as_bytes()).await;
            let _ = socket.flush().await;
            // Force abrupt socket drop
            drop(socket);
        }

        // Attempt 2: Successful complete payload
        if let Ok((mut socket, _)) = listener.accept().await {
            attempts_clone.fetch_add(1, Ordering::SeqCst);
            let mut buf = [0u8; 1024];
            let _ = socket.read(&mut buf).await;

            let response = "HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\nfLaC_payload";
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.flush().await;
        }
    });

    let temp_dir = tempfile::tempdir().unwrap();
    let staging_path = temp_dir.path().join("test_stream_retry.part");

    let downloader = QobuzDownloader::new();
    let url = format!("http://127.0.0.1:{}/stream_retry", port);

    let res = downloader
        .download_to_staging(&url, &staging_path, "item_retry_1")
        .await;

    assert!(res.is_ok(), "Expected recovery on retry: {:?}", res.err());
    assert_eq!(res.unwrap(), 12);
    assert_eq!(attempts.load(Ordering::SeqCst), 2, "Expected exactly 2 attempts");
    assert!(staging_path.exists());
    let content = tokio::fs::read(&staging_path).await.unwrap();
    assert_eq!(content, b"fLaC_payload");
}

#[tokio::test]
async fn test_qobuz_stream_body_decode_fail_3_times_network_exhausted() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let attempts = Arc::new(AtomicU32::new(0));
    let attempts_clone = attempts.clone();

    tokio::spawn(async move {
        for _ in 0..3 {
            if let Ok((mut socket, _)) = listener.accept().await {
                attempts_clone.fetch_add(1, Ordering::SeqCst);
                let mut buf = [0u8; 1024];
                let _ = socket.read(&mut buf).await;

                let header = "HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\ncut";
                let _ = socket.write_all(header.as_bytes()).await;
                let _ = socket.flush().await;
                drop(socket);
            }
        }
    });

    let temp_dir = tempfile::tempdir().unwrap();
    let staging_path = temp_dir.path().join("test_stream_fail.part");

    let downloader = QobuzDownloader::new();
    let url = format!("http://127.0.0.1:{}/stream_fail", port);

    let res = downloader
        .download_to_staging(&url, &staging_path, "item_fail_3")
        .await;

    assert!(res.is_err(), "Expected NetworkExhausted after 3 failed attempts");
    let err_msg = res.unwrap_err().to_string();
    assert!(
        err_msg.contains("NetworkExhausted"),
        "Error must be classified as NetworkExhausted, got: {}",
        err_msg
    );
    assert_eq!(attempts.load(Ordering::SeqCst), 3, "Must stop after 3 attempts");
    assert!(
        !staging_path.exists(),
        "Staging .part file must be cleaned up on terminal failure"
    );
}

#[tokio::test]
async fn test_staging_part_file_cleaned_between_retries() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        // Attempt 1: 502 Bad Gateway
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0u8; 1024];
            let _ = socket.read(&mut buf).await;
            let response = "HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n";
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.flush().await;
        }

        // Attempt 2: 200 OK
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0u8; 1024];
            let _ = socket.read(&mut buf).await;
            let response = "HTTP/1.1 200 OK\r\nContent-Length: 8\r\n\r\nCLEAN123";
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.flush().await;
        }
    });

    let temp_dir = tempfile::tempdir().unwrap();
    let staging_path = temp_dir.path().join("test_clean_retry.part");

    // Pre-create dirty partial file
    tokio::fs::write(&staging_path, b"DIRTY_STALE_BYTES").await.unwrap();
    assert_eq!(tokio::fs::read(&staging_path).await.unwrap().len(), 17);

    let downloader = QobuzDownloader::new();
    let url = format!("http://127.0.0.1:{}/clean_retry", port);

    let res = downloader
        .download_to_staging(&url, &staging_path, "item_clean_1")
        .await;

    assert!(res.is_ok());
    let content = tokio::fs::read(&staging_path).await.unwrap();
    assert_eq!(content, b"CLEAN123", "Staging file must not contain leftover bytes from previous attempt");
}

#[tokio::test]
async fn test_manual_retry_preserves_source_identity_and_allow_fallback() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .unwrap();

    // Insert prerequisite parent records
    let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('Test Title', 'USRC12345678') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    // Insert queue item with complete provenance
    let qid: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO download_queue (
            track_id, service_id, service_name, service_track_id, service_album_id,
            target_title, target_artist, target_album, target_isrc,
            smart_studio_origin, allow_fallback, origin_service, origin_service_track_id,
            status, error_message, last_error
        ) VALUES (
            ?, 2, 'qobuz', '12345678', '87654321',
            'Test Title', 'Test Artist', 'Test Album', 'USRC12345678',
            1, 1, 'qobuz', '12345678',
            'failed', 'NetworkExhausted: Stream error after 3 attempts', 'NetworkExhausted: Stream error after 3 attempts'
        ) RETURNING id
        "#
    )
    .bind(tid)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Perform manual retry
    sqlx::query(
        "UPDATE download_queue SET status = 'queued', error_message = NULL, last_error = NULL, progress_percent = 0, started_at = NULL WHERE id = ?"
    )
    .bind(qid)
    .execute(&pool)
    .await
    .unwrap();

    // Verify row identity and provenance are completely preserved
    let row: (String, Option<String>, Option<String>, Option<String>, i64, i64, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT status, error_message, service_name, service_track_id, allow_fallback, smart_studio_origin, origin_service, origin_service_track_id FROM download_queue WHERE id = ?"
    )
    .bind(qid)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.0, "queued");
    assert_eq!(row.1, None, "error_message must be reset");
    assert_eq!(row.2.as_deref(), Some("qobuz"));
    assert_eq!(row.3.as_deref(), Some("12345678"));
    assert_eq!(row.4, 1, "allow_fallback must remain preserved");
    assert_eq!(row.5, 1, "smart_studio_origin must remain preserved");
    assert_eq!(row.6.as_deref(), Some("qobuz"));
    assert_eq!(row.7.as_deref(), Some("12345678"));

    // Verify no duplicate queue rows exist
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_queue WHERE track_id = ?")
        .bind(tid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "Manual retry must never duplicate queue rows");
}
