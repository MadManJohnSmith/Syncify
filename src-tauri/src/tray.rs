//! System Tray Module for Syncify (Tauri v2)
//!
//! Handles system tray icon, context menu, window toggling, and desktop notifications.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Runtime, State,
};

// Real commands executed by the tray menu (IN-4) and the sync engine shared
// with the `sync_service` Tauri command, plus the canonical notification
// payload published on `syncify:notification`.
use crate::commands::{
    perform_sync_service_with_emitter, AppNotification, NotificationCategory, NotificationKind,
};

pub const SYNCIFY_TRAY_ID: &str = "syncify-tray";

static CLOSE_TO_TRAY: AtomicBool = AtomicBool::new(true);

/// Sets whether closing the main window minimizes to tray instead of exiting.
pub fn set_close_to_tray(enabled: bool) {
    CLOSE_TO_TRAY.store(enabled, Ordering::Relaxed);
}

/// Returns whether close to tray is enabled.
pub fn is_close_to_tray_enabled() -> bool {
    CLOSE_TO_TRAY.load(Ordering::Relaxed)
}

// ─────────────────────────────────────────────────────────────────────────────
// STARTUP BEHAVIOR PERSISTENCE
// ─────────────────────────────────────────────────────────────────────────────

/// `settings` table keys that the tray reads back on every call.
const KV_CLOSE_TO_TRAY: &str = "close_to_tray";
const KV_START_MINIMIZED: &str = "start_minimized";
const KV_START_ON_BOOT: &str = "start_on_boot";

fn kv_bool(raw: Option<&String>, default: bool) -> bool {
    match raw.map(|v| v.trim().to_ascii_lowercase()) {
        Some(v) if v == "true" || v == "1" => true,
        Some(v) if v == "false" || v == "0" => false,
        _ => default,
    }
}

/// Read the startup preferences back from the database.
///
/// `TraySettings::default()` is only the fallback for keys that were never
/// saved: returning defaults unconditionally made `get_tray_settings` disagree
/// with what the user had actually chosen, and the frontend then re-sent those
/// defaults on every save.
pub async fn load_tray_settings(db: &crate::DbPool) -> TraySettings {
    let defaults = TraySettings::default();
    let keys = vec![
        KV_CLOSE_TO_TRAY.to_string(),
        KV_START_MINIMIZED.to_string(),
        KV_START_ON_BOOT.to_string(),
    ];

    match crate::commands::perform_get_kv_settings(db, keys).await {
        Ok(map) => TraySettings {
            close_to_tray: kv_bool(map.get(KV_CLOSE_TO_TRAY), defaults.close_to_tray),
            start_minimized: kv_bool(map.get(KV_START_MINIMIZED), defaults.start_minimized),
            start_on_boot: kv_bool(map.get(KV_START_ON_BOOT), defaults.start_on_boot),
            ..defaults
        },
        Err(e) => {
            tracing::warn!("Could not read tray settings from the database: {}", e);
            defaults
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// AUTO-START REGISTRATION
// ─────────────────────────────────────────────────────────────────────────────

/// Names of the OS autostart artifacts Syncify writes.
pub const LINUX_AUTOSTART_FILE: &str = "syncify.desktop";
pub const MACOS_AUTOSTART_FILE: &str = "com.syncify.app.plist";
pub const WINDOWS_AUTOSTART_VALUE: &str = "Syncify";
pub const WINDOWS_AUTOSTART_RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

/// Contents of the XDG autostart entry. Kept pure so it can be asserted.
pub fn linux_autostart_desktop_entry(exe: &Path) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Syncify\n\
         Comment=Sync library manager\n\
         Exec={}\n\
         Icon=syncify\n\
         Terminal=false\n\
         X-GNOME-Autostart-enabled=true\n",
        exe.display()
    )
}

/// Contents of the per-user launch agent. Kept pure so it can be asserted.
pub fn macos_autostart_plist(exe: &Path) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
         \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n\
         <dict>\n\
         \x20 <key>Label</key>\n\
         \x20 <string>com.syncify.app</string>\n\
         \x20 <key>ProgramArguments</key>\n\
         \x20 <array>\n\
         \x20   <string>{}</string>\n\
         \x20 </array>\n\
         \x20 <key>RunAtLoad</key>\n\
         \x20 <true/>\n\
         \x20 <key>KeepAlive</key>\n\
         \x20 <false/>\n\
         </dict>\n\
         </plist>\n",
        exe.display()
    )
}

