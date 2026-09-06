//! Integration and Resilience Tests for TASK-53:
//! Prevention of panic during TLS / builder initialization in http_client.rs.

use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use syncify_tauri_lib::download::http_client::{
    build_central_http_client, create_http_client, create_http_client_with_timeout,
    shared_http_client, DEFAULT_TIMEOUT,
};

#[test]
fn test_build_central_http_client_never_panics() {
    // Verify build_central_http_client returns a valid Client across various timeouts without panicking
    let client_standard = build_central_http_client(DEFAULT_TIMEOUT);
    let client_short = build_central_http_client(Duration::from_secs(5));
    let client_custom = build_central_http_client(Duration::from_millis(500));

    assert!(!format!("{:?}", client_standard).is_empty());
    assert!(!format!("{:?}", client_short).is_empty());
    assert!(!format!("{:?}", client_custom).is_empty());
}

#[tokio::test]
async fn test_shared_and_created_clients_are_valid() {
    let shared = shared_http_client();
    let created = create_http_client();
    let custom = create_http_client_with_timeout(Duration::from_secs(10));

    assert!(!format!("{:?}", shared).is_empty());
    assert!(!format!("{:?}", created).is_empty());
    assert!(!format!("{:?}", custom).is_empty());

    let cloned = created.clone();
    assert_eq!(format!("{:?}", created), format!("{:?}", cloned));
}

#[tokio::test]
async fn test_clients_are_functional_for_network_requests() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("Failed to bind ephemeral test port");
    let port = listener.local_addr().expect("Failed to get local addr").port();

    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buf = [0u8; 1024];
                let _ = socket.read(&mut buf).await;
                let response = "HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\nSYNCIFY";
                let _ = socket.write_all(response.as_bytes()).await;
            });
        }
    });

    // 1. Test shared client
    let shared = shared_http_client();
    let resp1 = shared
        .get(format!("http://127.0.0.1:{}/test1", port))
        .send()
        .await
        .expect("Shared client request failed");
    assert_eq!(resp1.status(), reqwest::StatusCode::OK);
    assert_eq!(resp1.text().await.unwrap(), "SYNCIFY");

    // 2. Test create_http_client()
    let client = create_http_client();
    let resp2 = client
        .get(format!("http://127.0.0.1:{}/test2", port))
        .send()
        .await
        .expect("Created client request failed");
    assert_eq!(resp2.status(), reqwest::StatusCode::OK);
    assert_eq!(resp2.text().await.unwrap(), "SYNCIFY");

    // 3. Test build_central_http_client directly
    let built_client = build_central_http_client(Duration::from_secs(3));
    let resp3 = built_client
        .get(format!("http://127.0.0.1:{}/test3", port))
        .send()
        .await
        .expect("Directly built client request failed");
    assert_eq!(resp3.status(), reqwest::StatusCode::OK);
    assert_eq!(resp3.text().await.unwrap(), "SYNCIFY");
}
