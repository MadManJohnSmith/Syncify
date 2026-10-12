//! Shared configuration for console-subsystem children and bounded-memory stderr
//! draining. All consumers already depend on `syncify-core-domain`; neither a
//! new dependency edge nor a Tokio dependency is needed here.
use std::io::Read;
use std::process::{Child, Command};
use std::thread::JoinHandle;

/// Win32 `CREATE_NO_WINDOW` for console-subsystem children (`ffmpeg`, `flac`,
/// `reg`, etc.). `tokio::process::Command` callers use `as_std_mut()` to share
/// this exact flag and implementation with blocking callers.
pub const CREATE_NO_WINDOW_FLAG: u32 = 0x0800_0000;

/// Configure a command before spawning; a no-op on non-Windows systems.
pub fn configure_hidden(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW_FLAG);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

/// Fixed stack buffer used to drain stderr. No stderr content is stored,
/// logged or returned: external tools can echo URLs and credentials even in
/// otherwise innocuous messages, so textual redaction cannot be exhaustive.
const STDERR_READ_BUFFER_SIZE: usize = 4 * 1024;

/// Bounded-memory diagnostics for a child. Counters contain no secret text.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StderrDrainReport {
    pub bytes_read: u64,
    pub read_error: bool,
}

/// Take a spawned child's piped stderr and drain it concurrently with the
/// caller's read of stdout. Never join until stdout is drained (or the child
/// has been killed), otherwise a child filling both pipes could deadlock.
///
/// The buffer is 4 KiB, regardless of total output. Past that limit we keep
/// reading and *discard* bytes, so a chatty decoder never blocks on a full
/// pipe. Only the total byte count and read-error bit survive for diagnostics.
pub fn spawn_stderr_drain(child: &mut Child) -> Option<JoinHandle<StderrDrainReport>> {
    let mut stderr = child.stderr.take()?;
    Some(std::thread::spawn(move || {
        let mut report = StderrDrainReport::default();
        let mut buf = [0u8; STDERR_READ_BUFFER_SIZE];
        loop {
            match stderr.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => report.bytes_read = report.bytes_read.saturating_add(n as u64),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    report.read_error = true;
                    break;
                }
            }
        }
        report
    }))
}

/// Join the reader after the child has finished (or been killed). A panic in
/// the reader becomes a read-error report, never a leaked thread or a panic in
/// the caller.
pub fn join_stderr_drain(handle: Option<JoinHandle<StderrDrainReport>>) -> StderrDrainReport {
    handle
        .map(|h| {
            h.join().unwrap_or(StderrDrainReport {
                bytes_read: 0,
                read_error: true,
            })
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_matches_win32_definition() {
        assert_eq!(CREATE_NO_WINDOW_FLAG, 0x0800_0000);
    }

    #[test]
    fn configure_hidden_preserves_command_args() {
        let mut cmd = Command::new("syncify-nonexistent");
        cmd.arg("example");
        configure_hidden(&mut cmd);
        assert_eq!(cmd.get_args().next().unwrap(), "example");
    }

    #[cfg(unix)]
    #[test]
    fn drain_concurrent_large_stderr_while_stdout_is_piped() {
        use std::process::Stdio;

        let mut cmd = Command::new("sh");
        cmd.args([
            "-c",
            "i=0; while [ \"$i\" -lt 20000 ]; do printf 'Authorization: Bearer secret-value\\n' 1>&2; i=$((i+1)); done; printf 'decoded'",
        ]);
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn().expect("spawn shell");
        let drain = spawn_stderr_drain(&mut child);
        let mut stdout = String::new();
        child
            .stdout
            .take()
            .unwrap()
            .read_to_string(&mut stdout)
            .expect("stdout");
        let status = child.wait().expect("wait");
        let report = join_stderr_drain(drain);
        assert!(status.success());
        assert_eq!(stdout, "decoded");
        assert!(report.bytes_read > 4 * 1024);
        assert!(!report.read_error);
        // The report holds counts, not child-supplied text (which could be secret).
        assert!(!format!("{report:?}").contains("secret-value"));
    }

    #[cfg(unix)]
    #[test]
    fn drain_without_stderr_pipe_is_an_empty_report() {
        use std::process::Stdio;

        let mut child = Command::new("true")
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn");
        assert!(spawn_stderr_drain(&mut child).is_none());
        child.wait().unwrap();
        assert_eq!(join_stderr_drain(None), StderrDrainReport::default());
    }
}