/// Registry payload for the Windows `Run` key: an install path is very often
/// quoted already, so only add quotes when they are missing.
pub fn windows_autostart_command(exe: &Path) -> String {
    let raw = exe.display().to_string();
    if raw.starts_with('"') && raw.ends_with('"') {
        raw
    } else {
        format!("\"{}\"", raw)
    }
}

fn linux_autostart_path() -> Result<PathBuf, String> {
    dirs::config_dir()
        .map(|c| c.join("autostart").join(LINUX_AUTOSTART_FILE))
        .ok_or_else(|| "Could not resolve the user configuration directory".to_string())
}

fn macos_autostart_path() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|h| {
            h.join("Library")
                .join("LaunchAgents")
                .join(MACOS_AUTOSTART_FILE)
        })
        .ok_or_else(|| "Could not resolve the user home directory".to_string())
}

fn write_or_remove(path: &Path, contents: Option<String>) -> Result<(), String> {
    match contents {
        Some(body) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create {}: {}", parent.display(), e))?;
            }
            std::fs::write(path, body)
                .map_err(|e| format!("Failed to write {}: {}", path.display(), e))
        }
        None => match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("Failed to remove {}: {}", path.display(), e)),
        },
    }
}

fn windows_autostart(exe: &Path, enabled: bool) -> Result<(), String> {
    let mut command = std::process::Command::new("reg");
    if enabled {
        command
            .arg("add")
            .arg(WINDOWS_AUTOSTART_RUN_KEY)
            .arg("/v")
            .arg(WINDOWS_AUTOSTART_VALUE)
            .arg("/t")
            .arg("REG_SZ")
            .arg("/d")
            .arg(windows_autostart_command(exe))
            .arg("/f");
    } else {
        command
            .arg("delete")
            .arg(WINDOWS_AUTOSTART_RUN_KEY)
            .arg("/v")
            .arg(WINDOWS_AUTOSTART_VALUE)
            .arg("/f");
    }

    let status = command
        .status()
        .map_err(|e| format!("Failed to run reg.exe: {}", e))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "reg.exe returned {} for the autostart entry",
            status
        ))
    }
}

/// Choose the executable an autostart entry must point at.
///
/// Under an AppImage, `current_exe()` resolves inside the ephemeral mount point
/// (`/tmp/.mount_xxxx`), which no longer exists at the next login; `$APPIMAGE`
/// holds the real, stable path in that case.
pub fn pick_autostart_exe(
    appimage: Option<PathBuf>,
    current_exe: Option<PathBuf>,
) -> Result<PathBuf, String> {
    appimage
        .or(current_exe)
        .ok_or_else(|| "Could not resolve the executable path".to_string())
}

/// Resolve the executable to register, honouring `$APPIMAGE` when present.
pub fn resolve_autostart_exe() -> Result<PathBuf, String> {
    let appimage = std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty());
    pick_autostart_exe(appimage, std::env::current_exe().ok())
}

/// Enable or disable launching Syncify at login for the current user.
pub fn apply_autostart(exe: &Path, enabled: bool) -> Result<(), String> {
    if cfg!(target_os = "windows") {
        windows_autostart(exe, enabled)
    } else if cfg!(target_os = "macos") {
        write_or_remove(
            &macos_autostart_path()?,
            enabled.then(|| macos_autostart_plist(exe)),
        )
    } else {
        write_or_remove(
            &linux_autostart_path()?,
            enabled.then(|| linux_autostart_desktop_entry(exe)),
        )
    }
}

/// Whether the OS currently has a Syncify autostart entry registered.
pub fn is_autostart_enabled() -> bool {
    if cfg!(target_os = "windows") {
        std::process::Command::new("reg")
            .arg("query")
            .arg(WINDOWS_AUTOSTART_RUN_KEY)
            .arg("/v")
            .arg(WINDOWS_AUTOSTART_VALUE)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    } else if cfg!(target_os = "macos") {
        macos_autostart_path().map(|p| p.exists()).unwrap_or(false)
    } else {
        linux_autostart_path().map(|p| p.exists()).unwrap_or(false)
    }
}

