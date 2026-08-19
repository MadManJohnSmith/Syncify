use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use syncify_tauri_lib::crypto;
use syncify_tauri_lib::services::tidal::{
    check_album_availability, classify_album_expansion_error, clear_album_availability,
    record_album_availability, TidalAlbumExpansionStatus, DEFAULT_UNAVAILABLE_ALBUM_TTL_SECS,
};
use syncify_tauri_lib::commands::types::{ImportPreferences, ServiceSyncResult};

async fn setup_test_db() -> SqlitePool {
    let _ = crypto::init_keychain_crypto().or_else(|_| crypto::init_crypto([42u8; 32]));

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory test database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to apply migrations to test database");

    // Ensure Tidal service exists
    sqlx::query("INSERT OR IGNORE INTO services (id, name) VALUES (3, 'tidal')")
        .execute(&pool)
        .await
        .unwrap();

    // Insert test Tidal account (account_id = 50)
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, credentials_invalid) VALUES (50, 3, 'Tidal Test Account', 0)")
        .execute(&pool)
        .await
        .unwrap();

    pool
}

#[test]
fn test_classify_404_substatus_2001_as_unavailable_from_provider() {
    let body = r#"{"status":404,"subStatus":2001,"userMessage":"Album [309652808] not found"}"#;
    let status = reqwest::StatusCode::NOT_FOUND;

    let (expansion_status, sub_status, reason) = classify_album_expansion_error(status, body);

    assert_eq!(expansion_status, TidalAlbumExpansionStatus::UnavailableFromProvider);
    assert_eq!(sub_status, Some(2001));
    assert!(reason.contains("309652808"), "Reason must contain album ID message");
}

#[test]
fn test_classify_400_region_restricted_not_unavailable() {
    let body = r#"{"status":400,"subStatus":4005,"userMessage":"Asset is not available in country MX"}"#;
    let status = reqwest::StatusCode::BAD_REQUEST;

    let (expansion_status, sub_status, reason) = classify_album_expansion_error(status, body);

    assert_eq!(expansion_status, TidalAlbumExpansionStatus::RegionRestricted);
    assert_eq!(sub_status, Some(4005));
    assert!(reason.contains("country MX"));
}

#[test]
fn test_classify_401_auth_failed_not_unavailable() {
    let body = r#"{"status":401,"subStatus":1002,"userMessage":"Expired token"}"#;
    let status = reqwest::StatusCode::UNAUTHORIZED;

    let (expansion_status, sub_status, reason) = classify_album_expansion_error(status, body);

    assert_eq!(expansion_status, TidalAlbumExpansionStatus::AuthFailed);
    assert_eq!(sub_status, Some(1002));
    assert!(reason.contains("Expired token"));
}

#[test]
fn test_classify_429_rate_limited() {
    let body = r#"{"status":429,"userMessage":"Too Many Requests"}"#;
    let status = reqwest::StatusCode::TOO_MANY_REQUESTS;

    let (expansion_status, sub_status, reason) = classify_album_expansion_error(status, body);

    assert_eq!(expansion_status, TidalAlbumExpansionStatus::RateLimited);
    assert_eq!(sub_status, None);
    assert!(reason.contains("Too Many Requests"));
}

#[test]
fn test_classify_500_temporarily_failed() {
    let body = r#"{"status":500,"userMessage":"Internal server error"}"#;
    let status = reqwest::StatusCode::INTERNAL_SERVER_ERROR;

    let (expansion_status, _sub_status, reason) = classify_album_expansion_error(status, body);

    assert_eq!(expansion_status, TidalAlbumExpansionStatus::TemporarilyFailed);
    assert!(reason.contains("Internal server error"));
}

#[tokio::test]
async fn test_404_does_not_change_credentials_invalid() {
    let pool = setup_test_db().await;

    // Verify initial state: credentials_invalid = 0
    let creds_invalid: i64 = sqlx::query_scalar("SELECT credentials_invalid FROM accounts WHERE id = 50")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(creds_invalid, 0);

    // Record 404 unavailable album
    record_album_availability(
        &pool,
        3,
        "309652808",
        TidalAlbumExpansionStatus::UnavailableFromProvider,
        Some(404),
        Some(2001),
        Some("Album [309652808] not found"),
    )
    .await
    .unwrap();

    // Verify credentials_invalid is STILL 0 (NOT invalidated by 404)
    let creds_invalid_after: i64 = sqlx::query_scalar("SELECT credentials_invalid FROM accounts WHERE id = 50")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(creds_invalid_after, 0, "404 on album must never set credentials_invalid = 1");
}

