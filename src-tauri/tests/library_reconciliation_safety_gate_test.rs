//! Synthetic Integration Test Suite: Sprint S152A - Library Physical Reconciliation Safety Gate
//!
//! Deterministic in-memory/tempdir integration test suite verifying safety gate policies and invariants.
//! Validates:
//! 1. DryRun mode guarantees 0 mutations on SQLite DB and filesystem.
//! 2. `ReportOnly` policy performs discovery without deleting or relinking.
//! 3. `DeleteRecord` policy requires explicit `confirm_delete: true` authorization flag.
//! 4. Ambiguous orphans (title/artist match only or duplicate ISRCs) are NEVER relinked.
//! 5. Exact orphans (unambiguous ISRC, SYNCIFY_TRACK_ID, or valid service_track_id) are safely relinked.
//! 6. Staging purge only touches safe residuals in `.staging`.
//! 7. Account/provider isolation (`source_service_id` and `effective_service`).

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::commands::{
    perform_reconcile_library_physical_state, MissingFilePolicy, OrphanPolicy,
    ReconciliationOptions, ReconciliationScope, StagingPolicy,
};
use tempfile::TempDir;

async fn setup_test_schema(pool: &sqlx::SqlitePool) {
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
        "#,
    )
    .execute(pool)
    .await
    .expect("Failed to create test schema");
}

fn create_dummy_flac(path: &std::path::Path, isrc: Option<&str>, track_id: Option<i64>, title: &str, artist: &str) {
    let mut data = Vec::new();
    data.extend_from_slice(b"fLaC");
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x22]); // STREAMINFO header
    data.extend_from_slice(&[0u8; 34]); // STREAMINFO body (34 bytes)
    data.extend_from_slice(&[0x81, 0x00, 0x00, 0x00]); // PADDING header
    data.extend(vec![0xBB; 1024]);
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
        isrc: isrc.map(|s| s.to_string()),
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
    };

    let _ = syncify_flac_writer::apply_and_verify_flac_tags(path, &meta);

    // If explicit track_id requested, add VorbisComment SYNCIFY_TRACK_ID
    if let Some(tid) = track_id {
        if let Ok(mut flac_tag) = metaflac::Tag::read_from_path(path) {
            let vorbis = flac_tag.vorbis_comments_mut();
            vorbis.set("SYNCIFY_TRACK_ID", vec![tid.to_string()]);
            let _ = flac_tag.save();
        }
    }
}

#[tokio::test]
async fn test_dry_run_never_mutates_db_or_fs() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    setup_test_schema(&pool).await;

    let temp_root = TempDir::new().unwrap();
    let music_dir = temp_root.path().join("Music");
    let staging_dir = music_dir.join(".staging");
    std::fs::create_dir_all(&music_dir).unwrap();
    std::fs::create_dir_all(&staging_dir).unwrap();
    let music_dir_str = music_dir.to_str().unwrap();

    // Seed DB: Track 1 with missing file, Track 2 orphan file on disk
    sqlx::query("INSERT INTO tracks (id, title, isrc) VALUES (1, 'Ghost', 'ISRC001'), (2, 'Physical', 'ISRC002')")
        .execute(&pool)
        .await
        .unwrap();

    let missing_path = music_dir.join("nonexistent.flac");
    sqlx::query("INSERT INTO downloads (id, track_id, source_service_id, file_path) VALUES (10, 1, 2, ?)")
        .bind(missing_path.to_str().unwrap())
        .execute(&pool)
        .await
        .unwrap();

    let orphan_file = music_dir.join("orphan.flac");
    create_dummy_flac(&orphan_file, Some("ISRC002"), None, "Physical", "Artist");
    let staging_file = staging_dir.join("residual.part");
    std::fs::write(&staging_file, b"temp-bytes").unwrap();

    // Execute DryRun with all policies configured
    let opts = ReconciliationOptions {
        dry_run: true,
        scope: ReconciliationScope::All,
        missing_file_policy: MissingFilePolicy::DeleteRecord,
        orphan_policy: OrphanPolicy::RelinkIfExactIdentity,
        staging_policy: StagingPolicy::PurgeSafeResiduals,
        confirm_delete: Some(true),
        base_folder_override: Some(music_dir_str.to_string()),
    };

    let report = perform_reconcile_library_physical_state(&pool, Some(opts))
        .await
        .expect("DryRun must succeed");

    assert!(report.dry_run);
    assert_eq!(report.purged_missing, 0, "DryRun must NOT purge records");
    assert_eq!(report.relinked_orphans, 0, "DryRun must NOT insert records");
    assert_eq!(report.cleaned_staging_residuals, 0, "DryRun must NOT delete staging files");
    assert_eq!(report.missing_files.len(), 1);
    assert_eq!(report.orphan_files.len(), 1);
    assert_eq!(report.planned_actions.len(), 3); // 1 missing delete, 1 orphan relink, 1 staging purge
    assert_eq!(report.executed_actions.len(), 0);

    // Verify DB remains untouched
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads").fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1);

    // Verify Filesystem remains untouched
    assert!(orphan_file.exists());
    assert!(staging_file.exists());
}