/// Tray icon states
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum TrayState {
    #[default]
    Default,
    Downloading,
    Syncing,
    Error,
    Paused,
}

/// Tray and application behavior settings from frontend
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraySettings {
    #[serde(default = "default_true")]
    pub close_to_tray: bool,
    #[serde(default)]
    pub start_minimized: bool,
    #[serde(default)]
    pub start_on_boot: bool,
    #[serde(default = "default_true")]
    pub notifications_enabled: bool,
    #[serde(default = "default_true")]
    pub notify_download_complete: bool,
    #[serde(default = "default_true")]
    pub notify_sync_complete: bool,
    #[serde(default = "default_true")]
    pub notify_errors: bool,
    #[serde(default = "default_true")]
    pub notify_updates: bool,
    #[serde(default)]
    pub notification_sound: bool,
    #[serde(default)]
    pub notify_when_visible: bool,
    #[serde(default = "default_true")]
    pub show_tray_icon: bool,
    #[serde(default = "default_icon_style")]
    pub tray_icon_style: String,
}

fn default_true() -> bool {
    true
}

fn default_icon_style() -> String {
    "color".to_string()
}

impl Default for TraySettings {
    fn default() -> Self {
        Self {
            close_to_tray: true,
            start_minimized: false,
            start_on_boot: false,
            notifications_enabled: true,
            notify_download_complete: true,
            notify_sync_complete: true,
            notify_errors: true,
            notify_updates: true,
            notification_sound: false,
            notify_when_visible: false,
            show_tray_icon: true,
            tray_icon_style: "color".to_string(),
        }
    }
}

/// Build the tray context menu
pub fn build_tray_menu<R: Runtime>(
    app: &AppHandle<R>,
    is_visible: bool,
    is_downloading: bool,
    download_count: usize,
    sync_service: Option<&str>,
) -> Result<Menu<R>, tauri::Error> {
    let menu = Menu::new(app)?;

    let toggle_text = if is_visible {
        "Hide Syncify"
    } else {
        "Show Syncify"
    };
    let toggle_id = if is_visible { "hide" } else { "show" };
    let toggle_item = MenuItem::with_id(app, toggle_id, toggle_text, true, None::<&str>)?;
    menu.append(&toggle_item)?;

    let sep1 = PredefinedMenuItem::separator(app)?;
    menu.append(&sep1)?;

    let status_submenu = Submenu::new(app, "Status", true)?;
    if let Some(service) = sync_service {
        let sync_item = MenuItem::with_id(
            app,
            "status_sync",
            format!("Syncing {}...", service),
            false,
            None::<&str>,
        )?;
        status_submenu.append(&sync_item)?;
    }
    if is_downloading && download_count > 0 {
        let dl_item = MenuItem::with_id(
            app,
            "status_download",
            format!("Downloading {} tracks", download_count),
            false,
            None::<&str>,
        )?;
        status_submenu.append(&dl_item)?;
    }
    if sync_service.is_none() && !is_downloading {
        let idle_item =
            MenuItem::with_id(app, "status_idle", "✓ All caught up", false, None::<&str>)?;
        status_submenu.append(&idle_item)?;
    }
    menu.append(&status_submenu)?;

    let sep2 = PredefinedMenuItem::separator(app)?;
    menu.append(&sep2)?;

    let (pause_id, pause_text) = if is_downloading {
        ("pause_downloads", "Pause All Downloads")
    } else {
        ("resume_downloads", "Resume Downloads")
    };
    let pause_item = MenuItem::with_id(app, pause_id, pause_text, true, None::<&str>)?;
    menu.append(&pause_item)?;

    let sync_item = MenuItem::with_id(app, "sync_all", "Sync All Services", true, None::<&str>)?;
    menu.append(&sync_item)?;

    let sep3 = PredefinedMenuItem::separator(app)?;
    menu.append(&sep3)?;

    let settings_item = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    menu.append(&settings_item)?;

    // IN-4: the former 'Check for Updates' item was removed — it depended on
    // a dead 'tray-action' emit and no update-check command exists in Rust.

    let sep4 = PredefinedMenuItem::separator(app)?;
    menu.append(&sep4)?;

    let quit_item = MenuItem::with_id(app, "quit", "Quit Syncify", true, None::<&str>)?;
    menu.append(&quit_item)?;

    Ok(menu)
}

