use std::ffi::OsStr;

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
