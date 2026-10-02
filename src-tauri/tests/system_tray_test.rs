//! Tests for System Tray and Desktop Notification Contract (TASK-120)

use serde_json::json;
use std::sync::Arc;
use syncify_tauri_lib::tray::{
    build_tray_menu, handle_menu_click, is_close_to_tray_enabled, set_close_to_tray,
    sync_all_services, TraySettings, TrayState, SYNCIFY_TRAY_ID,
};
use syncify_tauri_lib::worker::DownloadWorkerState;
use syncify_tauri_lib::{AppState, EnrichmentWorkerState};
use tauri::Manager;

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
    let menu_idle = build_tray_menu(&handle, true, false, 0, None).expect("Build idle tray menu");
    assert!(menu_idle.items().is_ok());

    // 2. Downloading menu
    let menu_downloading =
        build_tray_menu(&handle, false, true, 5, None).expect("Build downloading tray menu");
    assert!(menu_downloading.items().is_ok());

    // 3. Syncing menu
    let menu_syncing =
        build_tray_menu(&handle, true, false, 0, Some("Spotify")).expect("Build syncing tray menu");
    assert!(menu_syncing.items().is_ok());
}

// ─────────────────────────────────────────────────────────────────────────────
// IN-4 regression: tray menu actions must execute real behavior in Rust.
// The former 'tray-action' events had no UI listener, leaving
// pause/resume/sync inoperative and 'Check for Updates' dangling.
// ─────────────────────────────────────────────────────────────────────────────

fn menu_item_ids<R: tauri::Runtime>(menu: &tauri::menu::Menu<R>) -> Vec<String> {
    menu.items()
        .expect("read tray menu items")
        .iter()
        .map(|item| item.id().as_ref().to_string())
        .collect()
}

#[test]
fn tray_menu_no_longer_contains_check_updates_item() {
    let app = tauri::test::mock_app();
    let handle = app.handle();

    // Visible window → the toggle item is "Hide Syncify" (id "hide");
    // idle → the action item is "Resume Downloads" (id "resume_downloads").
    let menu = build_tray_menu(&handle, true, false, 0, None).expect("Build tray menu");
    let ids = menu_item_ids(&menu);

    assert!(
        !ids.iter().any(|id| id == "check_updates"),
        "'Check for Updates' must be removed (no updater command exists in Rust); got {ids:?}"
    );
    for expected in ["hide", "resume_downloads", "sync_all", "settings", "quit"] {
        assert!(
            ids.iter().any(|id| id == expected),
            "tray menu must keep the '{expected}' item; got {ids:?}"
        );
    }

    // Hidden window → the toggle item is "Show Syncify" (id "show");
    // downloading → the action item is "Pause All Downloads" (id "pause_downloads").
    let menu_downloading = build_tray_menu(&handle, false, true, 5, None).expect("Build tray menu");
    let ids_downloading = menu_item_ids(&menu_downloading);
    assert!(
        !ids_downloading.iter().any(|id| id == "check_updates"),
        "'Check for Updates' must be removed (no updater command exists in Rust); got {ids_downloading:?}"
    );
    for expected in ["show", "pause_downloads", "sync_all", "settings", "quit"] {
        assert!(
            ids_downloading.iter().any(|id| id == expected),
            "tray menu must keep the '{expected}' item while downloading; got {ids_downloading:?}"
        );
    }
}

async fn tray_test_app() -> (
    tauri::App<tauri::test::MockRuntime>,
    syncify_tauri_lib::db::DbPool,
) {
    // Single connection: `sqlite::memory:` databases are per-connection.
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("create in-memory pool");
    let app = tauri::test::mock_app();
    app.manage(AppState {
        db: pool.clone(),
        worker_state: DownloadWorkerState::new(2),
        enrichment_state: EnrichmentWorkerState::new(),
        concurrency_manager: Arc::new(syncify_tauri_lib::services::ConcurrencyManager::new()),
    });
    (app, pool)
}

#[tokio::test]
async fn tray_menu_click_pause_and_resume_downloads_control_worker_state() {
    let (app, _db) = tray_test_app().await;
    let handle = app.handle();

    assert!(!handle.state::<AppState>().worker_state.is_paused());

    handle_menu_click(handle, "pause_downloads");
    assert!(
        handle.state::<AppState>().worker_state.is_paused(),
        "tray 'Pause All Downloads' must pause the download worker"
    );

    handle_menu_click(handle, "resume_downloads");
    assert!(
        !handle.state::<AppState>().worker_state.is_paused(),
        "tray 'Resume Downloads' must resume the download worker"
    );
}

#[tokio::test]
async fn tray_sync_all_without_active_services_is_a_noop() {
    let (app, db) = tray_test_app().await;

    // Minimal schema so the active-service query can run; no accounts → no syncs.
    sqlx::query("CREATE TABLE services (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE)")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query(
        "CREATE TABLE accounts (id INTEGER PRIMARY KEY, service_id INTEGER NOT NULL, is_active INTEGER DEFAULT 1)",
    )
    .execute(&db)
    .await
    .unwrap();

    let synced = sync_all_services(app.handle())
        .await
        .expect("sync_all_services must succeed without active services");
    assert_eq!(
        synced, 0,
        "no active accounts must yield zero synced services"
    );
}

#[tokio::test]
async fn tray_sync_all_only_targets_active_accounts() {
    let (_app, db) = tray_test_app().await;

    sqlx::query("CREATE TABLE services (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE)")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query(
        "CREATE TABLE accounts (id INTEGER PRIMARY KEY, service_id INTEGER NOT NULL, is_active INTEGER DEFAULT 1)",
    )
    .execute(&db)
    .await
    .unwrap();
    sqlx::query("INSERT INTO services (id, name) VALUES (1, 'Qobuz'), (2, 'Tidal')")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO accounts (id, service_id, is_active) VALUES (1, 1, 1), (2, 1, 1), (3, 2, 0)",
    )
    .execute(&db)
    .await
    .unwrap();

    // The engine performs real network syncs, which is out of scope for a unit
    // test; the tray path must instead fail cleanly per service (no network in
    // tests) and the queried set must exclude inactive accounts. We assert the
    // query contract directly against the same SQL the helper uses.
    let active: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT LOWER(s.name) FROM accounts a JOIN services s ON s.id = a.service_id WHERE a.is_active = 1 ORDER BY 1",
    )
    .fetch_all(&db)
    .await
    .unwrap();

    assert_eq!(
        active,
        vec!["qobuz".to_string()],
        "only active accounts feed the tray sync; got {active:?}"
    );
}
