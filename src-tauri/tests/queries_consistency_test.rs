//! Test suite for TASK-33: Schema and Query Consistency Verification
//!
//! Validates that all queries updated in dashboard.rs, migration.rs,
//! spotify.rs, and disambiguation_repair.rs execute cleanly against a canonical
//! SQLite database initialized with all migrations (sqlx::migrate!).

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

async fn create_migrated_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations must apply cleanly to in-memory SQLite");

    pool
}

#[tokio::test]
async fn test_dashboard_reset_to_defaults_tables_exist() {
    let pool = create_migrated_db().await;

    // Verify all tables referenced by reset_to_defaults ("all")
    let tables = [
        "advanced_settings",
        "sync_settings",
        "folder_settings", // Corrected from folder_file_settings
        "duplicate_settings",
        "audio_processing_settings",
        "lyrics_config",
    ];

    for table in tables {
        let delete_sql = format!("EXPLAIN QUERY PLAN DELETE FROM {} WHERE id = 1", table);
        let result = sqlx::query(&delete_sql).execute(&pool).await;
        assert!(
            result.is_ok(),
            "Table {} must exist and permit DELETE: {:?}",
            table,
            result.err()
        );

        let insert_sql = format!("EXPLAIN QUERY PLAN INSERT OR IGNORE INTO {} (id) VALUES (1)", table);
        let result = sqlx::query(&insert_sql).execute(&pool).await;
        assert!(
            result.is_ok(),
            "Table {} must permit INSERT OR IGNORE: {:?}",
            table,
            result.err()
        );
    }
}

