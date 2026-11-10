use std::process::Stdio;
use std::time::Duration;

use serde::de::DeserializeOwned;

use super::envelope::BridgeEnvelope;
use super::error::BridgeError;

/// Presupuesto de una llamada a un puente.
///
/// El timeout es obligatorio a propósito: cada call site declara cuánto puede
/// tardar su puente y ningún sitio nuevo hereda en silencio un valor pensado
/// para otro. `stdin` solo lo usa el refresco de credenciales de auth.
pub struct CallOptions<'a> {
    pub timeout: Duration,
    pub stdin: Option<&'a [u8]>,
}

impl<'a> CallOptions<'a> {
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            timeout,
            stdin: None,
        }
    }
}

/// Presupuesto para escribir la carga de stdin antes de cerrar el pipe.
pub const STDIN_WRITE_TIMEOUT: Duration = Duration::from_secs(10);

/// S222/Q6: a partir de aquí una captura deja `warn` (que sobrevive a
/// `RUST_LOG=info`, el nivel de `prueba-limpia.sh`). Una captura normal
/// (arranque de Python + HTTP del proveedor) va por debajo del segundo; varios
/// segundos por captura es la firma de tormenta que congelaba el runtime.
const SLOW_CAPTURE_WARN_MS: u64 = 2_000;

/// Salida cruda de un puente: para los call sites que interpretan su propio
/// payload (auth tiene su envelope histórico con campos extra).
#[derive(Debug)]
pub struct RawBridgeOutput {
    pub stdout: String,
    pub stderr: String,
    pub status: std::process::ExitStatus,
}

impl RawBridgeOutput {
    pub fn exit_ok(&self) -> bool {
        self.status.success()
    }

    pub fn exit_code(&self) -> Option<i32> {
        self.status.code()
    }
}

/// Cliente único para lanzar los puentes Python.
///
/// Resuelve intérprete, directorio de scripts y entorno por llamada, igual que
/// hacían los tres lanzadores anteriores, y fija el entorno Python compartido
/// (UTF-8, unbuffered, PYTHONPATH) con `cmd_utils::create_python_tokio_command`.
/// `script` es un nombre dentro de `scripts/`; una ruta absoluta también vale
/// (los tests escriben fixtures en un directorio temporal).
pub struct BridgeClient;

impl BridgeClient {
    /// Lanza el puente y devuelve su salida decodificada, sin interpretarla.
    pub async fn capture(
        script: &str,
        args: &[&str],
        options: CallOptions<'_>,
    ) -> Result<RawBridgeOutput, BridgeError> {
        let project_root = crate::commands::get_project_root();
        let python_cmd = crate::commands::get_python_executable();
        let scripts_dir = crate::cmd_utils::find_scripts_dir(&project_root);
        let script_path = scripts_dir.join(script);

        tracing::debug!(
            "Running bridge: {} {:?} (cwd: {:?})",
            script,
            args,
            project_root
        );
        let capture_start = std::time::Instant::now();

        let mut cmd =
            crate::cmd_utils::create_python_tokio_command(&python_cmd, Some(scripts_dir.as_path()));
        cmd.arg(&script_path);
        for arg in args {
            cmd.arg(arg);
        }
        // El contrato anterior corría con cwd=project_root; el helper fija
        // cwd=scripts para PYTHONPATH: se sobreescribe aquí (el último
        // current_dir gana) para no cambiar la resolución de rutas.
        cmd.current_dir(&project_root);
        cmd.kill_on_drop(true);
        match options.stdin {
            Some(_) => {
                cmd.stdin(Stdio::piped());
            }
            None => {
                cmd.stdin(Stdio::null());
            }
        }
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| BridgeError::Spawn {
            script: script.to_string(),
            source: e,
        })?;

        if let Some(payload) = options.stdin {
            let Some(mut stdin) = child.stdin.take() else {
                return Err(BridgeError::StdinWrite {
                    script: script.to_string(),
                    source: std::io::Error::other("stdin pipe unavailable"),
                });
            };
            use tokio::io::AsyncWriteExt;
            tokio::time::timeout(STDIN_WRITE_TIMEOUT, stdin.write_all(payload))
                .await
                .map_err(|_| BridgeError::StdinTimeout {
                    script: script.to_string(),
                })?
                .map_err(|e| BridgeError::StdinWrite {
                    script: script.to_string(),
                    source: e,
                })?;
            let _ = stdin.flush().await;
            drop(stdin); // EOF para el hijo
        }

        let output = tokio::time::timeout(options.timeout, child.wait_with_output())
            .await
            .map_err(|_| BridgeError::Timeout {
                script: script.to_string(),
                timeout: options.timeout,
            })?
            .map_err(|e| BridgeError::Wait {
                script: script.to_string(),
                source: e,
            })?;

        let capture_elapsed_ms = capture_start.elapsed().as_millis() as u64;
        tracing::debug!(
            script = %script,
            elapsed_ms = capture_elapsed_ms,
            "S222: bridge capture completado"
        );
        if capture_elapsed_ms >= SLOW_CAPTURE_WARN_MS {
            tracing::warn!(
                script = %script,
                elapsed_ms = capture_elapsed_ms,
                "S222: bridge capture lento"
            );
        }
        Ok(RawBridgeOutput {
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            status: output.status,
        })
    }

    /// Lanza el puente y deserializa el envelope `{success, data, error}`.
    ///
    /// Semántica heredada del lanzador genérico, byte a byte: se parsea stdout
    /// aunque el exit sea distinto de cero (un fallo de dominio llega con su
    /// envelope y exit 1); solo stdout vacío con exit fallido es `ExitFailure`.
    pub async fn call<T: DeserializeOwned>(
        script: &str,
        args: &[&str],
        options: CallOptions<'_>,
    ) -> Result<BridgeEnvelope<T>, BridgeError> {
        let raw = Self::capture(script, args, options).await?;

        if !raw.stderr.is_empty() {
            tracing::warn!("Bridge {} stderr: {}", script, raw.stderr);
        }

        let trimmed_stdout = raw.stdout.trim();
        let trimmed_stderr = raw.stderr.trim();

        if !raw.exit_ok() && trimmed_stdout.is_empty() {
            return Err(BridgeError::ExitFailure {
                script: script.to_string(),
                stderr: trimmed_stderr.to_string(),
                status: raw.status,
            });
        }

        serde_json::from_str::<BridgeEnvelope<T>>(trimmed_stdout).map_err(|_| BridgeError::Parse {
            script: script.to_string(),
            stdout: trimmed_stdout.to_string(),
            stderr: trimmed_stderr.to_string(),
        })
    }
}
