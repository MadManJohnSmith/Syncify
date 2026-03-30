//! System Tray Module for Syncify
//!
//! Handles system tray icon, menu, and notifications.

use tauri::{
    AppHandle, CustomMenuItem, Manager, Runtime, SystemTray, SystemTrayEvent, SystemTrayMenu,
    SystemTrayMenuItem, SystemTraySubmenu,
};

/// Tray icon states
#[derive(Debug, Clone, Copy, PartialEq, serde::Deserialize)]
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

/// Create the system tray with initial menu
pub fn create_system_tray() -> SystemTray {
    let menu = build_tray_menu(true, false, 0, None);
    SystemTray::new().with_menu(menu)
}

/// Build the tray context menu
fn build_tray_menu(
    is_visible: bool,
    is_downloading: bool,
    download_count: usize,
    sync_service: Option<&str>,
) -> SystemTrayMenu {
    // Show/Hide toggle
    let toggle_visibility = if is_visible {
        CustomMenuItem::new("hide", "Hide Syncify")
    } else {
        CustomMenuItem::new("show", "Show Syncify")
    };

    // Status items (disabled, informational)
    let status_menu = {
        let mut menu = SystemTrayMenu::new();

        if let Some(service) = sync_service {
            menu = menu.add_item(
                CustomMenuItem::new("status_sync", format!("Syncing {}...", service)).disabled(),
            );
        }

        if is_downloading && download_count > 0 {
            menu = menu.add_item(
                CustomMenuItem::new("status_download", format!("Downloading {} tracks", download_count))
                    .disabled(),
            );
        }

        if sync_service.is_none() && !is_downloading {
            menu = menu.add_item(CustomMenuItem::new("status_idle", "✓ All caught up").disabled());
        }

        menu
    };

    // Quick actions
    let pause_resume = if is_downloading {
        CustomMenuItem::new("pause_downloads", "Pause All Downloads")
    } else {
        CustomMenuItem::new("resume_downloads", "Resume Downloads")
    };

    // Build full menu
    SystemTrayMenu::new()
        .add_item(toggle_visibility)
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_submenu(SystemTraySubmenu::new("Status", status_menu))
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(pause_resume)
        .add_item(CustomMenuItem::new("sync_all", "Sync All Services"))
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(CustomMenuItem::new("settings", "Settings"))
        .add_item(CustomMenuItem::new("check_updates", "Check for Updates"))
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(CustomMenuItem::new("quit", "Quit Syncify"))
}

/// Handle tray events
pub fn handle_tray_event<R: Runtime>(app: &AppHandle<R>, event: SystemTrayEvent) {
    match event {
        SystemTrayEvent::LeftClick { .. } => {
            // Toggle window visibility
            if let Some(window) = app.get_window("main") {
                if window.is_visible().unwrap_or(false) {
                    let _ = window.hide();
                    // Emit event to frontend
                    let _ = app.emit_all("tray-action", "hide");
                } else {
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = app.emit_all("tray-action", "show");
                }
            }
        }
        SystemTrayEvent::MenuItemClick { id, .. } => {
            handle_menu_click(app, &id);
        }
        _ => {}
    }
}

/// Handle menu item clicks
fn handle_menu_click<R: Runtime>(app: &AppHandle<R>, id: &str) {
    match id {
        "show" => {
            if let Some(window) = app.get_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
            let _ = app.emit_all("tray-action", "show");
        }
        "hide" => {
            if let Some(window) = app.get_window("main") {
                let _ = window.hide();
            }
            let _ = app.emit_all("tray-action", "hide");
        }
        "pause_downloads" => {
            let _ = app.emit_all("tray-action", "pause-downloads");
        }
        "resume_downloads" => {
            let _ = app.emit_all("tray-action", "resume-downloads");
        }
        "sync_all" => {
            let _ = app.emit_all("tray-action", "sync-all");
        }
        "settings" => {
            // Show window and navigate to settings
            if let Some(window) = app.get_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
            let _ = app.emit_all("tray-action", "settings");
        }
        "check_updates" => {
            let _ = app.emit_all("tray-action", "check-updates");
        }
        "quit" => {
            let _ = app.emit_all("tray-action", "quit");
            std::process::exit(0);
        }
        _ => {}
    }
}

/// Update tray icon based on state
pub fn update_tray_icon<R: Runtime>(app: &AppHandle<R>, state: TrayState) {
    // In a real implementation, we would switch icons here
    // For now, we'll just log the state change
    tracing::debug!("Tray icon state changed to: {:?}", state);

    // Icon paths would be:
    // - icons/tray-default.png
    // - icons/tray-downloading.png (animated or progress)
    // - icons/tray-syncing.png (with sync arrows)
    // - icons/tray-error.png (with red dot)
    // - icons/tray-paused.png (with pause indicator)

    // Example (if we had the icons):
    // let icon_path = match state {
    //     TrayState::Default => "icons/tray-default.png",
    //     TrayState::Downloading => "icons/tray-downloading.png",
    //     TrayState::Syncing => "icons/tray-syncing.png",
    //     TrayState::Error => "icons/tray-error.png",
    //     TrayState::Paused => "icons/tray-paused.png",
    // };
    // if let Ok(icon) = tauri::Icon::File(icon_path.into()) {
    //     app.tray_handle().set_icon(icon).ok();
    // }
}

/// Update tray menu with current status
pub fn update_tray_menu<R: Runtime>(
    app: &AppHandle<R>,
    is_visible: bool,
    is_downloading: bool,
    download_count: usize,
    sync_service: Option<&str>,
) {
    let menu = build_tray_menu(is_visible, is_downloading, download_count, sync_service);
    if let Err(e) = app.tray_handle().set_menu(menu) {
        tracing::warn!("Failed to update tray menu: {}", e);
    }
}

/// Tauri command to update tray icon from frontend
#[tauri::command]
pub async fn update_tray_icon_command<R: Runtime>(
    app: AppHandle<R>,
    state: TrayState,
) -> Result<(), String> {
    update_tray_icon(&app, state);
    Ok(())
}

/// Tauri command to update tray status
#[tauri::command]
pub async fn update_tray_status<R: Runtime>(
    app: AppHandle<R>,
    is_downloading: bool,
    download_count: usize,
    sync_service: Option<String>,
) -> Result<(), String> {
    // Get window visibility
    let is_visible = app
        .get_window("main")
        .map(|w| w.is_visible().unwrap_or(true))
        .unwrap_or(true);

    update_tray_menu(
        &app,
        is_visible,
        is_downloading,
        download_count,
        sync_service.as_deref(),
    );
    Ok(())
}

/// Show desktop notification
#[tauri::command]
pub async fn show_notification(title: String, body: String) -> Result<(), String> {
    // Using tauri-plugin-notification or native OS notifications
    tracing::info!("Notification: {} - {}", title, body);

    #[cfg(target_os = "windows")]
    {
        // Windows toast notification would go here
        // For now, just log
    }

    Ok(())
}
