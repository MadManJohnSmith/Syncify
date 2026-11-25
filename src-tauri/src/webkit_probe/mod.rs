//! Sonda supervisada de composición WebKitGTK.
//!
//! El padre recopila un fingerprint sin inicializar GL y lanza el propio binario
//! en un modo hijo aislado. Solo una captura X11 con contenido y cambio entre
//! frames produce un veredicto positivo; Wayland no demuestra píxeles.

mod child;
mod environment;
pub mod policy;
mod report;
mod supervisor;

#[cfg(test)]
mod tests;

use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures::FutureExt;

use child::run_child;
use environment::EnvironmentFingerprint;
use report::{Assessment, Backend, FrameAssessment, ProbeEvidence, Verdict};
use supervisor::{supervise, Supervision};

const CHILD_FLAG: &str = "--syncify-webkit-probe-child";
const PROBE_TIMEOUT: Duration = Duration::from_secs(8);
const TERM_GRACE: Duration = Duration::from_secs(1);
const MAX_EVIDENCE_BYTES: u64 = 64 * 1024;
const CHILD_ENV_ALLOWLIST: &[&str] = &[
    "PATH",
    "LANG",
    "LC_ALL",
    "TZ",
    "DISPLAY",
    "WAYLAND_DISPLAY",
    "XAUTHORITY",
    "XDG_RUNTIME_DIR",
    "XDG_SESSION_TYPE",
    "XDG_CURRENT_DESKTOP",
    "XDG_DATA_DIRS",
    "XDG_CONFIG_DIRS",
    "DBUS_SESSION_BUS_ADDRESS",
    "GDK_BACKEND",
    "GDK_SCALE",
    "GDK_DPI_SCALE",
    "GTK_IM_MODULE",
    "QT_IM_MODULE",
    "XMODIFIERS",
    "GSETTINGS_SCHEMA_DIR",
    "FONTCONFIG_FILE",
    "FONTCONFIG_PATH",
    "LD_LIBRARY_PATH",
    "LD_PRELOAD",
    "LIBGL_ALWAYS_SOFTWARE",
    "MESA_LOADER_DRIVER_OVERRIDE",
    "__GLX_VENDOR_LIBRARY_NAME",
    "__EGL_VENDOR_LIBRARY_FILENAMES",
    "DRI_PRIME",
    "VK_ICD_FILENAMES",
];

fn configure_child_environment(
    command: &mut tokio::process::Command,
    workspace: &Path,
    backend: Backend,
) {
    // La fixture no necesita credenciales de Syncify ni variables de .env.
    // Heredar solo la sesión gráfica, locale y pila de librerías necesarias.
    command.env_clear();
    for name in CHILD_ENV_ALLOWLIST {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    for (name, value) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("LC_") {
            command.env(name, value);
        }
    }
    command
        .env("HOME", workspace)
        .env("GSETTINGS_BACKEND", "memory")
        .env("NO_AT_BRIDGE", "1")
        .env("RUST_BACKTRACE", "1")
        .env("XDG_DATA_HOME", workspace.join("data"))
        .env("XDG_CONFIG_HOME", workspace.join("config"))
        .env("XDG_CACHE_HOME", workspace.join("cache"))
        .env("TMPDIR", workspace.join("tmp"));
    if std::env::var_os("GDK_BACKEND").is_none() {
        command.env("GDK_BACKEND", backend.as_str());
    }
}

#[derive(Debug, Clone)]
pub struct ChildRequest {
    pub(crate) result_path: PathBuf,
    pub(crate) backend: Backend,
}

#[derive(Debug, Clone)]
pub struct ProbeReport {
    fingerprint: EnvironmentFingerprint,
    backend: Option<Backend>,
    supervision: Supervision,
    evidence: Option<ProbeEvidence>,
    assessment: Assessment,
    diagnostics: Vec<String>,
    elapsed_ms: u128,
}

