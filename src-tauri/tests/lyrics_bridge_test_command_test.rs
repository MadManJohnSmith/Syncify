//! Integration test suite for TASK-43: lyrics_bridge.py `test` command and stderr error propagation.

use syncify_tauri_lib::commands::{get_project_root, get_python_executable, test_lyrics_provider};

#[tokio::test]
async fn test_lyrics_provider_command_lrclib() {
    let result = test_lyrics_provider("lrclib".to_string()).await;
    assert!(
        result.is_ok(),
        "test_lyrics_provider('lrclib') should not error: {:?}",
        result.err()
    );
    let is_available = result.unwrap();
    // LRCLIB is expected to be reachable and report available
    assert!(
        is_available,
        "LRCLIB should report available when connectivity succeeds"
    );
}

#[tokio::test]
async fn test_lyrics_provider_command_apple_music() {
    let result = test_lyrics_provider("apple_music".to_string()).await;
    assert!(
        result.is_ok(),
        "test_lyrics_provider('apple_music') should return Ok: {:?}",
        result.err()
    );
    let is_available = result.unwrap();
    assert!(
        is_available,
        "Apple Music web interface should report available"
    );
}

#[tokio::test]
async fn test_lyrics_provider_command_genius() {
    let result = test_lyrics_provider("genius".to_string()).await;
    assert!(
        result.is_ok(),
        "test_lyrics_provider('genius') should return Ok: {:?}",
        result.err()
    );
    let is_available = result.unwrap();
    assert!(
        is_available,
        "Genius should report available"
    );
}

#[tokio::test]
async fn test_lyrics_provider_command_unknown_provider() {
    let result = test_lyrics_provider("nonexistent_mock_provider_xyz".to_string()).await;
    assert!(
        result.is_ok(),
        "test_lyrics_provider for unknown provider should return Ok(false), not panic: {:?}",
        result.err()
    );
    let is_available = result.unwrap();
    assert!(
        !is_available,
        "Unknown provider must report unavailable (false)"
    );
}

#[test]
fn test_lyrics_bridge_cli_test_command_lrclib() {
    let python_cmd = get_python_executable();
    let project_root = get_project_root();
    let script_path = project_root.join("scripts").join("lyrics_bridge.py");

    let output = syncify_tauri_lib::cmd_utils::create_std_command(&python_cmd)
        .arg(&script_path)
        .args(&["test", "--provider", "lrclib"])
        .current_dir(&project_root)
        .output()
        .expect("Failed to execute python lyrics_bridge.py");

    assert!(
        output.status.success(),
        "lyrics_bridge.py test --provider lrclib failed with status {:?}, stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(stdout_str.trim()).expect("stdout must be valid JSON");

    assert_eq!(
        parsed.get("success").and_then(|v| v.as_bool()),
        Some(true),
        "Expected success == true in JSON output: {}",
        stdout_str
    );
    assert_eq!(
        parsed.get("provider").and_then(|v| v.as_str()),
        Some("lrclib"),
        "Expected provider == lrclib: {}",
        stdout_str
    );
    assert_eq!(
        parsed.get("status").and_then(|v| v.as_str()),
        Some("available"),
        "Expected status == available: {}",
        stdout_str
    );
}

#[test]
fn test_lyrics_bridge_cli_test_command_unknown_provider() {
    let python_cmd = get_python_executable();
    let project_root = get_project_root();
    let script_path = project_root.join("scripts").join("lyrics_bridge.py");

    let output = syncify_tauri_lib::cmd_utils::create_std_command(&python_cmd)
        .arg(&script_path)
        .args(&["test", "--provider", "definitely_invalid_provider_abc"])
        .current_dir(&project_root)
        .output()
        .expect("Failed to execute python lyrics_bridge.py");

    assert!(
        !output.status.success(),
        "lyrics_bridge.py test with invalid provider must exit with non-zero code"
    );

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(stdout_str.trim()).expect("stdout must be valid JSON");

    assert_eq!(
        parsed.get("success").and_then(|v| v.as_bool()),
        Some(false),
        "Expected success == false for invalid provider"
    );
    assert!(
        parsed.get("error").is_some(),
        "Expected error message for invalid provider"
    );
}

#[test]
fn test_lyrics_bridge_stderr_error_propagation_on_invalid_arguments() {
    let python_cmd = get_python_executable();
    let project_root = get_project_root();
    let script_path = project_root.join("scripts").join("lyrics_bridge.py");

    // Invoke with unsupported flag to trigger argparse error on stderr
    let output = syncify_tauri_lib::cmd_utils::create_std_command(&python_cmd)
        .arg(&script_path)
        .args(&["test", "--unrecognized-flag-xyz"])
        .current_dir(&project_root)
        .output()
        .expect("Failed to run python command");

    assert!(
        !output.status.success(),
        "Expected failure when passing unrecognized argument"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unrecognized arguments") || stderr.contains("usage:"),
        "stderr should contain argument parsing diagnostics: {}",
        stderr
    );
}
