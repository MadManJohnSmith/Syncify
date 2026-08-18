use std::sync::{Arc, Mutex};
use syncify_tauri_lib::download::progress::{
    ByteStreamTracker, DownloadProgress, DownloadStatus, PROGRESS_TRACKER,
};
use syncify_tauri_lib::download::http_client::download_stream_to_file;
use tokio_util::sync::CancellationToken;

static TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[test]
fn test_byte_stream_tracker_with_content_length() {
    let mut tracker = ByteStreamTracker::new("track_101", "qobuz", Some(1_000_000));
    
    // Initial update (forced)
    let init = tracker.on_bytes(0, true).expect("Forced initial update");
    assert_eq!(init.item_id, "track_101");
    assert_eq!(init.service.as_deref(), Some("qobuz"));
    assert_eq!(init.bytes_downloaded, 0);
    assert_eq!(init.total_bytes, Some(1_000_000));
    assert_eq!(init.percent, Some(0.0));
    assert_eq!(init.phase, "downloading");
    assert_eq!(init.terminal, false);

    // Immediate intermediate update within 250ms should be throttled
    let throttled = tracker.on_bytes(100_000, false);
    assert!(throttled.is_none(), "Expected update within 250ms to be throttled");

    // Force update returns progress
    let forced = tracker.on_bytes(500_000, true).expect("Forced progress");
    assert_eq!(forced.bytes_downloaded, 500_000);
    assert_eq!(forced.total_bytes, Some(1_000_000));
    assert_eq!(forced.percent, Some(50.0));
    assert_eq!(forced.phase, "downloading");
    assert_eq!(forced.terminal, false);
}

#[test]
fn test_byte_stream_tracker_missing_content_length_no_fake_percent() {
    let mut tracker = ByteStreamTracker::new("track_102", "tidal", None);
    
    let init = tracker.on_bytes(0, true).expect("Forced initial update");
    assert_eq!(init.item_id, "track_102");
    assert_eq!(init.bytes_downloaded, 0);
    assert_eq!(init.total_bytes, None);
    assert_eq!(init.percent, None, "Must not invent fake percentage when total_bytes is None");

    let progress = tracker.on_bytes(2_500_000, true).expect("Forced progress");
    assert_eq!(progress.bytes_downloaded, 2_500_000);
    assert_eq!(progress.total_bytes, None);
    assert_eq!(progress.percent, None, "Must not invent fake percentage during download when Content-Length is missing");
}

#[tokio::test]
async fn test_progress_tracker_emitter_subscription() {
    let _lock = TEST_LOCK.lock().await;
    let events = Arc::new(Mutex::new(Vec::new()));
    let events_clone = events.clone();

    PROGRESS_TRACKER.set_emitter(move |prog| {
        if prog.item_id == "q_999" {
            events_clone.lock().unwrap().push(prog.clone());
        }
    });

    PROGRESS_TRACKER.update(DownloadProgress::downloading_bytes(
        "q_999",
        "qobuz",
        1024,
        Some(2048),
        150.0,
        140.0,
    ));

    PROGRESS_TRACKER.update(DownloadProgress::complete("q_999"));

    PROGRESS_TRACKER.clear_emitter();

    let captured = events.lock().unwrap().clone();
    assert_eq!(captured.len(), 2);
    
    // Check first event
    assert_eq!(captured[0].item_id, "q_999");
    assert_eq!(captured[0].status, DownloadStatus::Downloading);
    assert_eq!(captured[0].bytes_downloaded, 1024);
    assert_eq!(captured[0].total_bytes, Some(2048));
    assert_eq!(captured[0].percent, Some(50.0));
    assert_eq!(captured[0].instant_kbps, 150.0);
    assert_eq!(captured[0].average_kbps, 140.0);
    assert_eq!(captured[0].phase, "downloading");
    assert_eq!(captured[0].terminal, false);

    // Check terminal complete event
    assert_eq!(captured[1].item_id, "q_999");
    assert_eq!(captured[1].status, DownloadStatus::Complete);
    assert_eq!(captured[1].percent, Some(100.0));
    assert_eq!(captured[1].phase, "complete");
    assert_eq!(captured[1].terminal, true);
}

/// Helper function to start a local raw HTTP server for streaming payloads
async fn start_mock_http_server(
    payload: Vec<u8>,
    with_content_length: bool,
) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{}", addr);

    let handle = tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = [0u8; 1024];
            let _ = socket.read(&mut buf).await;

            let header = if with_content_length {
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                    payload.len()
                )
            } else {
                "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n".to_string()
            };

            let _ = socket.write_all(header.as_bytes()).await;
            // Write chunks to simulate streaming
            for chunk in payload.chunks(16 * 1024) {
                let _ = socket.write_all(chunk).await;
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
            let _ = socket.flush().await;
        }
    });

    (url, handle)
}

