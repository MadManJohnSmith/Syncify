use std::ffi::OsStr;
use std::path::Path;
use std::time::Duration;

/// Creates a `std::process::Command` configured with `CREATE_NO_WINDOW` on Windows
/// to prevent cmd.exe console popups.
#[allow(unused_mut)]
pub fn create_std_command<S: AsRef<OsStr>>(program: S) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd
}

/// Creates a `tokio::process::Command` configured with `CREATE_NO_WINDOW` on Windows
/// to prevent cmd.exe console popups.
#[allow(unused_mut)]
pub fn create_tokio_command<S: AsRef<OsStr>>(program: S) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(program);
    #[cfg(windows)]
    {
        cmd.creation_flags(0x08000000);
    }
    cmd
}

/// Creates a `tokio::process::Command` configured with `PYTHONUNBUFFERED=1` and `PYTHONIOENCODING=utf-8`
#[allow(dead_code)]
pub fn create_python_tokio_command<S: AsRef<OsStr>>(
    program: S,
    scripts_dir: Option<&Path>,
) -> tokio::process::Command {
    let mut cmd = create_tokio_command(program);
    cmd.env("PYTHONUNBUFFERED", "1");
    cmd.env("PYTHONIOENCODING", "utf-8");
    if let Some(dir) = scripts_dir {
        cmd.env("PYTHONPATH", dir);
        cmd.current_dir(dir);
    }
    cmd
}

/// Creates a `std::process::Command` configured with Python unbuffered environment
#[allow(dead_code)]
pub fn create_python_std_command<S: AsRef<OsStr>>(
    program: S,
    scripts_dir: Option<&Path>,
) -> std::process::Command {
    let mut cmd = create_std_command(program);
    cmd.env("PYTHONUNBUFFERED", "1");
    cmd.env("PYTHONIOENCODING", "utf-8");
    if let Some(dir) = scripts_dir {
        cmd.env("PYTHONPATH", dir);
        cmd.current_dir(dir);
    }
    cmd
}

/// Default timeout for asynchronous bridge operations (45 seconds)
#[allow(dead_code)]
pub const DEFAULT_BRIDGE_TIMEOUT: Duration = Duration::from_secs(45);

/// Runs an asynchronous tokio process command with a timeout guard
#[allow(dead_code)]
pub async fn run_command_with_timeout(
    mut cmd: tokio::process::Command,
    timeout_duration: Duration,
) -> Result<std::process::Output, String> {
    match tokio::time::timeout(timeout_duration, cmd.output()).await {
        Ok(res) => res.map_err(|e| format!("Command execution failed: {}", e)),
        Err(_) => Err(format!("Command timed out after {} seconds", timeout_duration.as_secs())),
    }
}
