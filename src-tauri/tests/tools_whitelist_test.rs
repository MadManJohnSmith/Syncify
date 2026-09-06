//! Tests for Dependency Whitelist Validation (TASK-116 / SEC-020)
//!
//! Validates that only authorized tools ("ffmpeg", "fpcalc") can be installed
//! or ensured via Tauri commands, rejecting invalid tools, injection attempts,
//! and CLI flags immediately without invoking external scripts.

use syncify_tauri_lib::commands::{
    ensure_dependency, install_dependency, validate_tool, ALLOWED_TOOLS,
};

#[test]
fn test_allowed_tools_constant_contents() {
    assert_eq!(ALLOWED_TOOLS, &["ffmpeg", "fpcalc"]);
}

#[test]
fn test_validate_tool_accepts_valid_tools() {
    // Exact matches
    assert_eq!(validate_tool("ffmpeg").unwrap(), "ffmpeg");
    assert_eq!(validate_tool("fpcalc").unwrap(), "fpcalc");

    // Case insensitivity
    assert_eq!(validate_tool("FFMPEG").unwrap(), "ffmpeg");
    assert_eq!(validate_tool("FPCALC").unwrap(), "fpcalc");
    assert_eq!(validate_tool("FpCalc").unwrap(), "fpcalc");
    assert_eq!(validate_tool("FFmpeg").unwrap(), "ffmpeg");

    // Leading and trailing whitespace trimming
    assert_eq!(validate_tool("  ffmpeg  ").unwrap(), "ffmpeg");
    assert_eq!(validate_tool("  FFMPEG  ").unwrap(), "ffmpeg");
    assert_eq!(validate_tool("\t\nfpcalc \r\n").unwrap(), "fpcalc");
}

#[test]
fn test_validate_tool_rejects_unauthorized_tools() {
    let unauthorized_tools = [
        "--help",
        "-v",
        "--version",
        "sh",
        "bash",
        "cmd.exe",
        "powershell",
        "malicious_tool",
        "python",
        "curl",
        "wget",
        "",
        "   ",
        "ffmpeg; rm -rf /",
        "ffmpeg --help",
        "fpcalc && whoami",
    ];

    for &tool in &unauthorized_tools {
        let result = validate_tool(tool);
        assert!(
            result.is_err(),
            "Expected tool '{}' to be rejected, but got Ok",
            tool
        );

        let err_msg = result.unwrap_err();
        let expected_msg = format!(
            "Herramienta no autorizada: '{}'. Herramientas permitidas: {:?}",
            tool, ALLOWED_TOOLS
        );
        assert_eq!(
            err_msg, expected_msg,
            "Error message mismatch for '{}'",
            tool
        );
    }
}

#[tokio::test]
async fn test_install_dependency_rejects_unauthorized_tools_immediately() {
    let unauthorized_tools = [
        "--help",
        "-v",
        "sh",
        "malicious_tool",
        "python3",
        "ffmpeg && calc",
    ];

    for &tool in &unauthorized_tools {
        let result = install_dependency(tool.to_string()).await;
        assert!(
            result.is_err(),
            "Expected install_dependency to reject '{}', but got Ok",
            tool
        );

        let err_msg = result.unwrap_err();
        let expected_msg = format!(
            "Herramienta no autorizada: '{}'. Herramientas permitidas: {:?}",
            tool, ALLOWED_TOOLS
        );
        assert_eq!(
            err_msg, expected_msg,
            "Error message mismatch in install_dependency for '{}'",
            tool
        );
    }
}

#[tokio::test]
async fn test_ensure_dependency_rejects_unauthorized_tools_immediately() {
    let unauthorized_tools = [
        "--help",
        "-v",
        "sh",
        "malicious_tool",
        "python3",
        "fpcalc | cat /etc/passwd",
    ];

    for &tool in &unauthorized_tools {
        let result = ensure_dependency(tool.to_string()).await;
        assert!(
            result.is_err(),
            "Expected ensure_dependency to reject '{}', but got Ok",
            tool
        );

        let err_msg = result.unwrap_err();
        let expected_msg = format!(
            "Herramienta no autorizada: '{}'. Herramientas permitidas: {:?}",
            tool, ALLOWED_TOOLS
        );
        assert_eq!(
            err_msg, expected_msg,
            "Error message mismatch in ensure_dependency for '{}'",
            tool
        );
    }
}

#[tokio::test]
async fn test_ensure_dependency_accepts_valid_tools_with_whitespace_and_casing() {
    // When ffmpeg and fpcalc are available on the system, ensure_dependency succeeds
    // and normalizes the tool name correctly.
    let valid_inputs = ["ffmpeg", "  FFMPEG  ", "fpcalc", "  FpCalc  "];

    for &input in &valid_inputs {
        let result = ensure_dependency(input.to_string()).await;
        // Result should succeed or return valid BridgeResult
        match result {
            Ok(bridge_result) => {
                assert!(
                    bridge_result.success,
                    "Expected success for valid tool '{}'",
                    input
                );
            }
            Err(e) => {
                // If python or dependencies aren't configured in CI, it must NOT be an authorization error
                assert!(
                    !e.contains("Herramienta no autorizada"),
                    "Valid tool '{}' was unexpectedly rejected as unauthorized: {}",
                    input,
                    e
                );
            }
        }
    }
}
