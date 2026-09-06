use syncify_tauri_lib::commands::{
    build_spotify_callback_response, process_spotify_callback_request, validate_spotify_callback,
    SpotifyCallbackError,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[test]
fn test_build_spotify_callback_response_direct() {
    let success = Ok("test_code".to_string());
    let (status, resp) = build_spotify_callback_response(&success);
    assert_eq!(status, 200);
    assert!(resp.starts_with("HTTP/1.1 200 OK"));

    let missing = Err(SpotifyCallbackError::MissingState);
    let (status_m, resp_m) = build_spotify_callback_response(&missing);
    assert_eq!(status_m, 400);
    assert!(resp_m.starts_with("HTTP/1.1 400 Bad Request"));
}

#[test]
fn test_valid_callback_with_matching_state_extracts_code() {
    let expected_state = "cryptosecure_state_token_123456789";
    let valid_request = format!(
        "GET /callback?code=sp_auth_code_xyz789&state={} HTTP/1.1\r\nHost: 127.0.0.1:8888\r\n\r\n",
        expected_state
    );

    let result = validate_spotify_callback(&valid_request, expected_state);
    assert_eq!(result, Ok("sp_auth_code_xyz789".to_string()));

    let (status, processed_result, body) =
        process_spotify_callback_request(&valid_request, expected_state);
    assert_eq!(status, 200);
    assert_eq!(processed_result, Ok("sp_auth_code_xyz789".to_string()));
    assert!(body.starts_with("HTTP/1.1 200 OK"));
    assert!(body.contains("Autenticado"));
}

#[test]
fn test_valid_callback_with_reversed_query_parameters() {
    let expected_state = "expected_state_abc";
    let valid_request = format!(
        "GET /callback?state={}&code=reversed_param_code_456 HTTP/1.1\r\nHost: 127.0.0.1:8888\r\n\r\n",
        expected_state
    );

    let result = validate_spotify_callback(&valid_request, expected_state);
    assert_eq!(result, Ok("reversed_param_code_456".to_string()));

    let (status, processed_result, body) =
        process_spotify_callback_request(&valid_request, expected_state);
    assert_eq!(status, 200);
    assert_eq!(processed_result, Ok("reversed_param_code_456".to_string()));
    assert!(body.starts_with("HTTP/1.1 200 OK"));
}

#[test]
fn test_valid_callback_with_url_encoded_parameters() {
    let expected_state = "state+with/special=chars";
    let encoded_state = urlencoding::encode(expected_state);
    let raw_code = "code+special/value==";
    let encoded_code = urlencoding::encode(raw_code);

    let valid_request = format!(
        "GET /callback?code={}&state={} HTTP/1.1\r\nHost: 127.0.0.1:8888\r\n\r\n",
        encoded_code, encoded_state
    );

    let result = validate_spotify_callback(&valid_request, expected_state);
    assert_eq!(result, Ok(raw_code.to_string()));

    let (status, _, body) = process_spotify_callback_request(&valid_request, expected_state);
    assert_eq!(status, 200);
    assert!(body.starts_with("HTTP/1.1 200 OK"));
}

#[test]
fn test_callback_rejected_when_state_is_missing() {
    let expected_state = "legitimate_state_token";
    let request_without_state =
        "GET /callback?code=any_auth_code_received HTTP/1.1\r\nHost: 127.0.0.1:8888\r\n\r\n";

    let result = validate_spotify_callback(request_without_state, expected_state);
    assert_eq!(result, Err(SpotifyCallbackError::MissingState));

    let (status, processed_result, body) =
        process_spotify_callback_request(request_without_state, expected_state);
    assert_eq!(status, 400);
    assert_eq!(processed_result, Err(SpotifyCallbackError::MissingState));
    assert!(body.starts_with("HTTP/1.1 400 Bad Request"));
    assert!(body.contains("state ausente"));
}

#[test]
fn test_callback_rejected_when_state_mismatches_csrf() {
    let expected_state = "legitimate_state_token_123";
    let attacker_state = "forged_or_stale_state_456";
    let forged_request = format!(
        "GET /callback?code=stolen_or_forged_code&state={} HTTP/1.1\r\nHost: 127.0.0.1:8888\r\n\r\n",
        attacker_state
    );

    let result = validate_spotify_callback(&forged_request, expected_state);
    assert_eq!(result, Err(SpotifyCallbackError::InvalidState));

    let (status, processed_result, body) =
        process_spotify_callback_request(&forged_request, expected_state);
    assert_eq!(status, 400);
    assert_eq!(processed_result, Err(SpotifyCallbackError::InvalidState));
    assert!(body.starts_with("HTTP/1.1 400 Bad Request"));
    assert!(body.contains("state inv"));
}

#[test]
fn test_callback_rejected_when_code_is_missing() {
    let expected_state = "legitimate_state_token";
    let request_without_code = format!(
        "GET /callback?state={} HTTP/1.1\r\nHost: 127.0.0.1:8888\r\n\r\n",
        expected_state
    );

    let result = validate_spotify_callback(&request_without_code, expected_state);
    assert_eq!(result, Err(SpotifyCallbackError::MissingCode));

    let (status, processed_result, body) =
        process_spotify_callback_request(&request_without_code, expected_state);
    assert_eq!(status, 400);
    assert_eq!(processed_result, Err(SpotifyCallbackError::MissingCode));
    assert!(body.starts_with("HTTP/1.1 400 Bad Request"));
    assert!(body.contains("autorizaci"));
}

#[test]
fn test_callback_rejected_when_spotify_returns_oauth_error() {
    let expected_state = "legitimate_state_token";
    let error_request = format!(
        "GET /callback?error=access_denied&state={} HTTP/1.1\r\nHost: 127.0.0.1:8888\r\n\r\n",
        expected_state
    );

    let result = validate_spotify_callback(&error_request, expected_state);
    assert_eq!(
        result,
        Err(SpotifyCallbackError::OAuthError("access_denied".to_string()))
    );

    let (status, _, body) = process_spotify_callback_request(&error_request, expected_state);
    assert_eq!(status, 400);
    assert!(body.starts_with("HTTP/1.1 400 Bad Request"));
    assert!(body.contains("access_denied"));
}

#[test]
fn test_callback_rejected_for_non_callback_endpoints() {
    let expected_state = "some_state";
    let non_callback_request =
        "GET /favicon.ico HTTP/1.1\r\nHost: 127.0.0.1:8888\r\n\r\n";

    let result = validate_spotify_callback(non_callback_request, expected_state);
    assert_eq!(result, Err(SpotifyCallbackError::NotCallback));

    let (status, _, body) = process_spotify_callback_request(non_callback_request, expected_state);
    assert_eq!(status, 404);
    assert!(body.starts_with("HTTP/1.1 404 Not Found"));
}

#[tokio::test]
async fn test_tcp_listener_end_to_end_state_enforcement() {
    // Spin up dynamic TCP listener
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind ephemeral TCP port");
    let local_addr = listener.local_addr().expect("Failed to get local addr");
    let expected_state = "session_bound_cryptographic_state_999";

    // Server loop task replicating spotify_auth_webview logic
    let expected_state_clone = expected_state.to_string();
    let server_handle = tokio::spawn(async move {
        let captured = loop {
            let (mut socket, _) = listener.accept().await.expect("Accept failed");
            let mut buf = [0u8; 1024];
            let n = socket.read(&mut buf).await.expect("Read failed");
            if n == 0 {
                continue;
            }
            let request = String::from_utf8_lossy(&buf[..n]);
            let (_status, callback_res, response) =
                process_spotify_callback_request(&request, &expected_state_clone);

            socket
                .write_all(response.as_bytes())
                .await
                .expect("Write failed");
            socket.flush().await.expect("Flush failed");

            if let Ok(code) = callback_res {
                break code;
            }
        };
        Some(captured)
    });

    // 1. Client sends request with MISSING state -> must receive HTTP 400
    {
        let mut stream = TcpStream::connect(local_addr)
            .await
            .expect("Connect failed");
        let req = "GET /callback?code=unverified_code_001 HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";
        stream.write_all(req.as_bytes()).await.expect("Write failed");

        let mut res = String::new();
        stream.read_to_string(&mut res).await.expect("Read failed");
        assert!(
            res.starts_with("HTTP/1.1 400 Bad Request"),
            "Expected 400 for missing state, got: {}",
            res
        );
    }

    // 2. Client sends request with DIVERGENT state -> must receive HTTP 400
    {
        let mut stream = TcpStream::connect(local_addr)
            .await
            .expect("Connect failed");
        let req = "GET /callback?code=unverified_code_002&state=tampered_or_divergent_state HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";
        stream.write_all(req.as_bytes()).await.expect("Write failed");

        let mut res = String::new();
        stream.read_to_string(&mut res).await.expect("Read failed");
        assert!(
            res.starts_with("HTTP/1.1 400 Bad Request"),
            "Expected 400 for divergent state, got: {}",
            res
        );
    }

    // 3. Client sends request with MATCHING expected state -> must receive HTTP 200 and complete server loop
    {
        let mut stream = TcpStream::connect(local_addr)
            .await
            .expect("Connect failed");
        let req = format!(
            "GET /callback?code=legitimate_auth_code_777&state={} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
            expected_state
        );
        stream.write_all(req.as_bytes()).await.expect("Write failed");

        let mut res = String::new();
        stream.read_to_string(&mut res).await.expect("Read failed");
        assert!(
            res.starts_with("HTTP/1.1 200 OK"),
            "Expected 200 for matching state, got: {}",
            res
        );
    }

    // Verify server successfully terminated and extracted the legitimate code
    let captured_code = server_handle
        .await
        .expect("Server task panicked")
        .expect("Server did not capture code");
    assert_eq!(captured_code, "legitimate_auth_code_777");
}
