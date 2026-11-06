use std::time::Duration;

/// Error de lanzamiento o lectura de un puente Python.
///
/// `Display` reproduce exactamente los mensajes que ya devolvían los tres
/// lanzadores: el refactor no cambia lo que ve el usuario. Los call sites con
/// mensajes propios (auth, settings) mapean las variantes que les importan.
#[derive(Debug)]
pub enum BridgeError {
    /// `cmd.spawn()` falló: el intérprete o el script no se pudieron ejecutar.
    Spawn {
        script: String,
        source: std::io::Error,
    },
    /// Escribir la carga en stdin excedió `STDIN_WRITE_TIMEOUT`.
    StdinTimeout { script: String },
    /// Escribir la carga en stdin falló.
    StdinWrite {
        script: String,
        source: std::io::Error,
    },
    /// `wait_with_output()` falló tras el spawn.
    Wait {
        script: String,
        source: std::io::Error,
    },
    /// El proceso excedió el presupuesto de `CallOptions::timeout`.
    Timeout { script: String, timeout: Duration },
    /// Salida no-cero sin stdout: no hay envelope que interpretar.
    ExitFailure {
        script: String,
        stderr: String,
        status: std::process::ExitStatus,
    },
    /// El stdout no deserializa al tipo esperado (o está vacío).
    Parse {
        script: String,
        stdout: String,
        stderr: String,
    },
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BridgeError::Spawn { script, source } => {
                write!(f, "Failed to run {}: {}", script, source)
            }
            BridgeError::StdinTimeout { script } => {
                write!(f, "Timed out writing to {} stdin", script)
            }
            BridgeError::StdinWrite { script, source } => {
                write!(f, "Failed to write to {} stdin: {}", script, source)
            }
            BridgeError::Wait { script, source } => {
                write!(f, "Failed to wait for {}: {}", script, source)
            }
            BridgeError::Timeout { script, timeout } => {
                write!(
                    f,
                    "Bridge {} timed out after {} seconds",
                    script,
                    timeout.as_secs()
                )
            }
            BridgeError::ExitFailure {
                script,
                stderr,
                status,
            } => {
                let detail = if !stderr.is_empty() {
                    stderr.clone()
                } else {
                    format!("Process exited with status {}", status)
                };
                write!(f, "Bridge {} failed: {}", script, detail)
            }
            BridgeError::Parse {
                script,
                stdout,
                stderr,
            } => {
                if stdout.is_empty() && !stderr.is_empty() {
                    write!(f, "Bridge {} produced no output. Error: {}", script, stderr)
                } else if !stderr.is_empty() {
                    write!(
                        f,
                        "Failed to parse result from {}: {} (stderr: {})",
                        script, stdout, stderr
                    )
                } else {
                    write!(f, "Failed to parse result from {}: {}", script, stdout)
                }
            }
        }
    }
}

impl std::error::Error for BridgeError {}
