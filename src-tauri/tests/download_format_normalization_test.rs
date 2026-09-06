//! TASK-66: Test Case-Sensitivity in CHECK Constraint of `downloads.file_format`
//!
//! Verifies:
//! 1. SQLite schema enforces `CHECK(file_format IN ('FLAC', 'ALAC', 'WAV', 'MP3', 'AAC', 'OGG', 'OPUS') OR file_format IS NULL)`.
//! 2. Direct insertion of lowercase formats ("flac", "mp3", "aac") fails with CHECK constraint violation.
//! 3. Normalization logic converts lowercase and trimmed formats to uppercase ("FLAC", "MP3", "AAC", "OPUS").
//! 4. Fallbacks by file extension work as intended for unknown format strings.
//! 5. Inserting normalized formats succeeds and persists correctly in `downloads`.

use sqlx::sqlite::SqlitePoolOptions;
use tempfile::TempDir;

fn normalize_physical_format(eff_fmt: Option<&str>, file_path: &str) -> String {
    let physical_format = eff_fmt
        .unwrap_or("FLAC")
        .trim()
        .to_uppercase();

    let valid_formats = ["FLAC", "AAC", "MP3", "ALAC", "OPUS"];
    if valid_formats.contains(&physical_format.as_str()) {
        physical_format
    } else if file_path.ends_with(".flac") {
        "FLAC".to_string()
    } else if file_path.ends_with(".mp3") {
        "MP3".to_string()
    } else if file_path.ends_with(".m4a") || file_path.ends_with(".aac") {
        "AAC".to_string()
    } else if file_path.ends_with(".opus") {
        "OPUS".to_string()
    } else {
        "FLAC".to_string()
    }
}

#[tokio::test]
async fn test_downloads_format_case_sensitivity_and_normalization() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("format_normalization_test.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Failed to connect to test DB");

    // Run migrations to get the real production schema including downloads CHECK constraint
    let migrator = sqlx::migrate!("./migrations");
    migrator.run(&pool).await.expect("Migrations must run cleanly");

    // Setup base records: service, artist, album, tracks
    sqlx::query("INSERT INTO services (id, name, supports_download, max_quality) VALUES (1, 'qobuz', 1, 'hires'), (2, 'tidal', 1, 'hires') ON CONFLICT(id) DO NOTHING")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO artists (id, name) VALUES (1, 'Artist Normalization Test')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO albums (id, title) VALUES (1, 'Album Normalization Test')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO tracks (id, title, album_id, duration_ms, audio_quality) VALUES 
        (101, 'Track FLAC Lowercase', 1, 180000, 'LOSSLESS'),
        (102, 'Track MP3 Lowercase', 1, 180000, 'LOSSLESS'),
        (103, 'Track AAC Lowercase', 1, 180000, 'LOSSLESS'),
        (104, 'Track OPUS Lowercase', 1, 180000, 'LOSSLESS'),
        (105, 'Track Unknown Fallback', 1, 180000, 'LOSSLESS'),
        (999, 'Track Direct Lowercase Raw', 1, 180000, 'LOSSLESS')")
        .execute(&pool)
        .await
        .unwrap();

    // 1. Verify that raw unnormalized lowercase format FAILS the SQLite CHECK constraint
    let raw_lowercase_insert = sqlx::query(
        "INSERT INTO downloads (track_id, source_service_id, file_path, file_format, downloaded_at)
         VALUES (999, 1, '/music/raw_flac.flac', 'flac', CURRENT_TIMESTAMP)"
    )
    .execute(&pool)
    .await;

    assert!(
        raw_lowercase_insert.is_err(),
        "Direct lowercase 'flac' must fail SQLite CHECK constraint on downloads.file_format"
    );
    let err_str = raw_lowercase_insert.unwrap_err().to_string();
    assert!(
        err_str.to_lowercase().contains("check constraint failed"),
        "Error should be a CHECK constraint failure, got: {}",
        err_str
    );

    // 2. Verify normalization logic for each scenario
    let test_cases = vec![
        (101i64, Some("flac"), "/music/track101.flac", "FLAC"),
        (102i64, Some("mp3"), "/music/track102.mp3", "MP3"),
        (103i64, Some("  aac  "), "/music/track103.m4a", "AAC"),
        (104i64, Some("opus"), "/music/track104.opus", "OPUS"),
        (105i64, Some("unknown_codec"), "/music/track105.flac", "FLAC"),
    ];

    for (track_id, raw_eff_fmt, file_path, expected_persisted) in test_cases {
        let normalized = normalize_physical_format(raw_eff_fmt, file_path);
        assert_eq!(normalized, expected_persisted, "Normalized format should match expected uppercase");

        // 3. Insert using normalized physical_format into downloads
        let insert_res = sqlx::query(
            r#"
            INSERT INTO downloads (
                track_id, source_service_id, file_path, file_format, downloaded_at
            ) VALUES (?, 1, ?, ?, CURRENT_TIMESTAMP)
            "#
        )
        .bind(track_id)
        .bind(file_path)
        .bind(&normalized)
        .execute(&pool)
        .await;

        assert!(
            insert_res.is_ok(),
            "Inserting normalized format '{}' for track {} must succeed without CHECK constraint error: {:?}",
            normalized, track_id, insert_res.err()
        );

        // 4. Verify persisted value in DB
        let stored_format: String = sqlx::query_scalar(
            "SELECT file_format FROM downloads WHERE track_id = ?"
        )
        .bind(track_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(
            stored_format, expected_persisted,
            "Persisted file_format in SQLite must be exact uppercase '{}'", expected_persisted
        );
    }
}