pub fn child_request_from_args(
    args: impl IntoIterator<Item = OsString>,
) -> Option<Result<ChildRequest, i32>> {
    let mut args = args.into_iter();
    if args.next().as_deref() != Some(OsStr::new(CHILD_FLAG)) {
        return None;
    }
    let Some(result_path) = args.next().filter(|value| !value.is_empty()) else {
        return Some(Err(2));
    };
    let Some(backend) = args
        .next()
        .as_deref()
        .and_then(|value| value.to_str())
        .and_then(Backend::parse)
    else {
        return Some(Err(2));
    };
    if args.next().is_some() {
        return Some(Err(2));
    }
    Some(Ok(ChildRequest {
        result_path: PathBuf::from(result_path),
        backend,
    }))
}

pub fn run_probe_child(request: &ChildRequest) -> i32 {
    run_child(request)
}

struct ProbeWorkspace {
    root: PathBuf,
}

impl ProbeWorkspace {
    fn create_at(root: PathBuf) -> Result<Self, String> {
        let mut builder = fs::DirBuilder::new();
        builder
            .mode(0o700)
            .create(&root)
            .map_err(|error| error.to_string())?;
        for name in ["data", "config", "cache", "tmp"] {
            fs::create_dir(root.join(name)).map_err(|error| error.to_string())?;
        }
        Ok(Self { root })
    }

    async fn new() -> Result<Self, String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        // XDG_RUNTIME_DIR es tmpfs local; /tmp es el fallback Linux. No usar
        // TMPDIR: puede apuntar a NFS/FUSE y bloquear el arranque.
        let base = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .unwrap_or_else(|| PathBuf::from("/tmp"));
        let root = base.join(format!(
            "syncify-webkit-probe-{}-{nonce}",
            std::process::id()
        ));
        let (send, receive) = tokio::sync::oneshot::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let thread_cancelled = Arc::clone(&cancelled);
        let create_root = root.clone();
        let _ = std::thread::Builder::new()
            .name("syncify-webkit-probe-workspace".into())
            .spawn(move || {
                let result = Self::create_at(create_root.clone());
                if thread_cancelled.load(Ordering::Relaxed) {
                    let _ = fs::remove_dir_all(create_root);
                }
                let _ = send.send(result);
            });
        match tokio::time::timeout(Duration::from_secs(1), receive).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err("workspace creation thread failed".into()),
            Err(_) => {
                cancelled.store(true, Ordering::Relaxed);
                Err("workspace creation exceeded 1s".into())
            }
        }
    }
}

impl Drop for ProbeWorkspace {
    fn drop(&mut self) {
        // La limpieza es best-effort y no puede retener el arranque si el
        // sistema de ficheros se cuelga. El hilo muere con el proceso.
        let root = self.root.clone();
        let _ = std::thread::Builder::new()
            .name("syncify-webkit-probe-cleanup".into())
            .spawn(move || {
                let _ = fs::remove_dir_all(root);
            });
    }
}

pub(crate) fn pixel_stats(
    bytes: &[u8],
    width: u32,
    height: u32,
    channels: usize,
    stride: usize,
    red_offset: usize,
    green_offset: usize,
    blue_offset: usize,
) -> report::PixelEvidence {
    let mut evidence = report::PixelEvidence::default();
    if width == 0
        || height == 0
        || channels < 3
        || red_offset >= channels
        || green_offset >= channels
        || blue_offset >= channels
        || stride < width as usize * channels
    {
        return evidence;
    }
    for y in 0..height as usize {
        let row_start = y * stride;
        let row_end = row_start + width as usize * channels;
        if row_end > bytes.len() {
            return evidence;
        }
    }

    evidence.available = true;
    evidence.capture_supported = true;
    evidence.width = width;
    evidence.height = height;
    for y in 0..height as usize {
        for x in 0..width as usize {
            let offset = y * stride + x * channels;
            let red = bytes[offset + red_offset];
            let green = bytes[offset + green_offset];
            let blue = bytes[offset + blue_offset];
            evidence.max_channel = evidence.max_channel.max(red).max(green).max(blue);
            if red > 90 && red.abs_diff(green) > 40 && red.abs_diff(blue) > 40 {
                evidence.red_px += 1;
            }
            if red.max(green).max(blue) - red.min(green).min(blue) > 20 {
                evidence.colored_px += 1;
            }
        }
    }
    evidence
}

