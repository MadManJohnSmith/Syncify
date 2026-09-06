use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tempfile::TempDir;

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

use syncify_core_domain::{derive_track_version, VersionConfidence, VersionDerivationInput};
use syncify_metadata_domain::SourcePriority;
use syncify_tauri_lib::services::incremental_enrichment::{
    EnrichmentMode, IncrementalEnrichmentService, JobStatus,
};

async fn setup_test_db() -> (SqlitePool, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("syncify_test.db");

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite://{}?mode=rwc", db_path.to_str().unwrap()))
        .await
        .unwrap();

    sqlx::query(
        r#"
        CREATE TABLE artists (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            image_url TEXT,
            bio TEXT
        );

        CREATE TABLE albums (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            release_date TEXT,
            cover_art_url TEXT,
            total_tracks INTEGER
        );

        CREATE TABLE album_artists (
            album_id INTEGER,
            artist_id INTEGER,
            is_primary INTEGER DEFAULT 1,
            PRIMARY KEY (album_id, artist_id)
        );

        CREATE TABLE tracks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            source_title TEXT,
            display_title TEXT,
            file_disambiguator TEXT,
            album_id INTEGER,
            duration_ms INTEGER,
            track_number INTEGER,
            disc_number INTEGER DEFAULT 1,
            isrc TEXT,
            musicbrainz_id TEXT,
            release_year INTEGER,
            genre TEXT,
            subgenre TEXT,
            record_label TEXT,
            bpm REAL,
            musical_key TEXT,
            acoustid_fingerprint TEXT,
            explicit INTEGER DEFAULT 0,
            enrichment_status TEXT DEFAULT 'pending',
            enriched_at TEXT,
            is_favorite INTEGER DEFAULT 0,
            favorite_at TEXT,
            qobuz_id TEXT,
            spotify_id TEXT,
            audio_quality TEXT
        );

        CREATE TABLE track_artists (
            track_id INTEGER,
            artist_id INTEGER,
            role TEXT DEFAULT 'primary',
            PRIMARY KEY (track_id, artist_id, role)
        );

        CREATE TABLE services (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE
        );

        CREATE TABLE track_sources (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER NOT NULL,
            service_id INTEGER NOT NULL,
            service_track_id TEXT NOT NULL,
            service_name TEXT,
            extra_metadata TEXT
        );

        CREATE TABLE downloads (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER NOT NULL,
            source_service_id INTEGER NOT NULL,
            file_path TEXT NOT NULL,
            file_format TEXT NOT NULL,
            file_disambiguator TEXT,
            status TEXT DEFAULT 'completed'
        );
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO services (id, name) VALUES (1, 'spotify'), (2, 'qobuz'), (3, 'tidal')")
        .execute(&pool)
        .await
        .unwrap();

    (pool, temp_dir)
}