/// Initializes the system tray icon and menu for Tauri v2
pub fn setup_system_tray<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<TrayIcon<R>, Box<dyn std::error::Error>> {
    let menu = build_tray_menu(app, true, false, 0, None)?;

    let mut builder = TrayIconBuilder::with_id(SYNCIFY_TRAY_ID)
        .menu(&menu)
        .tooltip("Syncify")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            handle_menu_click(app, event.id.as_ref());
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                toggle_main_window(app);
            }
        });

    if let Some(default_icon) = app.default_window_icon() {
        builder = builder.icon(default_icon.clone());
    }

    let tray = builder.build(app)?;
    tracing::info!("System tray initialized successfully");
    Ok(tray)
}

/// Run a tray initialization routine, containing any panic it may raise.
///
/// The `tray-icon` crate loads `libayatana-appindicator3` via FFI at build time
/// and `libappindicator-sys` panics (instead of returning an error) when the
/// shared library is not installed on the system. `catch_unwind` contains that
/// panic so the application can degrade gracefully to a tray-less mode.
///
/// TASK-154: graceful degradation when libayatana-appindicator3 is missing.
pub fn catch_tray_panic<T>(
    f: impl FnOnce() -> Result<T, Box<dyn std::error::Error>>,
) -> Result<T, Box<dyn std::error::Error>> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(result) => result,
        Err(payload) => {
            let msg = if let Some(s) = payload.downcast_ref::<&str>() {
                (*s).to_string()
            } else if let Some(s) = payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown panic while initializing the system tray".to_string()
            };
            Err(format!(
                "system tray initialization panicked (libayatana-appindicator3 missing?): {}",
                msg
            )
            .into())
        }
    }
}

/// Panic-isolated wrapper around [`setup_system_tray`].
///
/// Returns a controlled error (never panics) if the tray cannot be created,
/// including when the underlying appindicator FFI panics.
pub fn setup_system_tray_isolated<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<(), Box<dyn std::error::Error>> {
    match catch_tray_panic(|| setup_system_tray(app).map(|_tray| ())) {
        Ok(()) => {
            let handle = app.clone();
            tauri::async_runtime::spawn(async move {
                apply_startup_window_preference(&handle).await;
            });
            Ok(())
        }
        Err(e) => {
            // Without a tray icon there is no way to bring the window back, so
            // closing it would hide the only surface of a process that keeps
            // running. Fall back to closing the app instead.
            set_close_to_tray(false);
            Err(e)
        }
    }
}

/// Apply the persisted `close_to_tray` and `start_minimized` preferences.
///
/// Runs right after the tray comes up so the window state matches what the user
/// asked for instead of the runtime defaults.
async fn apply_startup_window_preference<R: Runtime>(app: &AppHandle<R>) {
    let settings = load_tray_settings(&app.state::<crate::AppState>().db).await;
    set_close_to_tray(settings.close_to_tray);
    if settings.start_minimized {
        hide_main_window(app);
    }
    // Re-registering an entry that the OS lost (AppImage moved, new machine,
    // user cleaned the autostart folder) keeps the saved promise true without
    // waiting for the user to toggle the switch again.
    if settings.start_on_boot && !is_autostart_enabled() {
        match resolve_autostart_exe().and_then(|exe| apply_autostart(&exe, true)) {
            Ok(()) => tracing::info!("Re-registered the missing autostart entry"),
            Err(e) => tracing::warn!("Could not re-register the autostart entry: {}", e),
        }
    }
}

/// Toggle main window visibility
pub fn toggle_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let is_visible = window.is_visible().unwrap_or(false);
        if is_visible {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
    }
}

/// Show and focus the main window
pub fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Hide the main window
pub fn hide_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

