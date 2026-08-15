//! E2E Test Suite for Sprint S104: Dashboard de Estadísticas y Health Checks en Tiempo Real
//!
//! Validates real-time dashboard statistics aggregation, service breakdown,
//! audio quality distribution, lyrics & metadata enrichment coverage, and system health checks.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

async fn create_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations through 0049 must apply cleanly");

    // Seed services & default accounts
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'hires')")
        .execute(&pool).await.unwrap();

    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, is_active, credentials_invalid) VALUES (1, 1, 'Spotify User', 1, 0)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, is_active, credentials_invalid) VALUES (2, 3, 'Tidal User', 1, 0)")
        .execute(&pool).await.unwrap();

    pool
}

#[tokio::test]
async fn test_get_dashboard_stats_empty_and_populated() {
    let db = create_test_db().await;

    // 1. Empty state
    let (t_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks").fetch_one(&db).await.unwrap();
    assert_eq!(t_count, 0);

    // 2. Populate 4 tracks
    // Track 1: With lyrics + enriched + downloaded + favorite
    let t1: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, musicbrainz_id, is_favorite, favorite_at) VALUES ('Track 1', 'mb-1', 1, '2026-08-15T12:00:00Z') RETURNING id"
    ).fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO lyrics (track_id, format, content) VALUES (?, 'plain', 'Some lyrics')").bind(t1).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO downloads (track_id, source_service_id, file_path, file_format) VALUES (?, 3, '/path/1.flac', 'FLAC')")
        .bind(t1).execute(&db).await.unwrap();

    // Track 2: With lyrics only
    let t2: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Track 2') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO lyrics (track_id, format, content) VALUES (?, 'plain', 'More lyrics')").bind(t2).execute(&db).await.unwrap();

    // Track 3: Enriched only
    let _t3: i64 = sqlx::query_scalar("INSERT INTO tracks (title, musicbrainz_id) VALUES ('Track 3', 'mb-3') RETURNING id")
        .fetch_one(&db).await.unwrap();

    // Track 4: Plain
    let _t4: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Track 4') RETURNING id")
        .fetch_one(&db).await.unwrap();

    let (total_tracks,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks").fetch_one(&db).await.unwrap();
    let (total_downloads,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM downloads").fetch_one(&db).await.unwrap();
    let (lyrics_count,): (i64,) = sqlx::query_as("SELECT COUNT(DISTINCT track_id) FROM lyrics WHERE content IS NOT NULL").fetch_one(&db).await.unwrap();
    let (enriched_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tracks WHERE musicbrainz_id IS NOT NULL").fetch_one(&db).await.unwrap();

    assert_eq!(total_tracks, 4);
    assert_eq!(total_downloads, 1);
    assert_eq!(lyrics_count, 2);
    assert_eq!(enriched_count, 2);

    let lyrics_pct = ((lyrics_count as f64) / (total_tracks as f64)) * 100.0;
    let enriched_pct = ((enriched_count as f64) / (total_tracks as f64)) * 100.0;

    assert!((lyrics_pct - 50.0).abs() < f64::EPSILON);
    assert!((enriched_pct - 50.0).abs() < f64::EPSILON);
}

#[tokio::test]
async fn test_dashboard_services_breakdown() {
    let db = create_test_db().await;

    let t1: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Song 1') RETURNING id").fetch_one(&db).await.unwrap();
    let t2: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Song 2') RETURNING id").fetch_one(&db).await.unwrap();

    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 1, 'sp_1')").bind(t1).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 3, 'td_2')").bind(t2).execute(&db).await.unwrap();

    let sp_tracks: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM track_sources WHERE service_id = 1").fetch_one(&db).await.unwrap();
    let td_tracks: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM track_sources WHERE service_id = 3").fetch_one(&db).await.unwrap();

    assert_eq!(sp_tracks.0, 1);
    assert_eq!(td_tracks.0, 1);
}

#[tokio::test]
async fn test_dashboard_quality_distribution() {
    let db = create_test_db().await;

    let t1: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Song 1') RETURNING id").fetch_one(&db).await.unwrap();
    let t2: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Song 2') RETURNING id").fetch_one(&db).await.unwrap();

    sqlx::query("INSERT INTO downloads (track_id, source_service_id, file_path, file_format) VALUES (?, 3, '/path/1.flac', 'FLAC')").bind(t1).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO downloads (track_id, source_service_id, file_path, file_format) VALUES (?, 1, '/path/2.aac', 'AAC')").bind(t2).execute(&db).await.unwrap();

    let quality_rows: Vec<(String, i64)> = sqlx::query_as("SELECT file_format, COUNT(*) FROM downloads GROUP BY file_format ORDER BY file_format ASC")
        .fetch_all(&db).await.unwrap();

    assert_eq!(quality_rows.len(), 2);
    assert_eq!(quality_rows[0].0, "AAC");
    assert_eq!(quality_rows[0].1, 1);
    assert_eq!(quality_rows[1].0, "FLAC");
    assert_eq!(quality_rows[1].1, 1);
}

#[tokio::test]
async fn test_system_health_checks_database_and_services() {
    let db = create_test_db().await;

    let db_ok = sqlx::query("SELECT 1").execute(&db).await.is_ok();
    assert!(db_ok, "Database connection health check must return true");

    let active_accounts: Vec<(String, String)> = sqlx::query_as(
        "SELECT s.name, a.display_name FROM accounts a JOIN services s ON s.id = a.service_id WHERE a.is_active = 1"
    ).fetch_all(&db).await.unwrap();

    assert_eq!(active_accounts.len(), 2);
}

#[tokio::test]
async fn test_service_health_expired_credentials_detection() {
    let db = create_test_db().await;

    // Invalidate Spotify credentials
    sqlx::query("UPDATE accounts SET credentials_invalid = 1 WHERE service_id = 1").execute(&db).await.unwrap();

    let status_row: (bool,) = sqlx::query_as("SELECT credentials_invalid FROM accounts WHERE service_id = 1").fetch_one(&db).await.unwrap();
    assert!(status_row.0, "Credentials invalid status must be true for expired account");
}

#[test]
fn test_print_compiled_migrations() {
    let migrator = sqlx::migrate!("./migrations");
    for m in migrator.migrations.iter() {
        let hex_chk: String = m.checksum.as_ref().iter().map(|b| format!("{:02X}", b)).collect();
        println!("MIGRATION_COMPILED: {} | {} | {}", m.version, m.description, hex_chk);
    }
}

#[tokio::test]
async fn test_physical_database_migration_runs_without_version_mismatch() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite:C:/Users/tardis/AppData/Local/com.syncify.app/syncify.db")
        .await
        .expect("Must connect to physical local syncify.db");

    let res = sqlx::migrate!("./migrations").run(&pool).await;
    assert!(res.is_ok(), "Physical DB migrations must run cleanly without VersionMismatch: {:?}", res.err());
}


