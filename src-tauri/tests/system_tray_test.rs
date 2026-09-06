//! Tests for System Tray and Desktop Notification Contract (TASK-120)

use serde_json::json;
use syncify_tauri_lib::tray::{
    build_tray_menu, is_close_to_tray_enabled, set_close_to_tray,
    TraySettings, TrayState, SYNCIFY_TRAY_ID,
};

#[test]
fn test_tray_state_serde() {
    let states = [
        (TrayState::Default, "\"default\""),
        (TrayState::Downloading, "\"downloading\""),
        (TrayState::Syncing, "\"syncing\""),
        (TrayState::Error, "\"error\""),
        (TrayState::Paused, "\"paused\""),
    ];

    for (state, expected_json) in states {
        let serialized = serde_json::to_string(&state).expect("serialize tray state");
        assert_eq!(serialized, expected_json);

        let deserialized: TrayState =
            serde_json::from_str(expected_json).expect("deserialize tray state");
        assert_eq!(deserialized, state);
    }
}

#[test]
fn test_tray_settings_default() {
    let settings = TraySettings::default();
    assert!(settings.close_to_tray);
    assert!(!settings.start_minimized);
    assert!(!settings.start_on_boot);
    assert!(settings.notifications_enabled);
    assert!(settings.notify_download_complete);
    assert!(settings.notify_sync_complete);
    assert!(settings.notify_errors);
    assert!(settings.notify_updates);
    assert!(!settings.notification_sound);
    assert!(!settings.notify_when_visible);
    assert!(settings.show_tray_icon);
    assert_eq!(settings.tray_icon_style, "color");
}

#[test]
fn test_tray_settings_camel_case_json_deserialization() {
    let json_payload = json!({
        "closeToTray": false,
        "startMinimized": true,
        "startOnBoot": true,
        "notificationsEnabled": true,
        "notifyDownloadComplete": false,
        "notifySyncComplete": true,
        "notifyErrors": true,
        "notifyUpdates": false,
        "notificationSound": true,
        "notifyWhenVisible": true,
        "showTrayIcon": false,
        "trayIconStyle": "white"
    });

    let deserialized: TraySettings =
        serde_json::from_value(json_payload).expect("deserialize TraySettings from camelCase JSON");

    assert!(!deserialized.close_to_tray);
    assert!(deserialized.start_minimized);
    assert!(deserialized.start_on_boot);
    assert!(deserialized.notifications_enabled);
    assert!(!deserialized.notify_download_complete);
    assert!(deserialized.notify_sync_complete);
    assert!(deserialized.notify_errors);
    assert!(!deserialized.notify_updates);
    assert!(deserialized.notification_sound);
    assert!(deserialized.notify_when_visible);
    assert!(!deserialized.show_tray_icon);
    assert_eq!(deserialized.tray_icon_style, "white");
}

#[test]
fn test_tray_settings_partial_json_uses_defaults() {
    let partial_json = json!({
        "closeToTray": false
    });

    let deserialized: TraySettings =
        serde_json::from_value(partial_json).expect("deserialize partial TraySettings");

    assert!(!deserialized.close_to_tray);
    // Unspecified fields should default properly
    assert!(deserialized.show_tray_icon);
    assert_eq!(deserialized.tray_icon_style, "color");
    assert!(deserialized.notifications_enabled);
}

#[test]
fn test_close_to_tray_toggle() {
    set_close_to_tray(false);
    assert!(!is_close_to_tray_enabled());

    set_close_to_tray(true);
    assert!(is_close_to_tray_enabled());
}

#[test]
fn test_syncify_tray_id_constant() {
    assert_eq!(SYNCIFY_TRAY_ID, "syncify-tray");
}

#[test]
fn test_tray_menu_building_with_mock_app() {
    let app = tauri::test::mock_app();
    let handle = app.handle();

    // 1. Idle menu
    let menu_idle = build_tray_menu(&handle, true, false, 0, None)
        .expect("Build idle tray menu");
    assert!(menu_idle.items().is_ok());

    // 2. Downloading menu
    let menu_downloading = build_tray_menu(&handle, false, true, 5, None)
        .expect("Build downloading tray menu");
    assert!(menu_downloading.items().is_ok());

    // 3. Syncing menu
    let menu_syncing = build_tray_menu(&handle, true, false, 0, Some("Spotify"))
        .expect("Build syncing tray menu");
    assert!(menu_syncing.items().is_ok());
}