#[tokio::test]
async fn test_report_only_policy_does_not_delete_on_apply() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    setup_test_schema(&pool).await;

    let temp_root = TempDir::new().unwrap();
    let music_dir = temp_root.path().join("Music");
    std::fs::create_dir_all(&music_dir).unwrap();
    let music_dir_str = music_dir.to_str().unwrap();

    sqlx::query("INSERT INTO tracks (id, title) VALUES (1, 'Track 1')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO downloads (id, track_id, source_service_id, file_path) VALUES (1, 1, 2, 'C:\\missing.flac')")
        .execute(&pool)
        .await
        .unwrap();

    let opts = ReconciliationOptions {
        dry_run: false,
        scope: ReconciliationScope::All,
        missing_file_policy: MissingFilePolicy::ReportOnly,
        orphan_policy: OrphanPolicy::ReportOnly,
        staging_policy: StagingPolicy::ReportOnly,
        confirm_delete: Some(false),
        base_folder_override: Some(music_dir_str.to_string()),
    };

    let report = perform_reconcile_library_physical_state(&pool, Some(opts)).await.unwrap();
    assert_eq!(report.purged_missing, 0);
    assert_eq!(report.missing_files.len(), 1);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads").fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1, "ReportOnly must preserve downloads table rows");
}

#[tokio::test]
async fn test_delete_requires_explicit_confirmation_flag() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    setup_test_schema(&pool).await;

    let temp_root = TempDir::new().unwrap();
    let music_dir = temp_root.path().join("Music");
    std::fs::create_dir_all(&music_dir).unwrap();
    let music_dir_str = music_dir.to_str().unwrap();

    // In Apply mode with DeleteRecord but confirm_delete: false / None -> Safety gate rejection
    let opts = ReconciliationOptions {
        dry_run: false,
        scope: ReconciliationScope::All,
        missing_file_policy: MissingFilePolicy::DeleteRecord,
        orphan_policy: OrphanPolicy::ReportOnly,
        staging_policy: StagingPolicy::ReportOnly,
        confirm_delete: Some(false), // Rejection!
        base_folder_override: Some(music_dir_str.to_string()),
    };

    let err = perform_reconcile_library_physical_state(&pool, Some(opts))
        .await
        .expect_err("Must reject unconfirmed DeleteRecord in Apply mode");

    assert!(err.contains("Safety gate rejection"));
}

