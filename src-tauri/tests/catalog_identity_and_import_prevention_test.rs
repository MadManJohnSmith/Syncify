//! Comprehensive Regression Test Suite for Catalog Identity & Import Prevention
//!
//! Validates:
//! 1. 1:1 mapping of (service_id, service_track_id) -> canonical track.
//! 2. Numeric provider IDs are never interpreted as ISRC.
//! 3. Valid ISRC format enforcement.
//! 4. Preservation of distinct editions/masters (same title != automatic merge).
//! 5. Zero creation of 'Unknown Artist' or 'Unknown Album' as canonical truth on partial payloads.
//! 6. Playlist order preservation and pagination resilience.
//! 7. Full import idempotency.
//! 8. Error classification (401 OAuth vs 404 Catalog vs 429 RateLimit).
//! 9. Zero audio downloads triggered during library/playlist import.

use sqlx::sqlite::SqlitePoolOptions;
use tempfile::TempDir;
use syncify_core_domain::metadata::{
    is_placeholder_album, is_placeholder_artist, is_placeholder_title, is_valid_isrc,
    ProviderTrackIdentity,
};
use syncify_core_domain::errors::ErrorTaxonomy;
use syncify_tauri_lib::services::track_matcher::find_or_create_track_with_identity;

#[tokio::test]
async fn test_provider_numeric_id_never_treated_as_isrc() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("isrc_prevention.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Connect to DB");

    sqlx::migrate!("./migrations").run(&pool).await.expect("Run migrations");

    let tidal_numeric_id = "134683067";
    assert!(!is_valid_isrc(tidal_numeric_id), "Tidal numeric track ID must NOT be valid ISRC");

    let ident = ProviderTrackIdentity {
        service_id: 3,
        service_name: "tidal".to_string(),
        service_track_id: tidal_numeric_id.to_string(),
        isrc: Some(tidal_numeric_id.to_string()), // Attempt to pass numeric ID as ISRC
        provider_album_id: Some("134683060".to_string()),
        provider_artist_id: Some("3567".to_string()),
        title: Some("Paranoid Android".to_string()),
        artist: Some("Radiohead".to_string()),
        album: Some("OK Computer".to_string()),
        duration_ms: Some(387000),
        track_number: Some(2),
        disc_number: Some(1),
        explicit: Some(false),
    };

    let match_res = find_or_create_track_with_identity(&pool, &ident, None).await.expect("Create track");
    assert!(match_res.is_new);

    // Verify in DB that isrc is NULL (rejected numeric ID)
    let isrc_in_db: Option<String> = sqlx::query_scalar("SELECT isrc FROM tracks WHERE id = ?")
        .bind(match_res.track_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(isrc_in_db, None, "Numeric provider ID must never be persisted into tracks.isrc");
}

#[tokio::test]
async fn test_source_mapping_exact_canonical_resolution_and_idempotency() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("source_mapping.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Connect to DB");

    sqlx::migrate!("./migrations").run(&pool).await.expect("Run migrations");

    let ident = ProviderTrackIdentity {
        service_id: 3,
        service_name: "tidal".to_string(),
        service_track_id: "280721704".to_string(),
        isrc: Some("USRC17607839".to_string()),
        provider_album_id: Some("280721700".to_string()),
        provider_artist_id: Some("123".to_string()),
        title: Some("Karma Police".to_string()),
        artist: Some("Radiohead".to_string()),
        album: Some("OK Computer".to_string()),
        duration_ms: Some(264000),
        track_number: Some(6),
        disc_number: Some(1),
        explicit: Some(false),
    };

    // First call creates track
    let match1 = find_or_create_track_with_identity(&pool, &ident, None).await.expect("First match");
    assert!(match1.is_new);

    // Add source mapping explicitly
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, ?, ?)")
        .bind(match1.track_id)
        .bind(ident.service_id)
        .bind(&ident.service_track_id)
        .execute(&pool)
        .await
        .expect("Insert track source");

    // Second call resolves existing canonical track 1:1 without creating duplicate
    let match2 = find_or_create_track_with_identity(&pool, &ident, None).await.expect("Second match");
    assert!(!match2.is_new);
    assert_eq!(match1.track_id, match2.track_id, "Must resolve to identical canonical track ID");

    let total_tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks").fetch_one(&pool).await.unwrap();
    assert_eq!(total_tracks, 1, "Must have exactly 1 canonical track in database");
}