#[tokio::test]
async fn test_dashboard_queries_compile_and_execute() {
    let pool = create_migrated_db().await;

    // 1. get_album_tracks query
    let get_album_tracks_sql = r#"
        EXPLAIN QUERY PLAN
        SELECT 
            t.id, 
            t.title, 
            COALESCE(
                (SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id ORDER BY CASE ta.role WHEN 'primary' THEN 1 WHEN 'main' THEN 2 ELSE 3 END, ta.artist_id ASC LIMIT 1),
                (SELECT a.name FROM album_artists aa JOIN artists a ON a.id = aa.artist_id WHERE aa.album_id = alb.id ORDER BY aa.is_primary DESC, aa.artist_id ASC LIMIT 1)
            ) as artist_name,
            (SELECT ta.artist_id FROM track_artists ta WHERE ta.track_id = t.id ORDER BY CASE ta.role WHEN 'primary' THEN 1 WHEN 'main' THEN 2 ELSE 3 END, ta.artist_id ASC LIMIT 1) as artist_id,
            alb.title as album_name, 
            alb.id as album_id,
            t.duration_ms,
            t.isrc,
            d.file_format as quality,
            CASE WHEN d.file_path IS NOT NULL THEN 'downloaded' ELSE 'not_downloaded' END as download_status,
            t.track_number,
            t.disc_number,
            t.genre,
            t.bpm,
            t.musical_key,
            t.release_year,
            t.explicit,
            t.is_favorite,
            t.favorite_at,
            d.file_path,
            alb.cover_art_url
        FROM tracks t
        JOIN albums alb ON t.album_id = alb.id
        LEFT JOIN downloads d ON d.track_id = t.id
        WHERE alb.title = 'Test' 
          AND (
              EXISTS (
                  SELECT 1 FROM track_artists ta 
                  JOIN artists a ON a.id = ta.artist_id 
                  WHERE ta.track_id = t.id AND a.name = 'Test'
              )
              OR EXISTS (
                  SELECT 1 FROM album_artists aa 
                  JOIN artists a ON a.id = aa.artist_id 
                  WHERE aa.album_id = alb.id AND a.name = 'Test'
              )
              OR 'Test' = ''
          )
        ORDER BY t.disc_number ASC NULLS LAST, t.track_number ASC NULLS LAST, t.title ASC
    "#;

    let res = sqlx::query(get_album_tracks_sql).execute(&pool).await;
    assert!(res.is_ok(), "get_album_tracks query plan failed: {:?}", res.err());

    // 2. get_artist_tracks query
    let get_artist_tracks_sql = r#"
        EXPLAIN QUERY PLAN
        SELECT 
            t.id, 
            t.title, 
            a.name as artist_name, 
            a.id as artist_id,
            alb.title as album_name, 
            alb.id as album_id,
            t.duration_ms,
            t.isrc,
            d.file_format as quality,
            CASE WHEN d.file_path IS NOT NULL THEN 'downloaded' ELSE 'not_downloaded' END as download_status,
            t.track_number,
            t.disc_number,
            t.genre,
            t.bpm,
            t.musical_key,
            t.release_year,
            t.explicit,
            t.is_favorite,
            t.favorite_at,
            d.file_path,
            alb.cover_art_url
        FROM tracks t
        JOIN track_artists ta ON ta.track_id = t.id AND ta.artist_id = 1
        JOIN artists a ON a.id = ta.artist_id
        LEFT JOIN albums alb ON alb.id = t.album_id
        LEFT JOIN downloads d ON d.track_id = t.id
        ORDER BY alb.title NULLS LAST, t.disc_number ASC NULLS LAST, t.track_number ASC NULLS LAST, t.title ASC
    "#;

    let res = sqlx::query(get_artist_tracks_sql).execute(&pool).await;
    assert!(res.is_ok(), "get_artist_tracks query plan failed: {:?}", res.err());

    // 3. get_album_detail query
    let get_album_detail_sql = r#"
        EXPLAIN QUERY PLAN
        SELECT 
            alb.id,
            alb.title,
            COALESCE(
                (SELECT a.name FROM album_artists aa JOIN artists a ON a.id = aa.artist_id WHERE aa.album_id = alb.id ORDER BY aa.is_primary DESC, aa.artist_id ASC LIMIT 1),
                (SELECT a.name FROM track_artists ta JOIN artists a ON a.id = ta.artist_id JOIN tracks tr ON tr.id = ta.track_id WHERE tr.album_id = alb.id ORDER BY CASE ta.role WHEN 'primary' THEN 1 WHEN 'main' THEN 2 ELSE 3 END, ta.artist_id ASC LIMIT 1),
                'Test'
            ) as artist_name,
            COALESCE(CAST(SUBSTR(alb.release_date, 1, 4) AS INTEGER), MIN(t.release_year)) as release_year,
            MIN(t.genre) as genre,
            alb.label,
            COUNT(t.id) as track_count,
            COALESCE(SUM(t.duration_ms), 0) as total_duration_ms,
            alb.cover_art_url
        FROM albums alb
        LEFT JOIN tracks t ON t.album_id = alb.id
        WHERE alb.title = 'Test' 
          AND (
              EXISTS (
                  SELECT 1 FROM album_artists aa 
                  JOIN artists a ON a.id = aa.artist_id 
                  WHERE aa.album_id = alb.id AND a.name = 'Test'
              )
              OR EXISTS (
                  SELECT 1 FROM track_artists ta 
                  JOIN artists a ON a.id = ta.artist_id 
                  JOIN tracks tr ON tr.id = ta.track_id
                  WHERE tr.album_id = alb.id AND a.name = 'Test'
              )
              OR 'Test' = ''
          )
        GROUP BY alb.id, alb.title, alb.release_date, alb.label, alb.cover_art_url
    "#;

    let res = sqlx::query(get_album_detail_sql).execute(&pool).await;
    assert!(res.is_ok(), "get_album_detail query plan failed: {:?}", res.err());
}

#[tokio::test]
async fn test_migration_accounts_credentials_queries() {
    let pool = create_migrated_db().await;

    let services = ["qobuz", "tidal", "spotify", "deezer", "soundcloud"];
    for service_name in services {
        let explain_sql = format!(
            "EXPLAIN QUERY PLAN SELECT a.credentials_json FROM accounts a JOIN services s ON s.id = a.service_id WHERE s.name = '{}' AND a.is_active = 1",
            service_name
        );
        let res = sqlx::query(&explain_sql).execute(&pool).await;
        assert!(
            res.is_ok(),
            "Migration query for {} credentials failed: {:?}",
            service_name,
            res.err()
        );
    }

    // Now insert an account and perform real fetch
    sqlx::query("INSERT OR IGNORE INTO services (id, name) VALUES (1, 'spotify'), (2, 'qobuz')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO accounts (service_id, credentials_json, is_active) VALUES (1, '{\"token\":\"mock_spotify\"}', 1)"
    )
    .execute(&pool)
    .await
    .unwrap();

    let fetched: Option<(String,)> = sqlx::query_as(
        "SELECT a.credentials_json FROM accounts a JOIN services s ON s.id = a.service_id WHERE s.name = 'spotify' AND a.is_active = 1"
    )
    .fetch_optional(&pool)
    .await
    .unwrap();

    assert_eq!(fetched, Some(("{\"token\":\"mock_spotify\"}".to_string(),)));
}