#[tokio::test]
async fn test_ambiguous_orphan_is_never_relinked_without_exact_identity() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    setup_test_schema(&pool).await;

    let temp_root = TempDir::new().unwrap();
    let music_dir = temp_root.path().join("Music");
    std::fs::create_dir_all(&music_dir).unwrap();
    let music_dir_str = music_dir.to_str().unwrap();

    // 1. Seed two tracks with same title/artist but no ISRC
    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Ambiguous Artist') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, artist_id) VALUES ('Album', ?) RETURNING id")
        .bind(artist_id).fetch_one(&pool).await.unwrap();
    
    let tid1: i64 = sqlx::query_scalar("INSERT INTO tracks (id, title, album_id) VALUES (101, 'Same Title', ?) RETURNING id")
        .bind(album_id).fetch_one(&pool).await.unwrap();
    let tid2: i64 = sqlx::query_scalar("INSERT INTO tracks (id, title, album_id) VALUES (102, 'Same Title', ?) RETURNING id")
        .bind(album_id).fetch_one(&pool).await.unwrap();
    
    sqlx::query("INSERT INTO track_artists (track_id, artist_id) VALUES (?, ?), (?, ?)")
        .bind(tid1).bind(artist_id).bind(tid2).bind(artist_id).execute(&pool).await.unwrap();

    // Physical flac with only title & artist tags (no ISRC, no SYNCIFY_TRACK_ID)
    let ambiguous_file = music_dir.join("ambiguous_song.flac");
    create_dummy_flac(&ambiguous_file, None, None, "Same Title", "Ambiguous Artist");

    // Execute Apply with RelinkIfExactIdentity
    let opts = ReconciliationOptions {
        dry_run: false,
        scope: ReconciliationScope::All,
        missing_file_policy: MissingFilePolicy::ReportOnly,
        orphan_policy: OrphanPolicy::RelinkIfExactIdentity,
        staging_policy: StagingPolicy::ReportOnly,
        confirm_delete: Some(false),
        base_folder_override: Some(music_dir_str.to_string()),
    };

    let report = perform_reconcile_library_physical_state(&pool, Some(opts)).await.unwrap();
    assert_eq!(report.relinked_orphans, 0, "Ambiguous orphan must NOT be relinked");
    assert_eq!(report.ambiguous_orphans.len(), 1, "Must be classified as ambiguous orphan");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads").fetch_one(&pool).await.unwrap();
    assert_eq!(count, 0, "No downloads row should be created for ambiguous orphan");
}

#[tokio::test]
async fn test_exact_orphan_relinked_by_isrc_or_track_id() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    setup_test_schema(&pool).await;

    let temp_root = TempDir::new().unwrap();
    let music_dir = temp_root.path().join("Music");
    std::fs::create_dir_all(&music_dir).unwrap();
    let music_dir_str = music_dir.to_str().unwrap();

    // Track A: exact unique ISRC
    sqlx::query("INSERT INTO tracks (id, title, isrc) VALUES (201, 'Exact Track A', 'ISRC_EXACT_201')")
        .execute(&pool).await.unwrap();
    let file_a = music_dir.join("track_a.flac");
    create_dummy_flac(&file_a, Some("ISRC_EXACT_201"), None, "Exact Track A", "Artist A");

    // Track B: exact SYNCIFY_TRACK_ID tag
    sqlx::query("INSERT INTO tracks (id, title) VALUES (202, 'Exact Track B')")
        .execute(&pool).await.unwrap();
    let file_b = music_dir.join("track_b.flac");
    create_dummy_flac(&file_b, None, Some(202), "Different Title", "Artist B");

    // Track C: exact Tidal track ID in filename
    sqlx::query("INSERT INTO tracks (id, title) VALUES (203, 'Tidal Track C')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (203, 3, '99887766')")
        .execute(&pool).await.unwrap();
    let file_c = music_dir.join("01 - Tidal Track 99887766.flac");
    create_dummy_flac(&file_c, None, None, "Tidal Song", "Tidal Artist");

    let opts = ReconciliationOptions {
        dry_run: false,
        scope: ReconciliationScope::All,
        missing_file_policy: MissingFilePolicy::ReportOnly,
        orphan_policy: OrphanPolicy::RelinkIfExactIdentity,
        staging_policy: StagingPolicy::ReportOnly,
        confirm_delete: Some(false),
        base_folder_override: Some(music_dir_str.to_string()),
    };

    let report = perform_reconcile_library_physical_state(&pool, Some(opts)).await.unwrap();
    assert_eq!(report.relinked_orphans, 3, "All 3 exact orphans must be relinked");
    assert_eq!(report.ambiguous_orphans.len(), 0);

    // Verify all 3 are in downloads
    let dls: Vec<(i64, String, i64)> = sqlx::query_as("SELECT track_id, effective_service, source_service_id FROM downloads ORDER BY track_id ASC")
        .fetch_all(&pool)
        .await
        .unwrap();

    assert_eq!(dls.len(), 3);
    assert_eq!(dls[0].0, 201);
    assert_eq!(dls[1].0, 202);
    assert_eq!(dls[2].0, 203);
    assert_eq!(dls[2].1, "tidal");
    assert_eq!(dls[2].2, 3); // tidal service_id
}

