//! Regression suite for the tray startup preferences (audit item 31).
//!
//! `get_tray_settings` answered with `TraySettings::default()` for every field
//! except `close_to_tray`, so the frontend read back `start_on_boot: false` /
//! `start_minimized: false` and sent those defaults back on every save — which
//! also wiped the autostart entry the user had just enabled.

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::tray::load_tray_settings;

async fn setup_test_db() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

async fn save(pool: &sqlx::SqlitePool, key: &str, value: &str) {
    sqlx::query(
        "INSERT INTO settings (key, value, updated_at) VALUES (?, ?, datetime('now')) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await
    .expect("Failed to persist the setting");
}

#[tokio::test]
async fn unsaved_keys_fall_back_to_the_struct_defaults() {
    let pool = setup_test_db().await;

    let settings = load_tray_settings(&pool).await;

    assert!(
        settings.close_to_tray,
        "the default keeps the app in the tray"
    );
    assert!(!settings.start_on_boot);
    assert!(!settings.start_minimized);
}

#[tokio::test]
async fn persisted_startup_preferences_are_returned_verbatim() {
    let pool = setup_test_db().await;
    save(&pool, "close_to_tray", "false").await;
    save(&pool, "start_minimized", "true").await;
    save(&pool, "start_on_boot", "true").await;

    let settings = load_tray_settings(&pool).await;

    assert!(!settings.close_to_tray);
    assert!(settings.start_minimized);
    assert!(settings.start_on_boot);
}

#[tokio::test]
async fn a_saved_false_is_not_confused_with_a_missing_key() {
    let pool = setup_test_db().await;
    save(&pool, "close_to_tray", "false").await;
    save(&pool, "start_on_boot", "false").await;

    let settings = load_tray_settings(&pool).await;

    assert!(
        !settings.close_to_tray,
        "an explicit false must win over the default true"
    );
    assert!(!settings.start_on_boot);
}

#[tokio::test]
async fn numeric_and_padded_boolean_encodings_are_understood() {
    let pool = setup_test_db().await;
    save(&pool, "start_on_boot", "1").await;
    save(&pool, "start_minimized", "0").await;
    save(&pool, "close_to_tray", " True ").await;

    let settings = load_tray_settings(&pool).await;

    assert!(settings.start_on_boot);
    assert!(!settings.start_minimized);
    assert!(settings.close_to_tray);
}

#[tokio::test]
async fn unrecognised_values_fall_back_instead_of_silently_disabling() {
    let pool = setup_test_db().await;
    save(&pool, "close_to_tray", "maybe").await;

    let settings = load_tray_settings(&pool).await;

    assert!(
        settings.close_to_tray,
        "an unparsable value must not silently hide the window behind the tray"
    );
}

#[tokio::test]
async fn notification_categories_round_trip_independently_from_startup_defaults() {
    let pool = setup_test_db().await;
    save(&pool, "notify_download_complete", "false").await;
    save(&pool, "notify_sync_complete", "true").await;
    save(&pool, "notify_errors", "false").await;
    save(&pool, "notify_updates", "false").await;
    save(&pool, "notification_sound", "true").await;
    save(&pool, "notify_when_visible", "true").await;

    let settings = load_tray_settings(&pool).await;
    assert!(!settings.notify_download_complete);
    assert!(settings.notify_sync_complete);
    assert!(!settings.notify_errors);
    assert!(!settings.notify_updates);
    assert!(settings.notification_sound);
    assert!(settings.notify_when_visible);
    assert!(settings.notifications_enabled);
}

#[tokio::test]
async fn download_success_and_errors_use_independent_notification_preferences() {
    let pool = setup_test_db().await;
    save(&pool, "notify_download_complete", "false").await;
    save(&pool, "notify_errors", "true").await;
    assert!(
        !syncify_tauri_lib::tray::notification_category_enabled(&pool, "download", false).await
    );
    assert!(syncify_tauri_lib::tray::notification_category_enabled(&pool, "download", true).await);

    save(&pool, "notify_errors", "false").await;
    assert!(!syncify_tauri_lib::tray::notification_category_enabled(&pool, "download", true).await);
    save(&pool, "notifications_enabled", "false").await;
    assert!(!syncify_tauri_lib::tray::notification_category_enabled(&pool, "sync", false).await);
}