#[tokio::test]
async fn test_spotify_token_persistence_query() {
    let pool = create_migrated_db().await;

    // Query 1: by account_id
    let by_id_sql = "EXPLAIN QUERY PLAN UPDATE accounts SET credentials_json = ?, credentials_invalid = 0, invalid_reason = NULL, last_auth_error = NULL WHERE id = ?";
    let res = sqlx::query(by_id_sql).execute(&pool).await;
    assert!(res.is_ok(), "Spotify update by id failed: {:?}", res.err());

    // Query 2: fallback by service_id
    let by_service_sql = "EXPLAIN QUERY PLAN UPDATE accounts SET credentials_json = ?, credentials_invalid = 0, invalid_reason = NULL, last_auth_error = NULL WHERE service_id = (SELECT id FROM services WHERE name = 'spotify')";
    let res = sqlx::query(by_service_sql).execute(&pool).await;
    assert!(res.is_ok(), "Spotify update by service failed: {:?}", res.err());

    // Test real execution
    sqlx::query("INSERT OR IGNORE INTO services (id, name) VALUES (1, 'spotify')")
        .execute(&pool)
        .await
        .unwrap();
    let account_id: i64 = sqlx::query_scalar(
        "INSERT INTO accounts (service_id, credentials_json, is_active) VALUES (1, 'initial', 1) RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let update_res = sqlx::query(
        "UPDATE accounts SET credentials_json = ?, credentials_invalid = 0, invalid_reason = NULL, last_auth_error = NULL WHERE id = ?"
    )
    .bind("updated_token_cipher")
    .bind(account_id)
    .execute(&pool)
    .await;

    assert!(update_res.is_ok());
    let (saved,): (String,) = sqlx::query_as("SELECT credentials_json FROM accounts WHERE id = ?")
        .bind(account_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(saved, "updated_token_cipher");
}

#[tokio::test]
async fn test_disambiguation_repair_queries() {
    let pool = create_migrated_db().await;

    // 1. Downloaded tracks query with t.file_disambiguator
    let tracks_sql = r#"
        EXPLAIN QUERY PLAN
        SELECT 
            t.id, 
            t.title, 
            t.isrc, 
            al.title as album_title, 
            t.musicbrainz_id,
            t.track_number, 
            t.album_id, 
            d.file_path,
            t.file_disambiguator
        FROM tracks t
        JOIN albums al ON al.id = t.album_id
        JOIN downloads d ON d.track_id = t.id
        WHERE d.file_path IS NOT NULL
    "#;
    let res = sqlx::query(tracks_sql).execute(&pool).await;
    assert!(res.is_ok(), "Downloaded tracks query failed: {:?}", res.err());

    // 2. Remixer credit query
    let remixer_sql = r#"
        EXPLAIN QUERY PLAN
        SELECT a.name 
        FROM track_artists ta 
        JOIN artists a ON a.id = ta.artist_id 
        WHERE ta.track_id = 1 AND (ta.role LIKE '%remix%' OR ta.role LIKE '%performer%') 
        LIMIT 1
    "#;
    let res = sqlx::query(remixer_sql).execute(&pool).await;
    assert!(res.is_ok(), "Remixer query failed: {:?}", res.err());

    // 3. Downloads update query
    let dl_update_sql = "EXPLAIN QUERY PLAN UPDATE downloads SET file_path = ?, file_disambiguator = ? WHERE track_id = ?";
    let res = sqlx::query(dl_update_sql).execute(&pool).await;
    assert!(res.is_ok(), "Downloads update failed: {:?}", res.err());

    // 4. Tracks update query
    let trk_update_sql = "EXPLAIN QUERY PLAN UPDATE tracks SET display_title = ?, source_title = ?, file_disambiguator = ? WHERE id = ?";
    let res = sqlx::query(trk_update_sql).execute(&pool).await;
    assert!(res.is_ok(), "Tracks update failed: {:?}", res.err());
}