#[tokio::test]
async fn test_unavailable_preserves_album_and_favorites() {
    let pool = setup_test_db().await;

    // Insert album as favorite
    sqlx::query(
        r#"
        INSERT INTO albums (title, tidal_id, total_tracks, is_favorite, favorite_at)
        VALUES ('DANSE MACABRE', '309652808', 13, 1, CURRENT_TIMESTAMP)
        "#
    )
    .execute(&pool)
    .await
    .unwrap();

    // Record unavailable status
    record_album_availability(
        &pool,
        3,
        "309652808",
        TidalAlbumExpansionStatus::UnavailableFromProvider,
        Some(404),
        Some(2001),
        Some("Album [309652808] not found"),
    )
    .await
    .unwrap();

    // Verify album row remains intact in albums table with is_favorite = 1
    let (is_fav, tidal_id, total_tracks): (i64, Option<String>, Option<i32>) = sqlx::query_as(
        "SELECT is_favorite, tidal_id, total_tracks FROM albums WHERE title = 'DANSE MACABRE'"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(is_fav, 1, "Album favorite flag must be preserved");
    assert_eq!(tidal_id.as_deref(), Some("309652808"));
    assert_eq!(total_tracks, Some(13));
}

#[tokio::test]
async fn test_resync_within_ttl_returns_cached_unavailable() {
    let pool = setup_test_db().await;

    // Record unavailable status
    record_album_availability(
        &pool,
        3,
        "309652808",
        TidalAlbumExpansionStatus::UnavailableFromProvider,
        Some(404),
        Some(2001),
        Some("Album [309652808] not found"),
    )
    .await
    .unwrap();

    // Check availability with standard 7-day TTL
    let check = check_album_availability(&pool, 3, "309652808", DEFAULT_UNAVAILABLE_ALBUM_TTL_SECS)
        .await
        .unwrap();

    assert!(check.is_some(), "Must return cached unavailable status within TTL");
    let (status, reason) = check.unwrap();
    assert_eq!(status, TidalAlbumExpansionStatus::UnavailableFromProvider);
    assert!(reason.contains("309652808"));
}

#[tokio::test]
async fn test_resync_expired_ttl_returns_none_to_trigger_api_call() {
    let pool = setup_test_db().await;

    // Insert record with last_checked in the past (e.g. 10 days ago)
    sqlx::query(
        r#"
        INSERT INTO service_album_availability
            (service_id, service_album_id, availability_status, http_status, sub_status, reason, last_checked)
        VALUES (3, '309652808', 'UnavailableFromProvider', 404, 2001, 'Album not found', datetime('now', '-10 days'))
        "#
    )
    .execute(&pool)
    .await
    .unwrap();

    // Check availability with 7-day TTL (7 * 86400 = 604800)
    let check = check_album_availability(&pool, 3, "309652808", DEFAULT_UNAVAILABLE_ALBUM_TTL_SECS)
        .await
        .unwrap();

    assert!(check.is_none(), "Expired TTL must return None to allow re-checking provider API");
}

#[tokio::test]
async fn test_200_ok_recovery_clears_unavailable_status() {
    let pool = setup_test_db().await;

    // 1. Record unavailable status
    record_album_availability(
        &pool,
        3,
        "309652808",
        TidalAlbumExpansionStatus::UnavailableFromProvider,
        Some(404),
        Some(2001),
        Some("Album [309652808] not found"),
    )
    .await
    .unwrap();

    let check_before = check_album_availability(&pool, 3, "309652808", DEFAULT_UNAVAILABLE_ALBUM_TTL_SECS)
        .await
        .unwrap();
    assert!(check_before.is_some());

    // 2. On 200 OK recovery: clear album availability
    clear_album_availability(&pool, 3, "309652808").await.unwrap();

    let check_after = check_album_availability(&pool, 3, "309652808", DEFAULT_UNAVAILABLE_ALBUM_TTL_SECS)
        .await
        .unwrap();
    assert!(check_after.is_none(), "Cleared album availability must return None");
}

#[test]
fn test_sync_result_outcome_success_with_warnings_when_albums_unavailable() {
    let result = ServiceSyncResult {
        service: "tidal".to_string(),
        account_id: Some(50),
        success: true,
        message: "Sync completed with warnings for tidal: 91 favorites, 107 albums (10 unavailable from provider)".to_string(),
        imported_tracks_total: 0,
        favorite_tracks_total: 91,
        favorite_albums_total: 107,
        favorite_artists_total: 0,
        playlists_total: 57,
        purchases_total: 0,
        skipped_tracks_total: 3526,
        albums_total: 107,
        metadata_enriched: 3526,
        metadata_partial: 0,
        availability_unknown: 0,
        availability_checked: 3526,
        phase_timings: None,
        album_expansion_metrics: None,
        tracks_processed: 3526,
        tracks_changed_unique: 0,
        tracks_new_global: 0,
        sources_new_for_service: 0,
        library_entries_new_for_account: 0,
        tracks_already_present: 3526,
        favorites_seen: 91,
        albums_seen: 107,
        playlists_seen: 57,
        tracks_expanded: 3435,
        tracks_expansion_failed: 0,
        albums_unavailable: 10,
        tracks_unavailable: 47,
        tracks_expansion_deferred: 47,
        sync_outcome: Some("success_with_warnings".to_string()),
        warnings: vec![
            "Album 'DANSE MACABRE' (309652808) is unavailable from Tidal (UnavailableFromProvider)".to_string(),
            "Album 'Figure It Out' (300185109) is unavailable from Tidal (UnavailableFromProvider)".to_string(),
        ],
        errors: vec![],
        ..Default::default()
    };

    assert!(result.success, "success must be true when only warnings / unavailable albums exist");
    assert_eq!(result.sync_outcome.as_deref(), Some("success_with_warnings"));
    assert_eq!(result.albums_unavailable, 10);
    assert_eq!(result.tracks_unavailable, 47);
    assert_eq!(result.tracks_expansion_failed, 0);
    assert!(result.errors.is_empty(), "Unavailable albums must produce warnings, not fatal errors");
}

#[test]
fn test_import_preferences_supports_force_retry_unavailable() {
    let prefs = ImportPreferences {
        service_name: "tidal".to_string(),
        favorite_tracks: true,
        favorite_albums: true,
        favorite_artists: false,
        playlists: true,
        purchases: false,
        library_history: false,
        include_appearances: false,
        incremental_sync: true,
        force_retry_unavailable: true,
    };

    assert!(prefs.force_retry_unavailable);

    let json_str = serde_json::to_string(&prefs).unwrap();
    let roundtrip: ImportPreferences = serde_json::from_str(&json_str).unwrap();
    assert!(roundtrip.force_retry_unavailable);
}

#[tokio::test]
async fn test_service_sync_result_ipc_contract_camel_case_validation() {
    let pool = setup_test_db().await;

    // Verify accounts credentials_invalid is false (0)
    let creds_invalid: i64 = sqlx::query_scalar("SELECT credentials_invalid FROM accounts WHERE id = 50")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(creds_invalid, 0, "credentials_invalid must be false (0)");

    let result = ServiceSyncResult {
        service: "tidal".to_string(),
        account_id: Some(50),
        success: true,
        message: "Sync completed with warnings for tidal: 91 favorites, 107 albums (10 unavailable from provider)".to_string(),
        imported_tracks_total: 0,
        favorite_tracks_total: 91,
        favorite_albums_total: 107,
        favorite_artists_total: 0,
        playlists_total: 57,
        purchases_total: 0,
        skipped_tracks_total: 3526,
        albums_total: 107,
        metadata_enriched: 3526,
        metadata_partial: 0,
        availability_unknown: 0,
        availability_checked: 3526,
        phase_timings: None,
        album_expansion_metrics: None,
        tracks_processed: 3526,
        tracks_changed_unique: 0,
        tracks_new_global: 0,
        sources_new_for_service: 0,
        library_entries_new_for_account: 0,
        tracks_already_present: 3526,
        favorites_seen: 91,
        albums_seen: 107,
        playlists_seen: 57,
        tracks_expanded: 3435,
        tracks_expansion_failed: 0,
        albums_unavailable: 10,
        tracks_unavailable: 47,
        tracks_expansion_deferred: 47,
        sync_outcome: Some("success_with_warnings".to_string()),
        warnings: vec![
            "Album 'DANSE MACABRE' (309652808) is unavailable from Tidal (UnavailableFromProvider)".to_string(),
        ],
        errors: vec![],
        ..Default::default()
    };

    let json_val = serde_json::to_value(&result).expect("Must serialize to JSON");

    // Assert exact IPC camelCase properties
    assert_eq!(json_val["syncOutcome"], "success_with_warnings");
    assert_eq!(json_val["albumsUnavailable"], 10);
    assert_eq!(json_val["tracksUnavailable"], 47);
    assert_eq!(json_val["tracksExpansionDeferred"], 47);
    assert_eq!(json_val["tracksExpansionFailed"], 0);
    assert_eq!(json_val["success"], true);

    // Verify snake_case does NOT exist in direct serialization
    assert!(json_val.get("sync_outcome").is_none());
    assert!(json_val.get("albums_unavailable").is_none());
    assert!(json_val.get("tracks_unavailable").is_none());
    assert!(json_val.get("tracks_expansion_deferred").is_none());
}

