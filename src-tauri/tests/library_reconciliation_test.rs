//! Synthetic Integration Test Suite for Sprint S152:
//! Library Physical State vs Downloads SQLite Reconciliation
//!
//! Deterministic in-memory/tempdir integration test suite verifying physical library reconciliation.
//!
//! Validates:
//! 1. Purging historical missing downloads records whose files no longer exist on disk.
//! 2. Re-linking physical orphan audio files in the library directory to `tracks` in DB.
//! 3. Cleaning orphaned staging residuals in `.staging`.
//! 4. Idempotency: Running reconciliation a second time produces 0 purged, 0 relinked, and 100% verified state.

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::commands::{
    perform_reconcile_library_physical_state, MissingFilePolicy, OrphanPolicy,
    ReconciliationOptions, ReconciliationScope, StagingPolicy,
};
use tempfile::TempDir;

#[tokio::test]
async fn test_library_physical_reconciliation_lifecycle() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test database");

    // Run minimal schema setup
    sqlx::query(
        r#"
        CREATE TABLE services (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            supports_download INTEGER DEFAULT 1
        );
        INSERT INTO services (id, name) VALUES (1, 'spotify'), (2, 'qobuz'), (3, 'tidal');

        CREATE TABLE artists (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL
        );

        CREATE TABLE albums (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL,
            artist_id INTEGER,
            cover_art_url TEXT
        );

        CREATE TABLE tracks (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL,
            album_id INTEGER,
            isrc TEXT,
            duration_ms INTEGER,
            download_status TEXT
        );

        CREATE TABLE track_artists (
            id INTEGER PRIMARY KEY,
            track_id INTEGER NOT NULL,
            artist_id INTEGER NOT NULL,
            role TEXT DEFAULT 'primary'
        );

        CREATE TABLE track_sources (
            id INTEGER PRIMARY KEY,
            track_id INTEGER NOT NULL,
            service_id INTEGER NOT NULL,
            service_track_id TEXT,
            available INTEGER DEFAULT 1
        );

        CREATE TABLE downloads (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER UNIQUE REFERENCES tracks(id),
            source_service_id INTEGER REFERENCES services(id),
            file_path TEXT NOT NULL,
            file_format TEXT,
            file_size_bytes INTEGER,
            file_hash TEXT,
            bit_depth INTEGER,
            sample_rate INTEGER,
            metadata_completeness INTEGER DEFAULT 0,
            downloaded_at TEXT DEFAULT CURRENT_TIMESTAMP,
            effective_service TEXT,
            effective_service_track_id TEXT
        );

        CREATE TABLE folder_settings (
            id INTEGER PRIMARY KEY,
            base_folder TEXT NOT NULL
        );
        "#
    )
    .execute(&pool)
    .await
    .expect("Failed to create test schema");

    let temp_root = TempDir::new().expect("Failed to create temporary directory");
    let base_music_dir = temp_root.path().join("Music");
    let staging_dir = base_music_dir.join(".staging");
    std::fs::create_dir_all(&base_music_dir).unwrap();
    std::fs::create_dir_all(&staging_dir).unwrap();

    let base_music_dir_str = base_music_dir.to_str().unwrap();

    sqlx::query("INSERT INTO folder_settings (id, base_folder) VALUES (1, ?)")
        .bind(base_music_dir_str)
        .execute(&pool)
        .await
        .unwrap();

    // 1. Seed tracks in database
    // Track 1: Existing on disk
    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Mid-Air Thief') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, artist_id) VALUES ('Crumbling', ?) RETURNING id")
        .bind(artist_id).fetch_one(&pool).await.unwrap();
    let track_id_1: i64 = sqlx::query_scalar("INSERT INTO tracks (id, title, album_id, isrc) VALUES (19, 'These Chains', ?, 'USEZ61920802') RETURNING id")
        .bind(album_id).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id) VALUES (?, ?)")
        .bind(track_id_1).bind(artist_id).execute(&pool).await.unwrap();

    // Track 2: Missing on disk (should be purged)
    let track_id_2: i64 = sqlx::query_scalar("INSERT INTO tracks (id, title, album_id, isrc) VALUES (999, 'Deleted Track', ?, 'USXX12345678') RETURNING id")
        .bind(album_id).fetch_one(&pool).await.unwrap();

    // Track 3: Orphan physical file on disk (should be re-linked)
    let artist_id_3: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Vito Bambino') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let album_id_3: i64 = sqlx::query_scalar("INSERT INTO albums (title, artist_id) VALUES ('Pracownia', ?) RETURNING id")
        .bind(artist_id_3).fetch_one(&pool).await.unwrap();
    let track_id_3: i64 = sqlx::query_scalar("INSERT INTO tracks (id, title, album_id, isrc) VALUES (25, 'Lekko', ?, 'PLUM72300154') RETURNING id")
        .bind(album_id_3).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id) VALUES (?, ?)")
        .bind(track_id_3).bind(artist_id_3).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 2, '208216979')")
        .bind(track_id_3).execute(&pool).await.unwrap();

    // 2. Create physical files on disk
    fn make_test_flac(path: &std::path::Path, isrc: &str, title: &str, artist: &str) {
        let mut data = Vec::new();
        data.extend_from_slice(b"fLaC");
        data.extend_from_slice(&[0x00, 0x00, 0x00, 0x22]); // STREAMINFO header
        data.extend_from_slice(&[0u8; 34]); // STREAMINFO body (34 bytes)
        data.extend_from_slice(&[0x81, 0x00, 0x00, 0x00]); // PADDING header
        data.extend(vec![0xAA; 1024]);
        std::fs::write(path, &data).unwrap();

        let meta = syncify_flac_writer::FlacMetadata {
            title: title.to_string(),
            artist: artist.to_string(),
            album: "Test Album".to_string(),
            album_artist: None,
            composer: None,
            performers: None,
            work: None,
            genre: None,
            style: None,
            mood: None,
            release_type: None,
            release_status: None,
            release_country: None,
            release_region: None,
            language: None,
            copyright: None,
            label: None,
            barcode: None,
            catalog_number: None,
            original_date: None,
            track_number: 1,
            track_total: 10,
            disc_number: 1,
            disc_total: 1,
            disc_subtitle: None,
            isrc: Some(isrc.to_string()),
            release_year: Some("2026".to_string()),
            release_date: None,
            explicit: None,
            bpm: None,
            initial_key: None,
            energy: None,
            danceability: None,
            loudness: None,
            replaygain_track_gain: None,
            replaygain_track_peak: None,
            replaygain_album_gain: None,
            replaygain_album_peak: None,
            r128_track_gain: None,
            comment: None,
            bit_depth: Some(24),
            sample_rate: Some(48000.0),
            lyrics_lrc: None,
            lyrics_source: None,
            cover_source: None,
            cover_data: None,
            audio_source: Some("Qobuz".to_string()),
            musicbrainz_track_id: None,
            musicbrainz_artist_id: None,
            musicbrainz_album_id: None,
            musicbrainz_albumartist_id: None,
            musicbrainz_release_group_id: None,
            musicbrainz_work_id: None,
            ..Default::default()
        };
        let _ = syncify_flac_writer::apply_and_verify_flac_tags(path, &meta);
    }

    // Real physical file for Track 1
    let track1_dir = base_music_dir.join("Mid-Air Thief").join("Crumbling");
    std::fs::create_dir_all(&track1_dir).unwrap();
    let track1_file = track1_dir.join("02 - These Chains.flac");
    make_test_flac(&track1_file, "USEZ61920802", "These Chains", "Mid-Air Thief");

    // Real physical file for Track 3 (Orphan on disk)
    let track3_dir = base_music_dir.join("Vito Bambino").join("Pracownia");
    std::fs::create_dir_all(&track3_dir).unwrap();
    let track3_file = track3_dir.join("08 - Lekko.flac");
    make_test_flac(&track3_file, "PLUM72300154", "Lekko", "Vito Bambino");

    // Staging residual files
    std::fs::write(staging_dir.join("animated.webp"), b"fake-webp-animation").unwrap();
    std::fs::write(staging_dir.join("folder.webp"), b"fake-webp-folder").unwrap();

    // Seed downloads table:
    // Row 1: Valid (points to track1_file)
    sqlx::query("INSERT INTO downloads (track_id, source_service_id, file_path, file_format, file_size_bytes) VALUES (?, 2, ?, 'FLAC', 1000)")
        .bind(track_id_1).bind(track1_file.to_str().unwrap()).execute(&pool).await.unwrap();

    // Row 2: Missing (points to non-existent file)
    let missing_path = base_music_dir.join("NonExistent").join("ghost_track.flac");
    sqlx::query("INSERT INTO downloads (track_id, source_service_id, file_path, file_format, file_size_bytes) VALUES (?, 2, ?, 'FLAC', 500)")
        .bind(track_id_2).bind(missing_path.to_str().unwrap()).execute(&pool).await.unwrap();

    // Check pre-state
    let count_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads").fetch_one(&pool).await.unwrap();
    assert_eq!(count_before, 2);

    // =========================================================================
    // EXECUTE RECONCILIATION (APPLY MODE WITH EXPLICIT CONFIRMATION)
    // =========================================================================
    let opts = ReconciliationOptions {
        dry_run: false,
        scope: ReconciliationScope::All,
        missing_file_policy: MissingFilePolicy::DeleteRecord,
        orphan_policy: OrphanPolicy::RelinkIfExactIdentity,
        staging_policy: StagingPolicy::PurgeSafeResiduals,
        confirm_delete: Some(true),
        base_folder_override: Some(base_music_dir_str.to_string()),
    };

    let report = perform_reconcile_library_physical_state(&pool, Some(opts.clone()))
        .await
        .expect("Reconciliation must succeed");

    assert_eq!(report.purged_missing, 1, "Must purge 1 missing download record");
    assert_eq!(report.cleaned_staging_residuals, 2, "Must clean 2 staging residual files");
    assert_eq!(report.verified_total, 2, "Verified total must be 2 after re-linking");

    // Verify downloads row for Track 2 (ghost) is removed
    let ghost_dl: Option<(i64,)> = sqlx::query_as("SELECT id FROM downloads WHERE track_id = ?")
        .bind(track_id_2)
        .fetch_optional(&pool)
        .await
        .unwrap();
    assert!(ghost_dl.is_none(), "Missing download row must be purged");

    // Verify downloads row for Track 3 (re-linked) is present
    let relinked_dl: Option<(String, i64)> = sqlx::query_as("SELECT file_path, file_size_bytes FROM downloads WHERE track_id = ?")
        .bind(track_id_3)
        .fetch_optional(&pool)
        .await
        .unwrap();
    assert!(relinked_dl.is_some(), "Orphan track must be re-linked in downloads");

    // Verify staging directory is clean
    let staging_entries: Vec<_> = std::fs::read_dir(&staging_dir).unwrap().filter_map(|e| e.ok()).collect();
    assert!(staging_entries.is_empty(), "Staging directory must have 0 residual files");

    // =========================================================================
    // IDEMPOTENCY CHECK
    // =========================================================================
    let report_second = perform_reconcile_library_physical_state(&pool, Some(opts))
        .await
        .expect("Second reconciliation must succeed");

    assert_eq!(report_second.purged_missing, 0, "Second run must purge 0");
    assert_eq!(report_second.relinked_orphans, 0, "Second run must re-link 0");
    assert_eq!(report_second.cleaned_staging_residuals, 0, "Second run must clean 0");
    assert_eq!(report_second.verified_total, 2, "Verified total must remain 2");
}