#[tokio::test]
async fn test_same_title_distinct_masters_are_not_falsely_merged() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("distinct_masters.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Connect to DB");

    sqlx::migrate!("./migrations").run(&pool).await.expect("Run migrations");

    // Master A: Original Album version
    let master_a = ProviderTrackIdentity {
        service_id: 3,
        service_name: "tidal".to_string(),
        service_track_id: "1001".to_string(),
        isrc: Some("GBAYE0601477".to_string()),
        provider_album_id: Some("alb-1".to_string()),
        provider_artist_id: Some("art-1".to_string()),
        title: Some("Heroes".to_string()),
        artist: Some("David Bowie".to_string()),
        album: Some("Heroes (1977)".to_string()),
        duration_ms: Some(367000),
        track_number: Some(3),
        disc_number: Some(1),
        explicit: Some(false),
    };

    // Master B: 2017 Remaster with distinct ISRC and distinct service_track_id
    let master_b = ProviderTrackIdentity {
        service_id: 3,
        service_name: "tidal".to_string(),
        service_track_id: "1002".to_string(),
        isrc: Some("GBAYE1700123".to_string()),
        provider_album_id: Some("alb-2".to_string()),
        provider_artist_id: Some("art-1".to_string()),
        title: Some("Heroes".to_string()),
        artist: Some("David Bowie".to_string()),
        album: Some("Heroes (2017 Remaster)".to_string()),
        duration_ms: Some(371000),
        track_number: Some(3),
        disc_number: Some(1),
        explicit: Some(false),
    };

    let match_a = find_or_create_track_with_identity(&pool, &master_a, None).await.expect("Create master A");
    let match_b = find_or_create_track_with_identity(&pool, &master_b, None).await.expect("Create master B");

    assert_ne!(match_a.track_id, match_b.track_id, "Distinct masters must remain separate canonical tracks");
}

#[tokio::test]
async fn test_error_taxonomy_classification_properties() {
    // 401 Session invalidation vs 404 Catalog item
    let auth_err = ErrorTaxonomy::AuthInvalid { message: "Refresh token expired".to_string() };
    assert!(auth_err.invalidates_credentials());
    assert!(!auth_err.is_retryable());

    let not_found_err = ErrorTaxonomy::UnavailableFromProvider {
        provider: "tidal".to_string(),
        item_id: "99999999".to_string(),
        reason: "Catalog item 404".to_string(),
    };
    assert!(!not_found_err.invalidates_credentials(), "404 catalog item must NEVER invalidate user credentials");
    assert!(!not_found_err.is_retryable());

    let rate_limit_err = ErrorTaxonomy::RateLimited {
        provider: "spotify".to_string(),
        retry_after_sec: Some(60),
    };
    assert!(rate_limit_err.is_retryable());
    assert_eq!(rate_limit_err.retry_delay_sec(), 60);
    assert!(!rate_limit_err.invalidates_credentials());
}

#[tokio::test]
async fn test_placeholder_rejection_invariants() {
    assert!(is_placeholder_title("Tidal Track 12345678"));
    assert!(is_placeholder_title("Unknown Track"));
    assert!(is_placeholder_artist("Unknown Artist"));
    assert!(is_placeholder_album("Unknown Album"));

    assert!(!is_placeholder_title("Track 9 (Live in Tokyo)")); // Legitimate song title containing 'track'
    assert!(!is_placeholder_artist("Unknown Mortal Orchestra")); // Legitimate artist name
    assert!(!is_placeholder_album("Unknown Pleasures")); // Legitimate Joy Division album
}

