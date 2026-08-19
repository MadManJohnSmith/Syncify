//! Integration Test: S144 25-Track Pilot Validation Gate
//! Validates:
//! - 10 Incomplete tracks
//! - 10 Pre-enriched tracks
//! - 5 Manual tracks (immutable precedence)
//! - 0 downloads mutations (0 path changes, 0 hash changes, 0 audio downloads)
//! - source_title immutability across all tracks
//! - display_title confidence policy
//! - In-memory job lifecycle and cancellation/restart consistency

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use tempfile::TempDir;
use syncify_tauri_lib::services::incremental_enrichment::{
    EnrichmentMode, IncrementalEnrichmentService, JobStatus,
};

async fn setup_pilot_db() -> (SqlitePool, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("pilot_25.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Failed to connect to test pilot DB");

    // Run canonical migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Canonical migrations must run cleanly");

    // Populate Albums & Artists
    sqlx::query("INSERT OR IGNORE INTO artists (id, name) VALUES (1, 'Gorillaz')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT OR IGNORE INTO albums (id, title) VALUES (10, 'Gorillaz Master')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT OR IGNORE INTO album_artists (album_id, artist_id, is_primary) VALUES (10, 1, 1)")
        .execute(&pool)
        .await
        .unwrap();

    // 1. Insert 10 Incomplete tracks (1..=10)
    for i in 1..=10 {
        sqlx::query(
            r#"INSERT INTO tracks (
                id, title, source_title, album_id, track_number, isrc, enrichment_status
            ) VALUES (?, ?, ?, 10, ?, ?, 'pending')"#
        )
        .bind(i)
        .bind(format!("Incomplete Track {}", i))
        .bind(format!("Incomplete Track {}", i))
        .bind(i as i32)
        .bind(format!("GBAYE01000{:02}", i))
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO downloads (id, track_id, source_service_id, file_path, file_hash, file_format) VALUES (?, ?, 2, ?, ?, 'FLAC')"
        )
        .bind(100 + i)
        .bind(i)
        .bind(format!("F:/Music/Gorillaz/Track_{}.flac", i))
        .bind(format!("hash_incomplete_{}", i))
        .execute(&pool)
        .await
        .unwrap();
    }

    // 2. Insert 10 Pre-enriched tracks (11..=20)
    for i in 11..=20 {
        sqlx::query(
            r#"INSERT INTO tracks (
                id, title, source_title, album_id, track_number, isrc, musicbrainz_id,
                release_year, genre, record_label, bpm, musical_key, enrichment_status
            ) VALUES (?, ?, ?, 10, ?, ?, ?, 2001, 'Alternative', 'Parlophone', 120.0, 'Am', 'enriched')"#
        )
        .bind(i)
        .bind(format!("Pre-enriched Track {}", i))
        .bind(format!("Pre-enriched Track {}", i))
        .bind(i as i32)
        .bind(format!("GBAYE01000{:02}", i))
        .bind(format!("mb-rec-pre-{}", i))
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO downloads (id, track_id, source_service_id, file_path, file_hash, file_format) VALUES (?, ?, 2, ?, ?, 'FLAC')"
        )
        .bind(100 + i)
        .bind(i)
        .bind(format!("F:/Music/Gorillaz/Track_{}.flac", i))
        .bind(format!("hash_pre_enriched_{}", i))
        .execute(&pool)
        .await
        .unwrap();
    }

    // 3. Insert 5 Manual metadata tracks (21..=25)
    for i in 21..=25 {
        sqlx::query(
            r#"INSERT INTO tracks (
                id, title, source_title, album_id, track_number, isrc,
                release_year, genre, record_label, bpm, musical_key, enrichment_status
            ) VALUES (?, ?, ?, 10, ?, ?, 1999, 'Custom Manual Genre', 'Manual Records', 135.0, 'F#m', 'manual')"#
        )
        .bind(i)
        .bind(format!("Manual Track Title {}", i))
        .bind(format!("Upstream Source Title {}", i))
        .bind(i as i32)
        .bind(format!("GBAYE01000{:02}", i))
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO downloads (id, track_id, source_service_id, file_path, file_hash, file_format) VALUES (?, ?, 2, ?, ?, 'FLAC')"
        )
        .bind(100 + i)
        .bind(i)
        .bind(format!("F:/Music/Gorillaz/Manual_Track_{}.flac", i))
        .bind(format!("hash_manual_{}", i))
        .execute(&pool)
        .await
        .unwrap();
    }

    (pool, temp_dir)
}

