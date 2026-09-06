//! System Tray Module for Syncify (Tauri v2)
//!
//! Handles system tray icon, context menu, window toggling, and desktop notifications.

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Runtime,
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

/// Tray icon states
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TrayState {
    Default,
    Downloading,
    Syncing,
    Error,
    Paused,
}

impl Default for TrayState {
    fn default() -> Self {
        Self::Default
    }
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

    let toggle_text = if is_visible { "Hide Syncify" } else { "Show Syncify" };
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
        let idle_item = MenuItem::with_id(
            app,
            "status_idle",
            "✓ All caught up",
            false,
            None::<&str>,
        )?;
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

    let updates_item = MenuItem::with_id(app, "check_updates", "Check for Updates", true, None::<&str>)?;
    menu.append(&updates_item)?;

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
            match event {
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => {
                    let app = tray.app_handle();
                    toggle_main_window(app);
                }
                _ => {}
            }
        });

    if let Some(default_icon) = app.default_window_icon() {
        builder = builder.icon(default_icon.clone());
    }

    let tray = builder.build(app)?;
    tracing::info!("System tray initialized successfully");
    Ok(tray)
}

/// Toggle main window visibility
pub fn toggle_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let is_visible = window.is_visible().unwrap_or(false);
        if is_visible {
            let _ = window.hide();
            let _ = app.emit("tray-action", "hide");
        } else {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
            let _ = app.emit("tray-action", "show");
        }
    }
}

/// Show and focus the main window
pub fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        let _ = app.emit("tray-action", "show");
    }
}

/// Hide the main window
pub fn hide_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
        let _ = app.emit("tray-action", "hide");
    }
}

/// Handle context menu clicks
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
            let _ = app.emit("tray-action", "pause-downloads");
        }
        "resume_downloads" => {
            let _ = app.emit("tray-action", "resume-downloads");
        }
        "sync_all" => {
            let _ = app.emit("tray-action", "sync-all");
        }
        "settings" => {
            show_main_window(app);
            let _ = app.emit("tray-action", "settings");
        }
        "check_updates" => {
            let _ = app.emit("tray-action", "check-updates");
        }
        "quit" => {
            let _ = app.emit("tray-action", "quit");
            app.exit(0);
        }
        _ => {}
    }
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
        let menu = build_tray_menu(app, is_visible, is_downloading, download_count, sync_service)?;
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

/// Alias for backward compatibility if invoked as update_tray_icon_command
#[tauri::command]
pub async fn update_tray_icon_command<R: Runtime>(
    app: AppHandle<R>,
    state: TrayState,
) -> Result<(), String> {
    update_tray_icon(app, state).await
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
    Ok(())
}

/// Tauri command to retrieve current tray settings
#[tauri::command]
pub async fn get_tray_settings() -> Result<TraySettings, String> {
    let mut settings = TraySettings::default();
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
