use std::ffi::OsStr;
use std::path::Path;
use std::time::Duration;
use syncify_tauri_lib::cmd_utils::{
    apply_bundled_tool_env, bundled_tool_candidates, create_python_std_command,
    create_python_tokio_command, create_std_command, resolve_tool, run_command_with_timeout,
    DEFAULT_BRIDGE_TIMEOUT,
};

#[test]
fn test_resolve_tool_ignores_non_bundled_programs() {
    assert!(resolve_tool("python").is_none());
    assert!(resolve_tool("python3").is_none());
    assert!(resolve_tool("sh").is_none());
    assert!(resolve_tool("/usr/bin/ffmpeg").is_none());
}

#[test]
fn test_bundled_tool_candidates_cover_exe_dir_and_bin() {
    let candidates = bundled_tool_candidates("ffmpeg");
    let exe_dir = std::env::current_exe()
        .expect("test executable path")
        .parent()
        .expect("exe directory")
        .to_path_buf();
    let file = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };

    assert!(
        candidates.contains(&exe_dir.join(file)),
        "candidates must include the executable directory: {:?}",
        candidates
    );
    assert!(
        candidates.contains(&exe_dir.join("bin").join(file)),
        "candidates must include bin/ beside the executable: {:?}",
        candidates
    );
    assert!(
        candidates.len() >= 4,
        "candidates must also walk up one level: {:?}",
        candidates
    );
}

#[test]
fn test_resolve_tool_prefers_env_override() {
    let override_path = std::env::temp_dir().join("syncify-cmdutils-test-ffmpeg");
    std::fs::write(&override_path, b"stub").expect("write stub tool");

    std::env::set_var("FFMPEG_PATH", &override_path);
    let resolved = resolve_tool("ffmpeg");
    std::env::remove_var("FFMPEG_PATH");

    assert_eq!(resolved, Some(override_path));
}

#[test]
fn test_apply_bundled_tool_env_publishes_overrides() {
    let override_path = std::env::temp_dir().join("syncify-cmdutils-test-fpcalc");
    std::fs::write(&override_path, b"stub").expect("write stub tool");

    std::env::set_var("FPCALC_PATH", &override_path);
    let mut cmd = create_std_command("python");
    apply_bundled_tool_env(&mut cmd);
    std::env::remove_var("FPCALC_PATH");

    let published = cmd
        .get_envs()
        .find(|(k, _)| *k == OsStr::new("FPCALC_PATH"))
        .and_then(|(_, v)| v)
        .expect("FPCALC_PATH must be published when a tool is resolved");
    assert_eq!(published, override_path.as_os_str());
}

#[test]
fn test_create_std_command_resolves_bundled_tool_path() {
    let override_path = std::env::temp_dir().join("syncify-cmdutils-test-ffprobe");
    std::fs::write(&override_path, b"stub").expect("write stub tool");

    std::env::set_var("FFPROBE_PATH", &override_path);
    let cmd = create_std_command("ffprobe");
    std::env::remove_var("FFPROBE_PATH");

    assert_eq!(
        cmd.get_program(),
        override_path.as_os_str(),
        "the bare tool name must be replaced by the resolved absolute path"
    );
}

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