pub(crate) fn changed_pixels(
    first: &[u8],
    second: &[u8],
    width: u32,
    height: u32,
    channels: usize,
    second_channels: usize,
    first_stride: usize,
    second_stride: usize,
) -> Option<u64> {
    if width == 0 || height == 0 || channels < 3 || channels != second_channels {
        return None;
    }
    let row_bytes = width as usize * channels;
    if first_stride < row_bytes || second_stride < row_bytes {
        return None;
    }
    for y in 0..height as usize {
        if y * first_stride + row_bytes > first.len()
            || y * second_stride + row_bytes > second.len()
        {
            return None;
        }
    }
    let mut changed = 0;
    for y in 0..height as usize {
        for x in 0..width as usize {
            let first_offset = y * first_stride + x * channels;
            let second_offset = y * second_stride + x * channels;
            if first[first_offset..first_offset + 3] != second[second_offset..second_offset + 3] {
                changed += 1;
            }
        }
    }
    Some(changed)
}

pub(crate) fn parse_evidence_bytes(bytes: &[u8]) -> Option<ProbeEvidence> {
    serde_json::from_slice(bytes).ok()
}

fn read_capped_sync(path: &Path, limit: u64) -> Vec<u8> {
    let Ok(file) = File::open(path) else {
        return Vec::new();
    };
    let mut bytes = Vec::new();
    if file.take(limit).read_to_end(&mut bytes).is_err() {
        return Vec::new();
    }
    bytes
}

async fn read_capped(path: PathBuf, limit: u64) -> Vec<u8> {
    let (send, receive) = tokio::sync::oneshot::channel();
    let spawned = std::thread::Builder::new()
        .name("syncify-webkit-probe-read".into())
        .spawn(move || {
            let _ = send.send(read_capped_sync(&path, limit));
        });
    if spawned.is_err() {
        return Vec::new();
    }
    tokio::time::timeout(Duration::from_millis(500), receive)
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or_default()
}

async fn create_stderr(path: PathBuf) -> Result<File, String> {
    let (send, receive) = tokio::sync::oneshot::channel();
    let spawned = std::thread::Builder::new()
        .name("syncify-webkit-probe-stderr".into())
        .spawn(move || {
            let result = OpenOptions::new()
                .create(true)
                .write(true)
                .mode(0o600)
                .open(path)
                .map_err(|error| error.to_string());
            let _ = send.send(result);
        });
    if spawned.is_err() {
        return Err("stderr creation thread failed".into());
    }
    match tokio::time::timeout(Duration::from_secs(1), receive).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err("stderr creation channel closed".into()),
        Err(_) => Err("stderr open exceeded 1s".into()),
    }
}

fn diagnostics_from_bytes(bytes: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(bytes);
    [
        ("Could not create default EGL display", "egl-display-failed"),
        ("Failed to create GBM buffer", "gbm-buffer-failed"),
        ("Aborting", "webkit-abort"),
        ("Cannot open display", "display-unavailable"),
    ]
    .into_iter()
    .filter_map(|(needle, label)| text.contains(needle).then_some(label.to_string()))
    .collect()
}

async fn diagnostics(path: PathBuf) -> Vec<String> {
    diagnostics_from_bytes(&read_capped(path, MAX_EVIDENCE_BYTES).await)
}

fn assessment(verdict: Verdict, frame: FrameAssessment, reason: &'static str) -> Assessment {
    Assessment {
        verdict,
        frame,
        reason,
    }
}

fn evidence_assessment(
    evidence: &ProbeEvidence,
    backend: Backend,
    fingerprint_complete: bool,
) -> Assessment {
    if !fingerprint_complete {
        return assessment(
            Verdict::Inconclusive,
            FrameAssessment::Unknown,
            "fingerprint-incomplete",
        );
    }
    if evidence.requested_backend != backend {
        return assessment(
            Verdict::Inconclusive,
            FrameAssessment::Unknown,
            "evidence-backend-mismatch",
        );
    }
    report::assess(evidence)
}