#[tokio::test]
async fn test_download_stream_to_file_with_content_length() {
    let _lock = TEST_LOCK.lock().await;
    let payload = vec![0x42u8; 100 * 1024]; // 100KB
    let (server_url, _server_handle) = start_mock_http_server(payload, true).await;

    let response = reqwest::get(&server_url)
        .await
        .expect("Failed to perform GET");

    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let target_file = temp_dir.path().join("audio.flac");

    let events = Arc::new(Mutex::new(Vec::new()));
    let events_clone = events.clone();

    PROGRESS_TRACKER.set_emitter(move |p| {
        if p.item_id == "test_item_1" {
            events_clone.lock().unwrap().push(p.clone());
        }
    });

    let bytes_written = download_stream_to_file(
        response,
        &target_file,
        "test_item_1",
        "mock_service",
        None,
        |_d, _t| {},
    ).await.expect("download_stream_to_file failed");

    PROGRESS_TRACKER.clear_emitter();

    assert_eq!(bytes_written, 100 * 1024);
    assert!(target_file.exists());
    assert_eq!(tokio::fs::metadata(&target_file).await.unwrap().len(), 100 * 1024);

    let recorded = events.lock().unwrap().clone();
    assert!(!recorded.is_empty(), "Recorded events should not be empty");

    // Monotonicity check
    let mut prev_bytes = 0;
    for ev in &recorded {
        assert!(ev.bytes_downloaded >= prev_bytes, "Bytes must be monotonic");
        prev_bytes = ev.bytes_downloaded;
        if let Some(total) = ev.total_bytes {
            assert_eq!(total, 100 * 1024);
        }
        if let Some(perc) = ev.percent {
            assert!(perc >= 0.0 && perc <= 100.0);
        }
    }
}

#[tokio::test]
async fn test_download_stream_to_file_chunked_missing_content_length() {
    let _lock = TEST_LOCK.lock().await;
    let payload = vec![0xAAu8; 64 * 1024];
    let (server_url, _server_handle) = start_mock_http_server(payload, false).await;

    let response = reqwest::get(&server_url)
        .await
        .expect("Failed to perform GET");

    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let target_file = temp_dir.path().join("chunked.flac");

    let events = Arc::new(Mutex::new(Vec::new()));
    let events_clone = events.clone();

    PROGRESS_TRACKER.set_emitter(move |p| {
        if p.item_id == "test_item_chunked" {
            events_clone.lock().unwrap().push(p.clone());
        }
    });

    let bytes_written = download_stream_to_file(
        response,
        &target_file,
        "test_item_chunked",
        "mock_service",
        None,
        |_d, _t| {},
    ).await.expect("download_stream_to_file chunked failed");

    PROGRESS_TRACKER.clear_emitter();

    assert_eq!(bytes_written, 64 * 1024);
    let recorded = events.lock().unwrap().clone();
    assert!(!recorded.is_empty());

    for ev in &recorded {
        assert_eq!(ev.total_bytes, None, "Total bytes must be None when Content-Length is missing");
        assert_eq!(ev.percent, None, "Percent must be None when total_bytes is None");
    }
}

#[tokio::test]
async fn test_download_stream_cancellation_cleans_up_and_emits_terminal() {
    let _lock = TEST_LOCK.lock().await;
    let payload = vec![0x55u8; 500 * 1024];
    let (server_url, _server_handle) = start_mock_http_server(payload, true).await;

    let response = reqwest::get(&server_url)
        .await
        .expect("Failed to perform GET");

    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let target_file = temp_dir.path().join("cancel.flac");

    let cancel_token = CancellationToken::new();
    cancel_token.cancel(); // Pre-cancel

    let events = Arc::new(Mutex::new(Vec::new()));
    let events_clone = events.clone();

    PROGRESS_TRACKER.set_emitter(move |p| {
        if p.item_id == "test_item_cancel" {
            events_clone.lock().unwrap().push(p.clone());
        }
    });

    let res = download_stream_to_file(
        response,
        &target_file,
        "test_item_cancel",
        "mock_service",
        Some(&cancel_token),
        |_d, _t| {},
    ).await;

    PROGRESS_TRACKER.clear_emitter();

    assert!(res.is_err(), "Must error on cancellation");
    assert!(!target_file.exists(), "Target file must be removed upon cancellation");

    let recorded = events.lock().unwrap().clone();
    let cancelled_ev = recorded.iter().find(|e| e.status == DownloadStatus::Cancelled);
    assert!(cancelled_ev.is_some(), "Must emit cancelled status event");
    assert_eq!(cancelled_ev.unwrap().terminal, true, "Cancelled event must be terminal");
}

#[test]
fn test_no_secrets_in_progress_events() {
    let progress = DownloadProgress::downloading_bytes(
        "q_secret_test",
        "qobuz",
        5000,
        Some(10000),
        500.0,
        450.0,
    );

    let serialized = serde_json::to_string(&progress).expect("Serialization failed");

    assert!(!serialized.contains("authorization"), "Must not leak authorization headers");
    assert!(!serialized.contains("bearer"), "Must not leak bearer tokens");
    assert!(!serialized.contains("cookie"), "Must not leak cookies");
    assert!(!serialized.contains("signature"), "Must not leak signatures");
}