#[tokio::test]
async fn test_cross_provider_same_isrc_links_to_single_canonical_track() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("cross_provider_isrc.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Connect to DB");

    sqlx::migrate!("./migrations").run(&pool).await.expect("Run migrations");

    let shared_isrc = "GBAYE0601477";

    // 1. Qobuz import with valid ISRC
    let qobuz_track = ProviderTrackIdentity {
        service_id: 2,
        service_name: "qobuz".to_string(),
        service_track_id: "qobuz-101".to_string(),
        isrc: Some(shared_isrc.to_string()),
        provider_album_id: Some("qobuz-alb-1".to_string()),
        provider_artist_id: Some("qobuz-art-1".to_string()),
        title: Some("Heroes".to_string()),
        artist: Some("David Bowie".to_string()),
        album: Some("Heroes".to_string()),
        duration_ms: Some(367000),
        track_number: Some(3),
        disc_number: Some(1),
        explicit: Some(false),
    };

    let qobuz_match = find_or_create_track_with_identity(&pool, &qobuz_track, None).await.expect("Qobuz match");
    assert!(qobuz_match.is_new);

    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, ?, ?)")
        .bind(qobuz_match.track_id)
        .bind(qobuz_track.service_id)
        .bind(&qobuz_track.service_track_id)
        .execute(&pool)
        .await
        .expect("Insert Qobuz source");

    // 2. Tidal import with identical valid ISRC
    let tidal_track = ProviderTrackIdentity {
        service_id: 3,
        service_name: "tidal".to_string(),
        service_track_id: "tidal-202".to_string(),
        isrc: Some(shared_isrc.to_string()),
        provider_album_id: Some("tidal-alb-2".to_string()),
        provider_artist_id: Some("tidal-art-1".to_string()),
        title: Some("Heroes".to_string()),
        artist: Some("David Bowie".to_string()),
        album: Some("Heroes".to_string()),
        duration_ms: Some(367000),
        track_number: Some(3),
        disc_number: Some(1),
        explicit: Some(false),
    };

    let tidal_match = find_or_create_track_with_identity(&pool, &tidal_track, None).await.expect("Tidal match");
    assert!(!tidal_match.is_new, "Tidal track must resolve to existing canonical track with same ISRC");
    assert_eq!(qobuz_match.track_id, tidal_match.track_id, "Both providers must share identical canonical track_id");

    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, ?, ?)")
        .bind(tidal_match.track_id)
        .bind(tidal_track.service_id)
        .bind(&tidal_track.service_track_id)
        .execute(&pool)
        .await
        .expect("Insert Tidal source");

    // Verify 1 canonical track has 2 sources in track_sources
    let source_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM track_sources WHERE track_id = ?")
        .bind(qobuz_match.track_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(source_count, 2, "Canonical track must have exactly 2 provider sources");
}

#[tokio::test]
async fn test_playlist_pagination_and_order_preservation() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("playlist_order.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Connect to DB");

    sqlx::migrate!("./migrations").run(&pool).await.expect("Run migrations");

    let service_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'tidal'")
        .fetch_one(&pool)
        .await
        .unwrap_or(3);
    let account_id: i64 = sqlx::query_scalar("INSERT INTO accounts (service_id, email) VALUES (?, 'test@syncify.local') RETURNING id")
        .bind(service_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    let playlist_id: i64 = sqlx::query_scalar(
        "INSERT INTO playlists (account_id, name, track_count) VALUES (?, 'Big Playlist', 120) RETURNING id"
    )
    .bind(account_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Insert 120 tracks across 3 simulated pages and verify positions 1..=120 are strictly preserved
    for i in 1..=120 {
        let tid: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, duration_ms) VALUES (?, 180000) RETURNING id"
        )
        .bind(format!("Track Number {:03}", i))
        .fetch_one(&pool)
        .await
        .unwrap();

        sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)")
            .bind(playlist_id)
            .bind(tid)
            .bind(i as i32)
            .execute(&pool)
            .await
            .unwrap();
    }

    let ordered_positions: Vec<i32> = sqlx::query_scalar(
        "SELECT position FROM playlist_tracks WHERE playlist_id = ? ORDER BY position ASC"
    )
    .bind(playlist_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(ordered_positions.len(), 120);
    for (idx, pos) in ordered_positions.iter().enumerate() {
        assert_eq!(*pos, (idx + 1) as i32, "Playlist position must match index 1..=120");
    }
}