#[tokio::test]
async fn test_staging_policy_purges_only_safe_residuals() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    setup_test_schema(&pool).await;

    let temp_root = TempDir::new().unwrap();
    let music_dir = temp_root.path().join("Music");
    let staging_dir = music_dir.join(".staging");
    std::fs::create_dir_all(&music_dir).unwrap();
    std::fs::create_dir_all(&staging_dir).unwrap();
    let music_dir_str = music_dir.to_str().unwrap();

    // Create valid music file outside staging
    let audio_file = music_dir.join("keep_me.flac");
    create_dummy_flac(&audio_file, None, None, "Keep", "Artist");

    // Create residuals inside .staging
    let part_file = staging_dir.join("partial.part");
    let tmp_file = staging_dir.join("cover.webp");
    std::fs::write(&part_file, b"part-bytes").unwrap();
    std::fs::write(&tmp_file, b"webp-bytes").unwrap();

    let opts = ReconciliationOptions {
        dry_run: false,
        scope: ReconciliationScope::All,
        missing_file_policy: MissingFilePolicy::ReportOnly,
        orphan_policy: OrphanPolicy::Ignore,
        staging_policy: StagingPolicy::PurgeSafeResiduals,
        confirm_delete: Some(false),
        base_folder_override: Some(music_dir_str.to_string()),
    };

    let report = perform_reconcile_library_physical_state(&pool, Some(opts)).await.unwrap();
    assert_eq!(report.cleaned_staging_residuals, 2);

    // Staging files deleted
    assert!(!part_file.exists());
    assert!(!tmp_file.exists());
    // Normal music file kept
    assert!(audio_file.exists());
}

#[tokio::test]
async fn test_no_personal_paths_in_source() {
    let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    assert!(src_dir.exists(), "Source directory must exist");

    fn check_dir(dir: &std::path::Path) {
        for entry in std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                check_dir(&path);
            } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                let content = std::fs::read_to_string(&path).unwrap();
                assert!(
                    !content.contains("Syncify-Control-1"),
                    "File {:?} contains prohibited personal path 'Syncify-Control-1'",
                    path
                );
                assert!(
                    !content.contains(r"F:\"),
                    "File {:?} contains prohibited hardcoded personal drive 'F:\\'",
                    path
                );
            }
        }
    }

    check_dir(&src_dir);
}

#[tokio::test]
async fn test_root_resolution_from_folder_settings() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    setup_test_schema(&pool).await;

    let temp_root = TempDir::new().unwrap();
    let music_dir = temp_root.path().join("MusicSettingsFolder");
    std::fs::create_dir_all(&music_dir).unwrap();
    let music_dir_str = music_dir.to_str().unwrap();

    // Configure folder_settings table
    sqlx::query("INSERT INTO folder_settings (id, base_folder) VALUES (1, ?)")
        .bind(music_dir_str)
        .execute(&pool)
        .await
        .unwrap();

    // Create a dummy file in the folder_settings directory
    let audio_file = music_dir.join("test_settings_track.flac");
    create_dummy_flac(&audio_file, Some("TESTSETTINGS123"), None, "Settings Track", "Settings Artist");

    // Reconcile with NO override and scope All => must resolve from folder_settings
    let opts = ReconciliationOptions {
        dry_run: true,
        scope: ReconciliationScope::All,
        missing_file_policy: MissingFilePolicy::ReportOnly,
        orphan_policy: OrphanPolicy::ReportOnly,
        staging_policy: StagingPolicy::ReportOnly,
        confirm_delete: None,
        base_folder_override: None,
    };

    let report = perform_reconcile_library_physical_state(&pool, Some(opts)).await.expect("Must resolve from folder_settings");
    assert_eq!(report.orphan_files.len(), 1, "Must find 1 orphan audio file from folder_settings root");
}