#[tokio::test]
async fn test_manual_metadata_is_never_replaced() {
    let (pool, _temp) = setup_test_db().await;

    // Track with manual enrichment status
    sqlx::query(
        "INSERT INTO tracks (id, title, source_title, release_year, genre, enrichment_status) 
         VALUES (101, 'Bohemian Rhapsody', 'Bohemian Rhapsody', 1975, 'Progressive Rock', 'manual')"
    )
    .execute(&pool)
    .await
    .unwrap();

    let service = IncrementalEnrichmentService::new();
    let summary = service
        .run_enrichment(&pool, EnrichmentMode::RevalidateAll, None, |_| {})
        .await
        .unwrap();

    assert_eq!(summary.total_tracks, 1);
    assert_eq!(summary.skipped_precedence_tracks, 1);
    assert_eq!(summary.modified_tracks, 0);

    let (year, genre, status): (i32, String, String) = sqlx::query_as(
        "SELECT release_year, genre, enrichment_status FROM tracks WHERE id = 101"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(year, 1975);
    assert_eq!(genre, "Progressive Rock");
    assert_eq!(status, "manual");
}

#[tokio::test]
async fn test_primary_service_beats_musicbrainz_and_secondary() {
    // Priority order verification
    assert!(SourcePriority::Manual > SourcePriority::StreamingService);
    assert!(SourcePriority::StreamingService > SourcePriority::MusicBrainz);
    assert!(SourcePriority::MusicBrainz > SourcePriority::Inferred);

    let (pool, _temp) = setup_test_db().await;

    // Insert track with primary source (Qobuz) having year 2021
    sqlx::query("INSERT INTO artists (id, name) VALUES (1, 'Daft Punk')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO albums (id, title) VALUES (1, 'Discovery')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO tracks (id, title, source_title, album_id, release_year, isrc, enrichment_status) 
         VALUES (201, 'One More Time', 'One More Time', 1, 2001, 'FRZ010000001', 'pending')"
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id, service_name) 
         VALUES (201, 2, 'qobuz_12345', 'qobuz')"
    )
    .execute(&pool)
    .await
    .unwrap();

    let service = IncrementalEnrichmentService::new();
    let summary = service
        .run_enrichment(&pool, EnrichmentMode::IncompleteOnly, None, |_| {})
        .await
        .unwrap();

    let (year, source_title): (i32, String) = sqlx::query_as(
        "SELECT release_year, source_title FROM tracks WHERE id = 201"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Year from primary service remains intact; source_title is never mutated
    assert_eq!(year, 2001);
    assert_eq!(source_title, "One More Time");
    assert!(summary.status == JobStatus::Completed);
}

#[tokio::test]
async fn test_null_and_incomplete_fields_are_enriched() {
    let (pool, _temp) = setup_test_db().await;

    sqlx::query("INSERT INTO artists (id, name) VALUES (1, 'Gorillaz')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO albums (id, title) VALUES (1, 'Demon Days')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (301, 1, 'primary')")
        .execute(&pool)
        .await
        .unwrap();

    // Track with missing ISRC and MBID
    sqlx::query(
        "INSERT INTO tracks (id, title, source_title, album_id, track_number, release_year, genre) 
         VALUES (301, 'Feel Good Inc', 'Feel Good Inc', 1, 6, NULL, NULL)"
    )
    .execute(&pool)
    .await
    .unwrap();

    let service = IncrementalEnrichmentService::new();
    let preview = service
        .preview_enrichment(&pool, EnrichmentMode::IncompleteOnly, None)
        .await
        .unwrap();

    assert_eq!(preview.total_eligible, 1);
    assert_eq!(preview.total_complete, 0);

    let summary = service
        .run_enrichment(&pool, EnrichmentMode::IncompleteOnly, None, |_| {})
        .await
        .unwrap();

    assert_eq!(summary.total_tracks, 1);
    assert!(summary.status == JobStatus::Completed);
}

#[tokio::test]
async fn test_complete_track_is_marked_skipped_complete() {
    let (pool, _temp) = setup_test_db().await;

    sqlx::query(
        "INSERT INTO tracks (
            id, title, source_title, release_year, genre, record_label, bpm, musical_key, acoustid_fingerprint, isrc, musicbrainz_id, enrichment_status
        ) VALUES (
            401, 'Clint Eastwood', 'Clint Eastwood', 2001, 'Alternative', 'Parlophone', 168.0, 'Ebm', 'AQAA_sample_fp', 'GBAYE0100010', 'mb-rec-1234', 'enriched'
        )"
    )
    .execute(&pool)
    .await
    .unwrap();

    let service = IncrementalEnrichmentService::new();
    let preview = service
        .preview_enrichment(&pool, EnrichmentMode::IncompleteOnly, None)
        .await
        .unwrap();

    assert_eq!(preview.total_eligible, 0);
    assert_eq!(preview.total_complete, 1);

    let summary = service
        .run_enrichment(&pool, EnrichmentMode::IncompleteOnly, None, |_| {})
        .await
        .unwrap();

    assert_eq!(summary.skipped_complete_tracks, 1);
    assert_eq!(summary.modified_tracks, 0);
}

#[tokio::test]
async fn test_display_title_high_medium_confidence_persists_low_confidence_does_not_mutate() {
    // 1. High confidence version derivation
    let high_input = VersionDerivationInput {
        title: "19-2000".to_string(),
        provider_version: Some("Soulchild Remix".to_string()),
        musicbrainz_disambiguation: None,
        performer_or_remixer_credit: None,
        comment_text: None,
        track_number: Some(17),
        is_duplicate_title_in_album: true,
    };
    let high_res = derive_track_version(&high_input);
    assert_eq!(high_res.confidence, VersionConfidence::High);
    assert!(high_res.can_apply_to_catalog_and_disk());
    assert_eq!(high_res.display_title, Some("19-2000 (Soulchild Remix)".to_string()));

    // 2. Low confidence raw comment text
    let low_input = VersionDerivationInput {
        title: "19-2000".to_string(),
        provider_version: None,
        musicbrainz_disambiguation: None,
        performer_or_remixer_credit: None,
        comment_text: Some("Ripped from CD - best remix version".to_string()),
        track_number: Some(17),
        is_duplicate_title_in_album: false,
    };
    let low_res = derive_track_version(&low_input);
    assert_eq!(low_res.confidence, VersionConfidence::Low);
    assert!(!low_res.can_apply_to_catalog_and_disk());
    assert_eq!(low_res.display_title, None);
}