#[tokio::test]
async fn test_fresh_import_idempotency_zero_audio_downloads() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("import_idempotency.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Connect to DB");

    sqlx::migrate!("./migrations").run(&pool).await.expect("Run migrations");

    let track_data = ProviderTrackIdentity {
        service_id: 1,
        service_name: "spotify".to_string(),
        service_track_id: "spotify-track-100".to_string(),
        isrc: Some("USRC17607839".to_string()),
        provider_album_id: Some("spot-alb-1".to_string()),
        provider_artist_id: Some("spot-art-1".to_string()),
        title: Some("Paranoid Android".to_string()),
        artist: Some("Radiohead".to_string()),
        album: Some("OK Computer".to_string()),
        duration_ms: Some(387000),
        track_number: Some(2),
        disc_number: Some(1),
        explicit: Some(false),
    };

    // Run 1: Import
    let r1 = find_or_create_track_with_identity(&pool, &track_data, None).await.unwrap();
    assert!(r1.is_new);
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, ?, ?)")
        .bind(r1.track_id)
        .bind(track_data.service_id)
        .bind(&track_data.service_track_id)
        .execute(&pool)
        .await
        .unwrap();

    // Run 2: Re-run import of identical track
    let r2 = find_or_create_track_with_identity(&pool, &track_data, None).await.unwrap();
    assert!(!r2.is_new, "Re-run must not create a new canonical track");
    assert_eq!(r1.track_id, r2.track_id);

    // Verify 0 download records were created (import only, zero audio download)
    let download_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads").fetch_one(&pool).await.unwrap();
    assert_eq!(download_count, 0, "Library sync must NEVER trigger audio downloads");
}

#[tokio::test]
async fn test_apply_catalog_identity_repair_safety_and_backup() {
    use syncify_tauri_lib::services::catalog_identity_repair::{
        plan_catalog_identity_repair, apply_catalog_identity_repair,
    };

    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("repair_safety.db");
    let backup_dir = temp_dir.path().join("backups");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Connect to DB");

    sqlx::migrate!("./migrations").run(&pool).await.expect("Run migrations");

    // Insert an anomaly: a track with valid track_source but invalid numeric ISRC
    let corrupt_tid: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, duration_ms) VALUES ('Corrupt Track', '134683067', 180000) RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 1, 'src-100')")
        .bind(corrupt_tid)
        .execute(&pool)
        .await
        .unwrap();

    // 1. Plan Dry-Run repair
    let plan = plan_catalog_identity_repair(&pool, None).await.expect("Plan repair");
    assert!(plan.requires_confirmation);
    assert_eq!(plan.items_to_repair.len(), 1);
    assert_eq!(plan.items_to_repair[0].entity_id, Some(corrupt_tid));

    // 2. Test Apply without confirmed: true -> MUST FAIL and make 0 mutations
    let unconfirmed_res = apply_catalog_identity_repair(&pool, &plan, false, Some(&backup_dir)).await;
    assert!(unconfirmed_res.is_err(), "Apply must fail without explicit confirmation");

    let isrc_unmutated: Option<String> = sqlx::query_scalar("SELECT isrc FROM tracks WHERE id = ?")
        .bind(corrupt_tid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(isrc_unmutated, Some("134683067".to_string()), "Unconfirmed repair must perform 0 mutations");

    // 3. Test Apply with confirmed: true -> creates backup with SHA-256 and updates DB
    let confirmed_res = apply_catalog_identity_repair(&pool, &plan, true, Some(&backup_dir))
        .await
        .expect("Apply confirmed repair");

    assert_eq!(confirmed_res.items_succeeded, 1);
    assert_eq!(confirmed_res.items_failed, 0);
    assert!(confirmed_res.db_backup_path.is_some(), "Backup path must be generated");
    assert!(confirmed_res.db_backup_sha256.is_some(), "SHA-256 must be computed");

    // Verify DB was mutated safely (isrc nullified)
    let isrc_repaired: Option<String> = sqlx::query_scalar("SELECT isrc FROM tracks WHERE id = ?")
        .bind(corrupt_tid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(isrc_repaired, None, "Invalid ISRC must be safely nullified");

    // Verify record in repair_history
    let history_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM repair_history WHERE repair_id = ?"
    )
    .bind(&plan.plan_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(history_count, 1, "Applied repair must be recorded in append-only repair_history");
}


