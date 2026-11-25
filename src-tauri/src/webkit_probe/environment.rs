//! Selección del backend y fingerprint versionado, sin inicializar GL en el padre.

use std::collections::BTreeSet;
use std::fs;
use std::process::Stdio;
use std::time::Duration;

use serde::Serialize;
use sha2::{Digest, Sha256};

use super::report::{Backend, PROBE_CONTRACT_VERSION};
use crate::cmd_utils::create_tokio_command;

const UNKNOWN: &str = "unknown";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DrmIdentity {
    pub(crate) driver: Option<String>,
    pub(crate) pci_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct DrmDevice {
    pub(crate) node: String,
    pub(crate) driver: String,
    pub(crate) driver_version: String,
    pub(crate) pci_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct GpuDevice {
    pub(crate) driver: String,
    pub(crate) driver_version: String,
    pub(crate) pci_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub(crate) struct RenderingOverrides {
    pub(crate) disable_dmabuf: Option<String>,
    pub(crate) disable_compositing: Option<String>,
    pub(crate) disable_gbm: Option<String>,
    pub(crate) software_rendering: Option<String>,
    pub(crate) mesa_driver_override: Option<String>,
    pub(crate) dri_prime: Option<String>,
    pub(crate) glx_vendor: Option<String>,
    pub(crate) egl_vendor: Option<String>,
    pub(crate) library_path: Option<String>,
    pub(crate) preload: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct EnvironmentFingerprint {
    pub(crate) probe_contract_version: u32,
    pub(crate) backend: Option<Backend>,
    pub(crate) session_type: String,
    pub(crate) desktop: String,
    pub(crate) display: String,
    pub(crate) wayland_display: String,
    pub(crate) gpu_devices: Vec<GpuDevice>,
    pub(crate) drm_devices: Vec<DrmDevice>,
    pub(crate) driver_version: String,
    pub(crate) kernel_release: String,
    pub(crate) webkitgtk_version: String,
    pub(crate) mesa_version: String,
    pub(crate) mesa_source: String,
    pub(crate) rendering_overrides: RenderingOverrides,
}

impl EnvironmentFingerprint {
    pub(crate) fn minimal(backend: Option<Backend>) -> Self {
        Self {
            probe_contract_version: PROBE_CONTRACT_VERSION,
            backend,
            session_type: UNKNOWN.into(),
            desktop: UNKNOWN.into(),
            display: UNKNOWN.into(),
            wayland_display: UNKNOWN.into(),
            gpu_devices: Vec::new(),
            drm_devices: Vec::new(),
            driver_version: UNKNOWN.into(),
            kernel_release: UNKNOWN.into(),
            webkitgtk_version: UNKNOWN.into(),
            mesa_version: UNKNOWN.into(),
            mesa_source: UNKNOWN.into(),
            rendering_overrides: RenderingOverrides::default(),
        }
    }

    pub(crate) fn key(&self) -> String {
        let Ok(bytes) = serde_json::to_vec(self) else {
            return "invalid-fingerprint".into();
        };
        format!("{:x}", Sha256::digest(bytes))
    }

    pub(crate) fn is_complete(&self) -> bool {
        self.backend.is_some()
            && self.session_type != UNKNOWN
            && !self.gpu_devices.is_empty()
            && !self.drm_devices.is_empty()
            && self.driver_version != UNKNOWN
            && self.webkitgtk_version != UNKNOWN
            && self.mesa_version != UNKNOWN
            && self.mesa_source != UNKNOWN
    }
}

fn endpoint_available(backend: Backend, display: Option<&str>, wayland: Option<&str>) -> bool {
    match backend {
        Backend::X11 => display.is_some_and(|value| !value.trim().is_empty()),
        Backend::Wayland => wayland.is_some_and(|value| !value.trim().is_empty()),
    }
}

pub(crate) fn select_backend(
    gdk_backend: Option<&str>,
    session_type: Option<&str>,
    display: Option<&str>,
    wayland_display: Option<&str>,
) -> Option<Backend> {
    if let Some(requested) = gdk_backend.map(str::trim).filter(|value| !value.is_empty()) {
        return requested.split(',').find_map(|value| {
            let backend = Backend::parse(value)?;
            endpoint_available(backend, display, wayland_display).then_some(backend)
        });
    }

    let session = session_type
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if session == Some("tty") {
        return None;
    }
    if let Some(backend) = session.and_then(|value| match value {
        "x11" => Some(Backend::X11),
        "wayland" => Some(Backend::Wayland),
        _ => None,
    }) {
        return endpoint_available(backend, display, wayland_display).then_some(backend);
    }

    match (
        endpoint_available(Backend::X11, display, wayland_display),
        endpoint_available(Backend::Wayland, display, wayland_display),
    ) {
        (true, false) => Some(Backend::X11),
        (false, true) => Some(Backend::Wayland),
        _ => None,
    }
}

pub(crate) fn parse_drm_uevent(value: &str) -> DrmIdentity {
    let mut identity = DrmIdentity::default();
    for line in value.lines() {
        if let Some(driver) = line.strip_prefix("DRIVER=") {
            identity.driver = Some(driver.trim().to_string());
        } else if let Some(pci_id) = line.strip_prefix("PCI_ID=") {
            identity.pci_id = Some(pci_id.trim().to_ascii_uppercase());
        }
    }
    identity
}

fn is_drm_node(name: &str) -> bool {
    ["card", "renderD"].iter().any(|prefix| {
        name.strip_prefix(prefix).is_some_and(|suffix| {
            !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
        })
    })
}

fn nvidia_driver_version() -> Option<String> {
    fs::read_to_string("/proc/driver/nvidia/version")
        .ok()
        .and_then(|value| {
            value.lines().find_map(|line| {
                line.strip_prefix("NVRM version:")
                    .map(str::trim)
                    .filter(|version| !version.is_empty())
                    .map(str::to_string)
            })
        })
}

fn driver_version_for(driver: &str, kernel: &str) -> String {
    if driver == "nvidia" {
        if let Some(version) = nvidia_driver_version() {
            return version;
        }
    }
    let version = read_trimmed(&format!("/sys/module/{driver}/version"));
    if version != UNKNOWN {
        version
    } else if kernel != UNKNOWN {
        format!("kernel:{kernel}")
    } else {
        UNKNOWN.into()
    }
}

fn collect_drm_devices(kernel: &str) -> Vec<DrmDevice> {
    let Ok(entries) = fs::read_dir("/sys/class/drm") else {
        return Vec::new();
    };
    let mut devices = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !is_drm_node(&name) {
            continue;
        }
        let device = entry.path().join("device");
        let uevent = fs::read_to_string(device.join("uevent")).unwrap_or_default();
        let mut identity = parse_drm_uevent(&uevent);
        if identity.driver.is_none() {
            identity.driver = fs::read_link(device.join("driver")).ok().and_then(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            });
        }
        let Some(driver) = identity.driver.filter(|value| !value.is_empty()) else {
            continue;
        };
        let driver_version = driver_version_for(&driver, kernel);
        devices.push(DrmDevice {
            node: name,
            driver,
            driver_version,
            pci_id: identity.pci_id,
        });
    }
    devices.sort_by(|left, right| left.node.cmp(&right.node));
    devices
}

fn collect_gpu_devices(drm: &[DrmDevice]) -> Vec<GpuDevice> {
    let mut identities = BTreeSet::new();
    for device in drm {
        identities.insert((
            device.driver.clone(),
            device.driver_version.clone(),
            device.pci_id.clone(),
        ));
    }
    identities
        .into_iter()
        .map(|(driver, driver_version, pci_id)| GpuDevice {
            driver,
            driver_version,
            pci_id,
        })
        .collect()
}

fn read_trimmed(path: &str) -> String {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| UNKNOWN.into())
}

pub(crate) fn parse_mesa_package_version(value: &str) -> Option<String> {
    let line = value.lines().map(str::trim).find(|line| !line.is_empty())?;
    let version = line
        .strip_prefix("mesa ")
        .and_then(|value| value.split_whitespace().next_back())
        .unwrap_or(line);
    let version = version
        .split_once(':')
        .filter(|(epoch, _)| epoch.bytes().all(|byte| byte.is_ascii_digit()))
        .map(|(_, version)| version)
        .unwrap_or(version);
    let valid = version.chars().all(|character| {
        character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_' | '+')
    });
    (version.starts_with(|character: char| character.is_ascii_digit()) && valid)
        .then(|| version.chars().take(120).collect())
}

async fn command_stdout(program: &str, args: &[&str]) -> Option<String> {
    let mut command = create_tokio_command(program);
    command.args(args);
    command
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(Stdio::piped());
    command.kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(2), command.output())
        .await
        .ok()?
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

async fn collect_mesa_version() -> (String, String) {
    let (debian, arch, rpm) = tokio::join!(
        command_stdout("dpkg-query", &["-W", "-f=${Version}\n", "libgl1-mesa-dri"],),
        command_stdout("pacman", &["-Q", "mesa"]),
        command_stdout("rpm", &["-q", "--qf", "%{VERSION}\n", "mesa-libGL"]),
    );
    if let Some(value) = debian.and_then(|value| parse_mesa_package_version(&value)) {
        return (value, "dpkg".into());
    }
    if let Some(value) = arch.and_then(|value| parse_mesa_package_version(&value)) {
        return (value, "pacman".into());
    }
    if let Some(value) = rpm.and_then(|value| parse_mesa_package_version(&value)) {
        return (value, "rpm".into());
    }
    (UNKNOWN.into(), UNKNOWN.into())
}

fn linked_webkitgtk_version() -> String {
    // Consultamos la biblioteca enlazada, no pkg-config: en un AppImage puede
    // describir el WebKitGTK del host en vez del que carga el proceso.
    let major = unsafe { webkit2gtk::ffi::webkit_get_major_version() };
    let minor = unsafe { webkit2gtk::ffi::webkit_get_minor_version() };
    let micro = unsafe { webkit2gtk::ffi::webkit_get_micro_version() };
    if major == 0 && minor == 0 && micro == 0 {
        UNKNOWN.into()
    } else {
        format!("{major}.{minor}.{micro}")
    }
}

fn short_digest(value: &str) -> String {
    Sha256::digest(value.as_bytes())
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(crate) fn fingerprint_display(value: String) -> String {
    if value.starts_with(':') {
        return value;
    }
    if let Some(unix) = value.strip_prefix("unix:") {
        let screen: String = unix
            .chars()
            .rev()
            .take_while(|character| character.is_ascii_digit())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        return format!(
            "unix-local:{}",
            if screen.is_empty() {
                "unknown"
            } else {
                &screen
            }
        );
    }
    if let Some((host, screen)) = value.rsplit_once(':') {
        if screen.bytes().all(|byte| byte.is_ascii_digit()) && !host.is_empty() {
            return format!("remote:{}:{screen}", short_digest(host));
        }
    }
    "display-redacted".into()
}

pub(crate) fn fingerprint_wayland_display(value: String) -> String {
    match value.rsplit('/').next() {
        Some(name) if !name.is_empty() => format!("socket:{name}"),
        _ => "socket-redacted".into(),
    }
}

fn env_value(name: &str) -> String {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| UNKNOWN.into())
}

fn env_override(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn env_override_digest(name: &str) -> Option<String> {
    env_override(name).map(|value| short_digest(&value))
}

pub(crate) async fn collect(backend: Option<Backend>) -> EnvironmentFingerprint {
    let kernel_release = read_trimmed("/proc/sys/kernel/osrelease");
    let drm_devices = collect_drm_devices(&kernel_release);
    let gpu_devices = collect_gpu_devices(&drm_devices);
    let driver_version = gpu_devices
        .iter()
        .map(|device| format!("{}@{}", device.driver, device.driver_version))
        .collect::<Vec<_>>()
        .join(",");
    let webkitgtk_version = linked_webkitgtk_version();
    let (mesa_version, mesa_source) = collect_mesa_version().await;

    EnvironmentFingerprint {
        probe_contract_version: PROBE_CONTRACT_VERSION,
        backend,
        session_type: env_value("XDG_SESSION_TYPE"),
        desktop: env_value("XDG_CURRENT_DESKTOP"),
        display: fingerprint_display(env_value("DISPLAY")),
        wayland_display: fingerprint_wayland_display(env_value("WAYLAND_DISPLAY")),
        gpu_devices,
        drm_devices,
        driver_version,
        kernel_release,
        webkitgtk_version,
        mesa_version,
        mesa_source,
        rendering_overrides: RenderingOverrides {
            disable_dmabuf: env_override("WEBKIT_DISABLE_DMABUF_RENDERER"),
            disable_compositing: env_override("WEBKIT_DISABLE_COMPOSITING_MODE"),
            disable_gbm: env_override("WEBKIT_DMABUF_RENDERER_DISABLE_GBM"),
            software_rendering: env_override("LIBGL_ALWAYS_SOFTWARE"),
            mesa_driver_override: env_override_digest("MESA_LOADER_DRIVER_OVERRIDE"),
            dri_prime: env_override_digest("DRI_PRIME"),
            glx_vendor: env_override_digest("__GLX_VENDOR_LIBRARY_NAME"),
            egl_vendor: env_override_digest("__EGL_VENDOR_LIBRARY_FILENAMES"),
            library_path: env_override_digest("LD_LIBRARY_PATH"),
            preload: env_override_digest("LD_PRELOAD"),
        },
    }
}
