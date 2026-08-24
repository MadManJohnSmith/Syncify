//! Integration Test: S176Q - Preflight Skip Reasons and Explicit Exclusions
//!
//! Asserts that:
//! 1. Exclusions occur ONLY for explicit rules:
//!    - Track not available on any download provider (Spotify unmapped)
//!    - Track already downloaded in local library
//!    - Explicit user filter (rejected quality under strict mode)
//!    - Provider requiring authenticated account
//! 2. Every excluded track records and reports its exact `skip_reason`.
//! 3. Selected count is never reduced without an explicit preflight exclusion reason.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use syncify_tauri_lib::commands::{perform_enqueue_tracks, perform_reconcile_queue, DownloadPreflightStatus};

async fn create_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations must apply cleanly");

    // Insert services
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (2, 'qobuz', 1, 'hires')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'hires')")
        .execute(&pool).await.unwrap();

    // Insert active accounts for Spotify and Qobuz, NO active account for Tidal (id 3)
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, email, is_active) VALUES (1, 1, 'Spotify User', 'user@spotify.com', 1)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, email, is_active) VALUES (2, 2, 'Qobuz User', 'user@qobuz.com', 1)")
        .execute(&pool).await.unwrap();

    pool
}

#[tokio::test]
async fn test_preflight_skip_reasons_explicit_recording() {
    let db = create_test_db().await;

    // 1. Eligible Track: Qobuz exact source with active account
    let tr_eligible: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Eligible Qobuz Track') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, 'q_elig_01', 'FLAC', 24, 96000, 150, 1)")
        .bind(tr_eligible).execute(&db).await.unwrap();

    // 2. Excluded: No download provider (Spotify track without mapping)
    let tr_no_provider: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Spotify Only Track') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO library_entries (account_id, track_id) VALUES (1, ?)")
        .bind(tr_no_provider).execute(&db).await.unwrap();

    // 3. Excluded: Already downloaded with skip policy active
    let tr_downloaded: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Already Downloaded Track') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, 'q_dl_01', 'FLAC', 24, 96000, 150, 1)")
        .bind(tr_downloaded).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO downloads (track_id, file_path) VALUES (?, 'C:/Music/dl_01.flac')")
        .bind(tr_downloaded).execute(&db).await.unwrap();

    // 4. Excluded: Explicit user filter (rejected quality under strict lossless request)
    let tr_low_quality: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Low Quality Track') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, 'q_lossy_01', 'AAC', 16, 44100, 40, 1)")
        .bind(tr_low_quality).execute(&db).await.unwrap();

    // 5. Excluded: Requires authenticated account (Tidal source but Tidal account is inactive/missing)
    let tr_auth_needed: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Tidal Unauthenticated Track') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 3, 't_unauth_01', 'FLAC', 16, 44100, 100, 1)")
        .bind(tr_auth_needed).execute(&db).await.unwrap();

    let selected_track_ids = vec![tr_eligible, tr_no_provider, tr_downloaded, tr_low_quality, tr_auth_needed];

    // Enqueue with strict quality enabled and skip already downloaded enabled
    let res = perform_enqueue_tracks(
        &db,
        selected_track_ids.clone(),
        Some(50),
        Some("lossless".to_string()),
        None,
        Some(true), // strict_quality
        Some(true),
        Some(true),
        Some(true), // skip_already_downloaded
    )
    .await
    .expect("perform_enqueue_tracks should succeed");

    assert_eq!(res.selected, 5);
    assert_eq!(res.eligible, 1);
    assert_eq!(res.enqueued, 1);
    assert_eq!(res.excluded_preflight.len(), 4);

    // Verify each exclusion has an explicit, non-empty skip_reason and correct status
    let no_prov_excl = res.excluded_preflight.iter().find(|e| e.track_id == tr_no_provider).unwrap();
    assert_eq!(no_prov_excl.status, DownloadPreflightStatus::NoDownloadProvider);
    assert!(!no_prov_excl.skip_reason.is_empty());
    assert!(no_prov_excl.skip_reason.contains("Spotify") || no_prov_excl.skip_reason.contains("No download provider"));

    let dl_excl = res.excluded_preflight.iter().find(|e| e.track_id == tr_downloaded).unwrap();
    assert_eq!(dl_excl.status, DownloadPreflightStatus::AlreadyDownloaded);
    assert!(dl_excl.skip_reason.contains("already downloaded"));

    let lq_excl = res.excluded_preflight.iter().find(|e| e.track_id == tr_low_quality).unwrap();
    assert_eq!(lq_excl.status, DownloadPreflightStatus::RejectedQuality);
    assert!(lq_excl.skip_reason.contains("Quality"));

    let auth_excl = res.excluded_preflight.iter().find(|e| e.track_id == tr_auth_needed).unwrap();
    assert_eq!(auth_excl.status, DownloadPreflightStatus::RequiresAuth);
    assert!(auth_excl.skip_reason.contains("No active account") || auth_excl.skip_reason.contains("account"));

    // Verify reconciliation report
    let recon = perform_reconcile_queue(&db, Some(selected_track_ids))
        .await
        .expect("reconcile_queue should succeed");

    assert_eq!(recon.selected, 5);
    assert_eq!(recon.pending, 1);
    assert_eq!(recon.eligible, 2);
    assert_eq!(recon.excluded_preflight, 3);
    assert_eq!(recon.exclusions.len(), 3);
}