/// Handle context menu clicks
///
/// Every action executes its real implementation directly in Rust (IN-4):
/// the former `tray-action` events had no listener in the UI, which left
/// pause/resume/sync inoperative and has since been removed.
pub fn handle_menu_click<R: Runtime>(app: &AppHandle<R>, id: &str) {
    match id {
        "toggle" => {
            toggle_main_window(app);
        }
        "show" => {
            show_main_window(app);
        }
        "hide" => {
            hide_main_window(app);
        }
        "pause_downloads" => {
            crate::commands::pause_downloads(app.state::<crate::AppState>());
        }
        "resume_downloads" => {
            crate::commands::resume_downloads(app.state::<crate::AppState>());
        }
        "sync_all" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = sync_all_services(&app).await {
                    tracing::error!("Tray 'Sync All Services' failed: {}", e);
                }
            });
        }
        "settings" => {
            show_main_window(app);
        }
        "quit" => {
            app.exit(0);
        }
        _ => {}
    }
}

/// Run a full sync across every active service (tray 'Sync All Services').
///
/// Uses the same engine as the `sync_service` Tauri command
/// (`perform_sync_service_with_emitter`), so progress events reach the UI
/// through the canonical channels. A summary notification is published on
/// the canonical `syncify:notification` channel consumed by the UI toasts.
pub async fn sync_all_services<R: Runtime>(app: &AppHandle<R>) -> Result<usize, String> {
    let db = app.state::<crate::AppState>().db.clone();

    let services: Vec<String> = sqlx::query_scalar(
        r#"SELECT DISTINCT LOWER(s.name)
           FROM accounts a
           JOIN services s ON s.id = a.service_id
           WHERE a.is_active = 1
           ORDER BY 1"#,
    )
    .fetch_all(&db)
    .await
    .map_err(|e| format!("Failed to list active services: {}", e))?;

    if services.is_empty() {
        tracing::info!("Tray 'Sync All Services': no active services configured");
        return Ok(0);
    }

    let mut synced = 0usize;
    let mut failed: Vec<String> = Vec::new();
    for service in &services {
        match perform_sync_service_with_emitter(&db, service, None, None, Some(app)).await {
            Ok(_) => synced += 1,
            Err(e) => {
                tracing::warn!("Tray sync of '{}' failed: {}", service, e);
                failed.push(format!("{}: {}", service, e));
            }
        }
    }

    let (kind, title, message) = if failed.is_empty() {
        (
            NotificationKind::Success,
            "Sync complete".to_string(),
            format!("{} service(s) synchronized.", synced),
        )
    } else if synced == 0 {
        (
            NotificationKind::Error,
            "Sync failed".to_string(),
            failed.join(", "),
        )
    } else {
        (
            NotificationKind::Warning,
            "Sync completed with errors".to_string(),
            format!(
                "{} service(s) synchronized; failed: {}",
                synced,
                failed.join(", ")
            ),
        )
    };

    let notification = AppNotification::new(kind, title, message, NotificationCategory::Sync, None);
    let _ = app.emit("syncify:notification", &notification);

    Ok(synced)
}

/// Update tray icon based on state
pub fn update_tray_icon_state<R: Runtime>(app: &AppHandle<R>, state: TrayState) {
    tracing::debug!("Tray icon state changed to: {:?}", state);

    if let Some(tray) = app.tray_by_id(SYNCIFY_TRAY_ID) {
        let tooltip = match state {
            TrayState::Default => "Syncify - Ready",
            TrayState::Downloading => "Syncify - Downloading",
            TrayState::Syncing => "Syncify - Syncing",
            TrayState::Error => "Syncify - Error occurred",
            TrayState::Paused => "Syncify - Downloads paused",
        };
        let _ = tray.set_tooltip(Some(tooltip));
    }
}