#[tokio::test]
async fn test_explicit_root_resolution_and_selected_root_scope() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    setup_test_schema(&pool).await;

    let temp_root = TempDir::new().unwrap();
    let explicit_dir = temp_root.path().join("ExplicitMusic");
    std::fs::create_dir_all(&explicit_dir).unwrap();
    let explicit_dir_str = explicit_dir.to_str().unwrap();

    // Create a dummy file
    let audio_file = explicit_dir.join("explicit_track.flac");
    create_dummy_flac(&audio_file, Some("EXPLICIT123"), None, "Explicit Track", "Explicit Artist");

    // 1. Via base_folder_override
    let opts_override = ReconciliationOptions {
        dry_run: true,
        scope: ReconciliationScope::All,
        missing_file_policy: MissingFilePolicy::ReportOnly,
        orphan_policy: OrphanPolicy::ReportOnly,
        staging_policy: StagingPolicy::ReportOnly,
        confirm_delete: None,
        base_folder_override: Some(explicit_dir_str.to_string()),
    };

    let report1 = perform_reconcile_library_physical_state(&pool, Some(opts_override)).await.expect("Must resolve explicit override");
    assert_eq!(report1.orphan_files.len(), 1);

    // 2. Via SelectedRoot scope
    let opts_scope = ReconciliationOptions {
        dry_run: true,
        scope: ReconciliationScope::SelectedRoot(explicit_dir_str.to_string()),
        missing_file_policy: MissingFilePolicy::ReportOnly,
        orphan_policy: OrphanPolicy::ReportOnly,
        staging_policy: StagingPolicy::ReportOnly,
        confirm_delete: None,
        base_folder_override: None,
    };

    let report2 = perform_reconcile_library_physical_state(&pool, Some(opts_scope)).await.expect("Must resolve SelectedRoot scope");
    assert_eq!(report2.orphan_files.len(), 1);
}

#[tokio::test]
async fn test_missing_root_returns_error_without_mutations() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    setup_test_schema(&pool).await;

    // Insert a download record
    sqlx::query("INSERT INTO tracks (id, title) VALUES (1, 'Existing Track')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO downloads (id, track_id, source_service_id, file_path) VALUES (1, 1, 2, '/non/existent/path.flac')")
        .execute(&pool).await.unwrap();

    let initial_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads").fetch_one(&pool).await.unwrap();
    assert_eq!(initial_count, 1);

    // Attempt reconciliation pointing to a completely non-existent folder
    let opts = ReconciliationOptions {
        dry_run: false,
        scope: ReconciliationScope::All,
        missing_file_policy: MissingFilePolicy::DeleteRecord,
        orphan_policy: OrphanPolicy::RelinkIfExactIdentity,
        staging_policy: StagingPolicy::PurgeSafeResiduals,
        confirm_delete: Some(true),
        base_folder_override: Some("/this/directory/definitely/does/not/exist/999888".to_string()),
    };

    let result = perform_reconcile_library_physical_state(&pool, Some(opts)).await;
    assert!(result.is_err(), "Must return an explicit error for missing root folder");
    let err_msg = result.err().unwrap();
    assert!(err_msg.contains("does not exist") || err_msg.contains("invalid"), "Error message must be explicit: {}", err_msg);

    // Verify ZERO mutations occurred in DB
    let final_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads").fetch_one(&pool).await.unwrap();
    assert_eq!(final_count, initial_count, "Database must NOT be mutated when root folder is invalid or missing");
}

