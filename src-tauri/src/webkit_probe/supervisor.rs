//! Supervisor Unix: timeout, TERM con gracia, KILL y limpieza del grupo entero.

use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, ExitStatus};
use std::time::Duration;

use tokio::process::Command;

const POLL_INTERVAL: Duration = Duration::from_millis(10);
const KILL_REAP_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Default)]
pub(crate) struct Supervision {
    pub(crate) exit_code: Option<i32>,
    pub(crate) signal: Option<i32>,
    pub(crate) timed_out: bool,
    pub(crate) kill_escalated: bool,
    pub(crate) spawn_error: Option<String>,
    pub(crate) cleanup_error: Option<String>,
}

struct ProcessGroup(i32);

impl ProcessGroup {
    fn signal(&self, signal: i32) -> Result<(), String> {
        // Un PGID negativo evita tocar el grupo del supervisor o de la app.
        let result = unsafe { libc::kill(-self.0, signal) };
        if result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
            Ok(())
        } else {
            Err(format!(
                "killpg({}, {signal}) failed: {}",
                self.0,
                std::io::Error::last_os_error()
            ))
        }
    }

    fn sweep(&self) -> Result<(), String> {
        self.signal(libc::SIGKILL)
    }
}

impl Drop for ProcessGroup {
    fn drop(&mut self) {
        let _ = self.sweep();
    }
}

fn record_status(outcome: &mut Supervision, status: ExitStatus) {
    outcome.exit_code = status.code();
    outcome.signal = status.signal();
}

async fn wait_for_exit(child: &mut Child, budget: Duration) -> Result<Option<ExitStatus>, String> {
    let deadline = std::time::Instant::now() + budget;
    loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            return Ok(Some(status));
        }
        let now = std::time::Instant::now();
        if now >= deadline {
            return Ok(None);
        }
        tokio::time::sleep((deadline - now).min(POLL_INTERVAL)).await;
    }
}

pub(crate) async fn supervise(command: Command, timeout: Duration, grace: Duration) -> Supervision {
    // StdChild permite esperar en un hilo propio si el kernel no reap ni tras
    // KILL, sin depender de la vida del runtime de Tokio/Tauri.
    let mut command: std::process::Command = command.into_std();
    command.process_group(0);

    let mut outcome = Supervision::default();
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            outcome.spawn_error = Some(error.to_string());
            return outcome;
        }
    };
    let group = ProcessGroup(child.id() as i32);

    match wait_for_exit(&mut child, timeout).await {
        Ok(Some(status)) => record_status(&mut outcome, status),
        Ok(None) => {
            outcome.timed_out = true;
            let _ = group.signal(libc::SIGTERM);
            match wait_for_exit(&mut child, grace).await {
                Ok(Some(status)) => record_status(&mut outcome, status),
                Ok(None) => {
                    outcome.kill_escalated = true;
                    let _ = group.signal(libc::SIGKILL);
                    match wait_for_exit(&mut child, KILL_REAP_TIMEOUT).await {
                        Ok(Some(status)) => record_status(&mut outcome, status),
                        Ok(None) => {
                            outcome.cleanup_error =
                                Some("reaping after KILL exceeded 500ms".into());
                            if let Err(error) = std::thread::Builder::new()
                                .name("syncify-webkit-probe-reaper".into())
                                .spawn(move || {
                                    let _ = child.wait();
                                })
                            {
                                outcome.cleanup_error = Some(format!(
                                    "reaping after KILL exceeded 500ms and reaper failed: {error}"
                                ));
                            }
                        }
                        Err(error) => {
                            outcome.cleanup_error = Some(format!("wait after KILL failed: {error}"))
                        }
                    }
                }
                Err(error) => {
                    outcome.cleanup_error = Some(format!("wait after TERM failed: {error}"))
                }
            }
        }
        Err(error) => outcome.cleanup_error = Some(format!("wait failed: {error}")),
    }

    if let Err(error) = group.sweep() {
        outcome.cleanup_error = Some(error);
    }
    outcome
}