#[tokio::test]
async fn test_pilot_25_tracks_comprehensive_validation() {
    let (pool, _temp) = setup_pilot_db().await;
    let service = IncrementalEnrichmentService::new();

    let pilot_ids: Vec<i64> = (1..=25).collect();

    // === PHASE 1: PREVIEW VALIDATION (NO MUTATION) ===
    let preview = service
        .preview_enrichment(&pool, EnrichmentMode::Selection, Some(pilot_ids.clone()))
        .await
        .expect("Preview must succeed");

    assert_eq!(preview.total_tracks, 25);
    assert_eq!(preview.total_eligible, 10, "Only 10 incomplete tracks are eligible for enrichment");
    assert_eq!(preview.total_complete, 10, "10 pre-enriched tracks are skipped as complete");
    assert_eq!(preview.total_skipped_precedence, 5, "5 manual tracks are protected by precedence");

    // Snapshot Downloads state before execution
    let dl_before: Vec<(i64, String, Option<String>)> =
        sqlx::query_as("SELECT id, file_path, file_hash FROM downloads ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();

    // Snapshot Source Titles before execution
    let titles_before: Vec<(i64, String, Option<String>)> =
        sqlx::query_as("SELECT id, title, source_title FROM tracks ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();

    // === PHASE 2: PILOT EXECUTION (25 TRACKS) ===
    let summary = service
        .run_enrichment(&pool, EnrichmentMode::Selection, Some(pilot_ids.clone()), |_| {})
        .await
        .expect("Pilot enrichment execution must succeed");

    assert_eq!(summary.total_tracks, 25);
    assert_eq!(summary.processed_tracks, 25);
    assert_eq!(summary.skipped_precedence_tracks, 5, "5 manual tracks must be skipped");
    assert!(summary.skipped_complete_tracks >= 10, "At least 10 complete tracks must be skipped");
    assert_eq!(summary.status, JobStatus::Completed);

    // === PHASE 3: STRICT INVARIANTS ASSERTIONS ===

    // 1. Manual tracks (21..=25) are 100% untouched
    for i in 21..=25 {
        let (enrich_status, genre, yr, title, src_title): (String, Option<String>, Option<i32>, String, Option<String>) = sqlx::query_as(
            "SELECT enrichment_status, genre, release_year, title, source_title FROM tracks WHERE id = ?"
        )
        .bind(i)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(enrich_status, "manual", "Track {} must remain manual", i);
        assert_eq!(genre.as_deref(), Some("Custom Manual Genre"));
        assert_eq!(yr, Some(1999));
        assert_eq!(title, format!("Manual Track Title {}", i));
        assert_eq!(src_title.as_deref(), Some(&format!("Upstream Source Title {}", i)[..]));
    }

    // 2. Pre-enriched tracks (11..=20) are unchanged
    for i in 11..=20 {
        let (mbid, yr, genre): (Option<String>, Option<i32>, Option<String>) = sqlx::query_as(
            "SELECT musicbrainz_id, release_year, genre FROM tracks WHERE id = ?"
        )
        .bind(i)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(mbid.as_deref(), Some(&format!("mb-rec-pre-{}", i)[..]));
        assert_eq!(yr, Some(2001));
        assert_eq!(genre.as_deref(), Some("Alternative"));
    }

    // 3. source_title is 100% immutable across all 25 tracks
    let titles_after: Vec<(i64, String, Option<String>)> =
        sqlx::query_as("SELECT id, title, source_title FROM tracks ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(titles_before, titles_after, "source_title must be completely invariant");

    // 4. Downloads table is 100% immutable (0 writes, 0 path changes, 0 hash changes)
    let dl_after: Vec<(i64, String, Option<String>)> =
        sqlx::query_as("SELECT id, file_path, file_hash FROM downloads ORDER BY id")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(dl_before, dl_after, "Downloads table must NOT be mutated during incremental enrichment");

    // === PHASE 4: CANCELLATION & RESTART CONSISTENCY ===
    let fresh_service = IncrementalEnrichmentService::new();
    fresh_service.cancel_job();

    let cancelled_summary = fresh_service
        .run_enrichment(&pool, EnrichmentMode::Selection, Some(pilot_ids.clone()), |_| {})
        .await
        .unwrap();

    assert_eq!(cancelled_summary.status, JobStatus::Cancelled);
    assert_eq!(cancelled_summary.current_phase.as_deref(), Some("Cancelled"));

    // Reset cancellation and restart
    fresh_service.reset_cancellation();
    let restarted_summary = fresh_service
        .run_enrichment(&pool, EnrichmentMode::Selection, Some(pilot_ids.clone()), |_| {})
        .await
        .unwrap();

    assert_eq!(restarted_summary.status, JobStatus::Completed);
    assert_eq!(restarted_summary.processed_tracks, 25);
}