fn supervision_failure(backend: Backend, supervision: &Supervision) -> Option<Assessment> {
    if supervision.timed_out {
        return Some(assessment(
            Verdict::Inconclusive,
            FrameAssessment::Unknown,
            "probe-timeout",
        ));
    }
    if supervision.spawn_error.is_some() {
        return Some(assessment(
            Verdict::Inconclusive,
            FrameAssessment::Unknown,
            "probe-spawn-failed",
        ));
    }
    if supervision.cleanup_error.is_some() {
        return Some(assessment(
            Verdict::Inconclusive,
            FrameAssessment::Unknown,
            "probe-cleanup-failed",
        ));
    }
    if supervision.signal.is_some() || supervision.exit_code != Some(0) {
        let (verdict, reason) = if backend == Backend::Wayland {
            (Verdict::Inconclusive, "wayland-probe-process-failed")
        } else if supervision.signal.is_some() {
            (Verdict::Unsupported, "probe-signaled")
        } else {
            (Verdict::Unsupported, "probe-exit-failed")
        };
        return Some(assessment(verdict, FrameAssessment::Unknown, reason));
    }
    None
}

async fn run_probe(backend: Backend, fingerprint: EnvironmentFingerprint) -> ProbeReport {
    let started = Instant::now();
    let workspace = match ProbeWorkspace::new().await {
        Ok(workspace) => workspace,
        Err(error) => {
            return ProbeReport {
                fingerprint,
                backend: Some(backend),
                supervision: Supervision::default(),
                evidence: None,
                assessment: assessment(
                    Verdict::Inconclusive,
                    FrameAssessment::Unknown,
                    "workspace-create-failed",
                ),
                diagnostics: vec![format!("workspace: {error}")],
                elapsed_ms: started.elapsed().as_millis(),
            };
        }
    };
    let result_path = workspace.root.join("result.json");
    let stderr_path = workspace.root.join("stderr.log");
    let stderr_file = match create_stderr(stderr_path.clone()).await {
        Ok(file) => file,
        Err(error) => {
            return ProbeReport {
                fingerprint,
                backend: Some(backend),
                supervision: Supervision::default(),
                evidence: None,
                assessment: assessment(
                    Verdict::Inconclusive,
                    FrameAssessment::Unknown,
                    "stderr-create-failed",
                ),
                diagnostics: vec![format!("stderr: {error}")],
                elapsed_ms: started.elapsed().as_millis(),
            };
        }
    };
    let executable = match std::env::current_exe() {
        Ok(executable) => executable,
        Err(error) => {
            return ProbeReport {
                fingerprint,
                backend: Some(backend),
                supervision: Supervision::default(),
                evidence: None,
                assessment: assessment(
                    Verdict::Inconclusive,
                    FrameAssessment::Unknown,
                    "current-exe-failed",
                ),
                diagnostics: vec![format!("current_exe: {error}")],
                elapsed_ms: started.elapsed().as_millis(),
            };
        }
    };

    let mut command = crate::cmd_utils::create_tokio_command(executable);
    configure_child_environment(&mut command, &workspace.root, backend);
    command
        .arg(CHILD_FLAG)
        .arg(&result_path)
        .arg(backend.as_str())
        .current_dir(&workspace.root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr_file));

    let supervision = supervise(command, PROBE_TIMEOUT, TERM_GRACE).await;
    let (diagnostics, result_bytes) = tokio::join!(
        diagnostics(stderr_path),
        read_capped(result_path, MAX_EVIDENCE_BYTES),
    );
    let evidence = if supervision.exit_code == Some(0)
        && supervision.signal.is_none()
        && !supervision.timed_out
        && supervision.spawn_error.is_none()
        && supervision.cleanup_error.is_none()
    {
        parse_evidence_bytes(&result_bytes)
    } else {
        None
    };

    let assessment = supervision_failure(backend, &supervision).unwrap_or_else(|| {
        if let Some(evidence) = &evidence {
            evidence_assessment(evidence, backend, fingerprint.is_complete())
        } else {
            assessment(
                Verdict::Inconclusive,
                FrameAssessment::Unknown,
                "probe-evidence-invalid",
            )
        }
    });

    ProbeReport {
        fingerprint,
        backend: Some(backend),
        supervision,
        evidence,
        assessment,
        diagnostics,
        elapsed_ms: started.elapsed().as_millis(),
    }
}

