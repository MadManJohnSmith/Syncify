//! Integration test suite for TASK-149 (SEC-022):
//! Verification of secure transmission of Spotify sp_dc session cookie
//! via stdin rather than command line arguments.

use syncify_tauri_lib::commands::{refresh_spotify_session, run_auth_bridge_subprocess};

#[tokio::test]
async fn test_refresh_spotify_session_via_stdin_success() {
    std::env::set_var("SYNCIFY_AUTH_MOCK", "1");
    let test_cookie = "mock_secret_sp_dc_session_cookie_rust_test_123";

    let result = refresh_spotify_session(test_cookie.to_string()).await;
    assert!(result.is_ok(), "Expected refresh_spotify_session to succeed: {:?}", result);

    let auth_result = result.unwrap();
    assert!(auth_result.success, "AuthResult success flag must be true");
    assert!(auth_result.data.is_some(), "AuthResult must contain data payload");

    let data = auth_result.data.unwrap();
    let access_token = data.get("accessToken").and_then(|v| v.as_str());
    assert!(access_token.is_some(), "Expected accessToken in response: {:?}", data);
    assert!(!data.get("isAnonymous").and_then(|v| v.as_bool()).unwrap_or(true));
}

#[tokio::test]
async fn test_refresh_spotify_session_empty_cookie_rejected() {
    let result = refresh_spotify_session("   ".to_string()).await;
    assert!(result.is_err(), "Empty cookie must be rejected before spawning subprocess");
    let err = result.unwrap_err();
    assert!(err.contains("empty"), "Error message should mention empty cookie: {}", err);
}

#[tokio::test]
async fn test_refresh_without_stdin_fails_controlled() {
    // Unset SYNCIFY_SP_DC so neither stdin nor env provides it
    std::env::remove_var("SYNCIFY_SP_DC");
    std::env::remove_var("SPOTIFY_SP_DC");
    std::env::set_var("SYNCIFY_AUTH_MOCK", "1");

    let result = run_auth_bridge_subprocess("spotify", "refresh", None).await;
    assert!(result.is_err(), "Subprocess refresh without stdin must fail: {:?}", result);
    let err = result.unwrap_err();
    assert!(err.contains("sp_dc") || err.contains("Auth bridge error"), "Unexpected error: {}", err);
}

#[tokio::test]
async fn test_run_auth_bridge_subprocess_with_json_stdin() {
    std::env::set_var("SYNCIFY_AUTH_MOCK", "1");
    let json_payload = r#"{"sp_dc": "mock_secret_sp_dc_pipe_payload_456"}"#;

    let result = run_auth_bridge_subprocess("spotify", "refresh", Some(json_payload)).await;
    assert!(result.is_ok(), "Expected success with JSON payload: {:?}", result);

    let auth_result = result.unwrap();
    assert!(auth_result.success);
    assert!(auth_result.data.is_some());
}
