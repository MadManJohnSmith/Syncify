use std::ffi::OsStr;
use std::path::Path;
use std::time::Duration;
use syncify_tauri_lib::cmd_utils::{
    create_python_std_command, create_python_tokio_command, run_command_with_timeout,
    DEFAULT_BRIDGE_TIMEOUT,
};

#[test]
fn test_create_python_std_command_env_injection() {
    let scripts_dir = Path::new("/fake/scripts/dir");
    let cmd = create_python_std_command("python", Some(scripts_dir));

    let envs: Vec<(&OsStr, Option<&OsStr>)> = cmd.get_envs().collect();

    assert!(
        envs.iter()
            .any(|(k, v)| *k == "PYTHONUNBUFFERED" && *v == Some(OsStr::new("1"))),
        "PYTHONUNBUFFERED=1 must be set in std::process::Command"
    );

    assert!(
        envs.iter()
            .any(|(k, v)| *k == "PYTHONIOENCODING" && *v == Some(OsStr::new("utf-8"))),
        "PYTHONIOENCODING=utf-8 must be set in std::process::Command"
    );

    assert!(
        envs.iter()
            .any(|(k, v)| *k == "PYTHONPATH" && *v == Some(scripts_dir.as_os_str())),
        "PYTHONPATH must be set when scripts_dir is provided"
    );
}

#[test]
fn test_create_python_tokio_command_env_injection() {
    let scripts_dir = Path::new("/fake/scripts/dir");
    let cmd = create_python_tokio_command("python", Some(scripts_dir));

    let envs: Vec<(&OsStr, Option<&OsStr>)> = cmd.as_std().get_envs().collect();

    assert!(
        envs.iter()
            .any(|(k, v)| *k == "PYTHONUNBUFFERED" && *v == Some(OsStr::new("1"))),
        "PYTHONUNBUFFERED=1 must be set in tokio::process::Command"
    );

    assert!(
        envs.iter()
            .any(|(k, v)| *k == "PYTHONIOENCODING" && *v == Some(OsStr::new("utf-8"))),
        "PYTHONIOENCODING=utf-8 must be set in tokio::process::Command"
    );

    assert!(
        envs.iter()
            .any(|(k, v)| *k == "PYTHONPATH" && *v == Some(scripts_dir.as_os_str())),
        "PYTHONPATH must be set when scripts_dir is provided"
    );
}

#[tokio::test]
async fn test_run_command_with_timeout_terminates_cleanly() {
    #[cfg(unix)]
    let mut cmd = tokio::process::Command::new("sleep");
    #[cfg(unix)]
    cmd.arg("5");

    #[cfg(windows)]
    let mut cmd = tokio::process::Command::new("ping");
    #[cfg(windows)]
    cmd.args(["-n", "6", "127.0.0.1"]);

    let start = std::time::Instant::now();
    let res = run_command_with_timeout(cmd, Duration::from_secs(1)).await;
    let elapsed = start.elapsed();

    assert!(res.is_err(), "Command should have timed out");
    let err_msg = res.unwrap_err();
    assert!(
        err_msg.contains("timed out after 1 seconds"),
        "Error message should indicate timeout: {}",
        err_msg
    );
    assert!(
        elapsed < Duration::from_secs(4),
        "Execution should abort near timeout duration, elapsed: {:?}",
        elapsed
    );
}

#[tokio::test]
async fn test_run_command_with_timeout_succeeds_for_fast_command() {
    #[cfg(unix)]
    let mut cmd = tokio::process::Command::new("echo");
    #[cfg(unix)]
    cmd.arg("hello");

    #[cfg(windows)]
    let mut cmd = tokio::process::Command::new("cmd");
    #[cfg(windows)]
    cmd.args(["/C", "echo hello"]);

    let res = run_command_with_timeout(cmd, DEFAULT_BRIDGE_TIMEOUT).await;
    assert!(res.is_ok(), "Fast command should succeed within timeout");
    let output = res.unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.trim().contains("hello"));
}