/// Una sonda que falla, aborta o se cuelga nunca debe tumbar el arranque.
pub async fn probe_startup() -> ProbeReport {
    let Some(backend) = environment::select_backend(
        std::env::var("GDK_BACKEND").ok().as_deref(),
        std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
        std::env::var("DISPLAY").ok().as_deref(),
        std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
    ) else {
        return ProbeReport {
            fingerprint: EnvironmentFingerprint::minimal(None),
            backend: None,
            supervision: Supervision::default(),
            evidence: None,
            assessment: assessment(
                Verdict::Skipped,
                FrameAssessment::Unknown,
                "no-graphical-session",
            ),
            diagnostics: Vec::new(),
            elapsed_ms: 0,
        };
    };
    let collect = std::panic::AssertUnwindSafe(environment::collect(Some(backend))).catch_unwind();
    let fingerprint = match tokio::time::timeout(Duration::from_secs(3), collect).await {
        Ok(Ok(fingerprint)) => fingerprint,
        Ok(Err(_)) | Err(_) => EnvironmentFingerprint::minimal(Some(backend)),
    };
    let fingerprint_for_panic = fingerprint.clone();
    match std::panic::AssertUnwindSafe(run_probe(backend, fingerprint))
        .catch_unwind()
        .await
    {
        Ok(report) => report,
        Err(_) => ProbeReport {
            fingerprint: fingerprint_for_panic,
            backend: Some(backend),
            supervision: Supervision::default(),
            evidence: None,
            assessment: assessment(
                Verdict::Inconclusive,
                FrameAssessment::Unknown,
                "probe-panicked",
            ),
            diagnostics: Vec::new(),
            elapsed_ms: 0,
        },
    }
}

pub fn log_report(report: &ProbeReport) {
    let gpu = report
        .fingerprint
        .gpu_devices
        .iter()
        .map(|device| match &device.pci_id {
            Some(pci_id) => format!("{}@{}@{pci_id}", device.driver, device.driver_version),
            None => format!("{}@{}", device.driver, device.driver_version),
        })
        .collect::<Vec<_>>()
        .join(",");
    let drm = report
        .fingerprint
        .drm_devices
        .iter()
        .map(|device| format!("{}:{}", device.node, device.driver))
        .collect::<Vec<_>>()
        .join(",");
    let evidence = report.evidence.as_ref();
    macro_rules! log_probe {
        ($level:ident, $message:expr) => {
            tracing::$level!(
                fingerprint = %report.fingerprint.key(),
                fingerprint_complete = report.fingerprint.is_complete(),
                backend = report.backend.map(Backend::as_str).unwrap_or("none"),
                session_type = %report.fingerprint.session_type,
                desktop = %report.fingerprint.desktop,
                gpu = %gpu,
                drm = %drm,
                driver_version = %report.fingerprint.driver_version,
                kernel = %report.fingerprint.kernel_release,
                webkitgtk = %report.fingerprint.webkitgtk_version,
                mesa = %report.fingerprint.mesa_version,
                mesa_source = %report.fingerprint.mesa_source,
                verdict = report.assessment.verdict.as_str(),
                frame = report.assessment.frame.as_str(),
                reason = report.assessment.reason,
                elapsed_ms = report.elapsed_ms,
                exit_code = report.supervision.exit_code.unwrap_or(-1),
                signal = report.supervision.signal.unwrap_or(-1),
                timed_out = report.supervision.timed_out,
                kill_escalated = report.supervision.kill_escalated,
                frames_delta = evidence.map(|value| value.frames_delta).unwrap_or(0),
                paints_delta = evidence.map(|value| value.paints_delta).unwrap_or(0),
                window_red_px = evidence.map(|value| value.window.red_px).unwrap_or(0),
                window_changed_px = evidence.and_then(|value| value.window.changed_px).unwrap_or(0),
                snapshot_red_px = evidence.map(|value| value.snapshot.red_px).unwrap_or(0),
                diagnostics = %report.diagnostics.join(","),
                $message
            )
        };
    }
    match report.assessment.verdict {
        Verdict::Inconclusive => log_probe!(
            warn,
            "WebKitGTK startup probe inconclusive; compatible route retained"
        ),
        Verdict::Supported | Verdict::Unsupported => {
            log_probe!(info, "WebKitGTK startup probe completed")
        }
        Verdict::Skipped => log_probe!(debug, "WebKitGTK startup probe skipped"),
    }
}