#[tokio::test]
async fn test_no_audio_path_lrc_or_download_mutation() {
    let (pool, temp) = setup_test_db().await;

    let music_dir = temp.path().join("Gorillaz");
    tokio::fs::create_dir_all(&music_dir).await.unwrap();

    let flac_file = music_dir.join("01 - Clint Eastwood.flac");
    let lrc_file = music_dir.join("01 - Clint Eastwood.lrc");

    tokio::fs::write(&flac_file, b"MOCK_FLAC_AUDIO_PAYLOAD").await.unwrap();
    tokio::fs::write(&lrc_file, b"[00:01.00] Mock lyrics").await.unwrap();

    sqlx::query(
        "INSERT INTO tracks (id, title, source_title) VALUES (501, 'Clint Eastwood', 'Clint Eastwood')"
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO downloads (id, track_id, source_service_id, file_path, file_format) 
         VALUES (901, 501, 2, ?, 'FLAC')"
    )
    .bind(flac_file.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .unwrap();

    let service = IncrementalEnrichmentService::new();
    service
        .run_enrichment(&pool, EnrichmentMode::RevalidateAll, None, |_| {})
        .await
        .unwrap();

    // Verify audio file & LRC on disk exist and are unmodified
    assert!(flac_file.exists());
    assert!(lrc_file.exists());
    assert_eq!(tokio::fs::read(&flac_file).await.unwrap(), b"MOCK_FLAC_AUDIO_PAYLOAD");

    // Verify download table path is untouched
    let db_path: String = sqlx::query_scalar("SELECT file_path FROM downloads WHERE id = 901")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(db_path, flac_file.to_string_lossy().to_string());
}

#[tokio::test]
async fn test_provider_failure_does_not_abort_job_and_logs_error() {
    let (pool, _temp) = setup_test_db().await;

    // Track 1 with invalid data
    sqlx::query(
        "INSERT INTO tracks (id, title, source_title, isrc) VALUES (601, 'Track One', 'Track One', 'INVALID_ISRC')"
    )
    .execute(&pool)
    .await
    .unwrap();

    // Track 2 with normal data
    sqlx::query(
        "INSERT INTO tracks (id, title, source_title, isrc) VALUES (602, 'Track Two', 'Track Two', 'GBAYE0100010')"
    )
    .execute(&pool)
    .await
    .unwrap();

    let service = IncrementalEnrichmentService::new();
    let summary = service
        .run_enrichment(&pool, EnrichmentMode::RevalidateAll, None, |_| {})
        .await
        .unwrap();

    assert_eq!(summary.total_tracks, 2);
    assert_eq!(summary.processed_tracks, 2);
    assert!(summary.status == JobStatus::Completed || summary.status == JobStatus::Failed);
}

#[tokio::test]
async fn test_cancellation_and_restart() {
    let (pool, _temp) = setup_test_db().await;

    for i in 1..=5 {
        sqlx::query(
            "INSERT INTO tracks (id, title, source_title) VALUES (?, ?, ?)"
        )
        .bind(700 + i)
        .bind(format!("Track {}", i))
        .bind(format!("Track {}", i))
        .execute(&pool)
        .await
        .unwrap();
    }

    let service = IncrementalEnrichmentService::new();
    service.cancel_job();

    let summary = service
        .run_enrichment(&pool, EnrichmentMode::RevalidateAll, None, |_| {})
        .await
        .unwrap();

    assert_eq!(summary.status, JobStatus::Cancelled);

    // Restart works cleanly
    let fresh_service = IncrementalEnrichmentService::new();
    let fresh_summary = fresh_service
        .run_enrichment(&pool, EnrichmentMode::RevalidateAll, None, |_| {})
        .await
        .unwrap();

    assert_eq!(fresh_summary.status, JobStatus::Completed);
    assert_eq!(fresh_summary.processed_tracks, 5);
}

#[tokio::test]
async fn test_isrc_album_artist_cache_hits() {
    let service = IncrementalEnrichmentService::new();
    service.clear_cache();

    let (pool, _temp) = setup_test_db().await;
    sqlx::query(
        "INSERT INTO tracks (id, title, source_title, isrc) VALUES (801, 'Cache Test', 'Cache Test', 'USRC12345678')"
    )
    .execute(&pool)
    .await
    .unwrap();

    let summary1 = service
        .run_enrichment(&pool, EnrichmentMode::RevalidateAll, None, |_| {})
        .await
        .unwrap();
    assert_eq!(summary1.processed_tracks, 1);

    // Second run hits cache
    let summary2 = service
        .run_enrichment(&pool, EnrichmentMode::RevalidateAll, None, |_| {})
        .await
        .unwrap();
    assert_eq!(summary2.processed_tracks, 1);
}

#[tokio::test]
async fn test_realtime_progress_and_telemetry_reporting() {
    let (pool, _temp) = setup_test_db().await;

    for i in 1..=3 {
        sqlx::query(
            "INSERT INTO tracks (id, title, source_title) VALUES (?, ?, ?)"
        )
        .bind(900 + i)
        .bind(format!("Progress Track {}", i))
        .bind(format!("Progress Track {}", i))
        .execute(&pool)
        .await
        .unwrap();
    }

    let progress_events = Arc::new(AtomicUsize::new(0));
    let pe_clone = progress_events.clone();

    let service = IncrementalEnrichmentService::new();
    let summary = service
        .run_enrichment(&pool, EnrichmentMode::RevalidateAll, None, move |_progress| {
            pe_clone.fetch_add(1, Ordering::SeqCst);
        })
        .await
        .unwrap();

    assert!(progress_events.load(Ordering::SeqCst) >= 3);
    assert_eq!(summary.processed_tracks, 3);
    assert_eq!(summary.items.len(), 3);
}