/// Update tray menu with current status
pub fn update_tray_menu<R: Runtime>(
    app: &AppHandle<R>,
    is_visible: bool,
    is_downloading: bool,
    download_count: usize,
    sync_service: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(tray) = app.tray_by_id(SYNCIFY_TRAY_ID) {
        let menu = build_tray_menu(
            app,
            is_visible,
            is_downloading,
            download_count,
            sync_service,
        )?;
        tray.set_menu(Some(menu))?;
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// TAURI COMMANDS
// ─────────────────────────────────────────────────────────────────────────────

/// Tauri command to update tray icon from frontend
#[tauri::command]
pub async fn update_tray_icon<R: Runtime>(
    app: AppHandle<R>,
    state: TrayState,
) -> Result<(), String> {
    update_tray_icon_state(&app, state);
    Ok(())
}

/// Tauri command to update tray status (menu & notifications)
#[tauri::command]
pub async fn update_tray_status<R: Runtime>(
    app: AppHandle<R>,
    is_downloading: bool,
    download_count: usize,
    sync_service: Option<String>,
) -> Result<(), String> {
    let is_visible = app
        .get_webview_window("main")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(true);

    if let Err(e) = update_tray_menu(
        &app,
        is_visible,
        is_downloading,
        download_count,
        sync_service.as_deref(),
    ) {
        tracing::warn!("Failed to update tray menu: {}", e);
    }
    Ok(())
}

/// Tauri command to update tray settings from frontend
#[tauri::command]
pub async fn update_tray_settings<R: Runtime>(
    app: AppHandle<R>,
    settings: TraySettings,
) -> Result<(), String> {
    set_close_to_tray(settings.close_to_tray);

    if let Some(tray) = app.tray_by_id(SYNCIFY_TRAY_ID) {
        if let Err(e) = tray.set_visible(settings.show_tray_icon) {
            tracing::warn!("Failed to set tray visibility: {}", e);
        }
    }

    // `start_on_boot` is persisted by the frontend in the same batch, but the OS
    // entry only exists if it is (re)registered here; a saved preference that
    // was never applied to the system was exactly the dead toggle it looked like.
    let exe = resolve_autostart_exe()?;
    apply_autostart(&exe, settings.start_on_boot)?;

    Ok(())
}

/// Tauri command to retrieve current tray settings
#[tauri::command]
pub async fn get_tray_settings(state: State<'_, crate::AppState>) -> Result<TraySettings, String> {
    let mut settings = load_tray_settings(&state.db).await;
    // The running atomic wins for this one field: it is what the window-close
    // handler actually consults, and the tray-less fallback may have flipped it.
    settings.close_to_tray = is_close_to_tray_enabled();
    Ok(settings)
}

/// Show desktop notification
#[tauri::command]
pub async fn show_notification<R: Runtime>(
    app: AppHandle<R>,
    title: String,
    body: String,
) -> Result<(), String> {
    tracing::info!(title = %title, body = %body, "Notification dispatched");

    let _ = app.emit(
        "tray-notification",
        serde_json::json!({
            "title": title,
            "body": body,
        }),
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::catch_tray_panic;

    /// TASK-154 regression: a panic raised inside the tray initialization FFI
    /// (e.g. `libappindicator-sys` failing to load ayatana-appindicator3)
    /// must be contained and reported as a controlled error, not propagate.
    #[test]
    fn tray_init_panic_is_contained() {
        let result = catch_tray_panic(|| -> Result<(), Box<dyn std::error::Error>> {
            panic!("Failed to load ayatana-appindicator3 or appindicator3 dynamic library");
        });
        let err = result.expect_err("panic must be converted into a controlled error");
        let msg = err.to_string();
        assert!(
            msg.contains("panicked"),
            "error should mention the contained panic, got: {msg}"
        );
        assert!(
            msg.contains("ayatana"),
            "error should carry the original panic message, got: {msg}"
        );
    }

    #[test]
    fn tray_init_success_passes_through() {
        let result = catch_tray_panic(|| -> Result<u8, Box<dyn std::error::Error>> { Ok(7) });
        assert_eq!(result.expect("must be Ok"), 7);
    }

    #[test]
    fn tray_init_regular_error_passes_through() {
        let result = catch_tray_panic(|| -> Result<(), Box<dyn std::error::Error>> {
            Err("menu build failed".into())
        });
        let err = result.expect_err("must propagate the regular error");
        assert_eq!(err.to_string(), "menu build failed");
    }
}
