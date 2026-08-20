//! S159: Repair Integrity Hash Guardrail Integration Tests
//!
//! Validates:
//! 1. unchanged file allows repair (VorbisComment tagging, relocation, DB update, complete report)
//! 2. changed tag/file blocks repair (aborts immediately with RepairInputChanged and 0 mutations)
//! 3. changed audio blocks repair (audio payload mismatch blocks repair)
//! 4. LRC changed blocks coordinated move (sidecar LRC modified/deleted blocks operation)
//! 5. DB update not run when hash mismatch (SQLite records untouched)
//! 6. rollback preserves original (atomic rollback restores exact baseline file bit-for-bit)
//! 7. output report complete (baseline, validation result, applied actions, rollback state, output hashes)

use sqlx::sqlite::SqlitePoolOptions;
use std::path::Path;
use tempfile::TempDir;
use syncify_core_domain::repair::RepairValidationStatus;
use syncify_tauri_lib::services::disambiguation_repair::{
    plan_disambiguation_repair, execute_disambiguation_repair,
};
use syncify_tauri_lib::services::repair_guardrail::{
    compute_file_sha256, compute_repair_baseline, validate_repair_baseline,
};
use syncify_tauri_lib::services::tidal_pipeline::{
    reenrich_download_file, reenrich_download_file_with_baseline,
};

async fn write_test_flac(path: &Path, audio_payload: &[u8]) {
    let mut flac_bytes = Vec::new();
    flac_bytes.extend_from_slice(b"fLaC");
    flac_bytes.extend_from_slice(&[0x80, 0x00, 0x00, 0x22]); // Last block, STREAMINFO, 34 bytes
    let mut streaminfo = [0u8; 34];
    streaminfo[0..2].copy_from_slice(&4608u16.to_be_bytes());
    streaminfo[2..4].copy_from_slice(&4608u16.to_be_bytes());
    streaminfo[10] = 0x0A;
    streaminfo[11] = 0xC4;
    streaminfo[12] = 0x42;
    streaminfo[13] = 0xF0;
    flac_bytes.extend_from_slice(&streaminfo);
    flac_bytes.extend_from_slice(audio_payload);
    tokio::fs::write(path, &flac_bytes).await.expect("Failed to write test flac");
}

async fn create_test_db() -> sqlx::Pool<sqlx::Sqlite> {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::query(
        r#"
        CREATE TABLE services (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            supports_download INTEGER DEFAULT 0,
            max_quality TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        INSERT INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'hires');

        CREATE TABLE folder_settings (
            id INTEGER PRIMARY KEY,
            base_folder TEXT
        );

        CREATE TABLE artists (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            tidal_id INTEGER,
            musicbrainz_id TEXT UNIQUE,
            spotify_id TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE albums (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            artist_id INTEGER REFERENCES artists(id),
            release_date TEXT,
            total_tracks INTEGER,
            cover_art_url TEXT,
            cover_art_path TEXT,
            folder_cover_art_path TEXT,
            folder_cover_art_updated_at TEXT,
            tidal_id INTEGER,
            musicbrainz_id TEXT UNIQUE,
            spotify_id TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE tracks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            album_id INTEGER REFERENCES albums(id) ON DELETE CASCADE,
            title TEXT NOT NULL,
            source_title TEXT,
            display_title TEXT,
            file_disambiguator TEXT,
            track_number INTEGER,
            disc_number INTEGER DEFAULT 1,
            duration_ms INTEGER,
            isrc TEXT,
            musicbrainz_id TEXT,
            spotify_id TEXT,
            tidal_id INTEGER,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE track_artists (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
            artist_id INTEGER NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
            role TEXT NOT NULL DEFAULT 'primary',
            position INTEGER DEFAULT 0,
            UNIQUE(track_id, artist_id, role)
        );

        CREATE TABLE track_sources (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
            service_id INTEGER NOT NULL REFERENCES services(id) ON DELETE CASCADE,
            service_track_id TEXT NOT NULL,
            quality TEXT,
            url TEXT,
            extra_metadata TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(track_id, service_id)
        );

        CREATE TABLE downloads (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
            file_path TEXT NOT NULL,
            file_format TEXT NOT NULL,
            bit_depth INTEGER,
            sample_rate INTEGER,
            file_size_bytes INTEGER,
            duration_ms INTEGER,
            metadata_completeness INTEGER DEFAULT 0,
            lyrics_synced INTEGER DEFAULT 0,
            file_disambiguator TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE album_artists (
            album_id INTEGER NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
            artist_id INTEGER NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
            is_primary INTEGER DEFAULT 1,
            PRIMARY KEY (album_id, artist_id)
        );
        "#
    )
    .execute(&pool)
    .await
    .expect("Failed to initialize test tables");

    pool
}

#[tokio::test]
async fn test_guardrail_unchanged_file_allows_repair() {
    let pool = create_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    let audio_dir = temp_dir.path().join("Unknown Artist").join("Unknown Album");
    tokio::fs::create_dir_all(&audio_dir).await.unwrap();
    let flac_path = audio_dir.join("01 - Tidal Track 134683067.flac");
    let lrc_path = audio_dir.join("01 - Tidal Track 134683067.lrc");

    let audio_payload = b"\xFF\xF8\x18\x00\x00\x00\x00\x00_AUDIO_PAYLOAD_TRACK_50";
    write_test_flac(&flac_path, audio_payload).await;
    tokio::fs::write(&lrc_path, b"[00:01.00] Test Lyrics").await.unwrap();

    // Configure folder settings
    sqlx::query("INSERT INTO folder_settings (id, base_folder) VALUES (1, ?)")
        .bind(temp_dir.path().to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();

    // Insert real metadata target
    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Radiohead') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, artist_id, release_date) VALUES ('OK Computer', ?, '1997-05-21') RETURNING id")
        .bind(artist_id).fetch_one(&pool).await.unwrap();
    let real_track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (album_id, title, track_number, isrc) VALUES (?, 'Airbag', 1, 'GBAYE9700001') RETURNING id")
        .bind(album_id).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(real_track_id).bind(artist_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 3, '134683067')")
        .bind(real_track_id).execute(&pool).await.unwrap();

    // Insert corrupt placeholder track and download row
    let ghost_track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, track_number) VALUES ('Tidal Track 134683067', 1) RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let dl_id: i64 = sqlx::query_scalar("INSERT INTO downloads (track_id, file_path, file_format, metadata_completeness) VALUES (?, ?, 'FLAC', 0) RETURNING id")
        .bind(ghost_track_id).bind(flac_path.to_string_lossy().to_string()).fetch_one(&pool).await.unwrap();

    // 1. Dry run produces valid baseline
    let dry_run_res = reenrich_download_file(&pool, dl_id, true).await.unwrap();
    assert!(dry_run_res.dry_run);
    assert!(dry_run_res.baseline.is_some());
    let baseline = dry_run_res.baseline.unwrap();
    assert_eq!(baseline.file_path, flac_path.to_string_lossy().to_string());
    assert!(baseline.audio_content_hash.is_some());
    assert!(baseline.lrc_sha256.is_some());

    // 2. Apply execution succeeds and preserves audio content payload
    let apply_res = reenrich_download_file(&pool, dl_id, false).await.unwrap();
    assert!(apply_res.success);
    assert!(!apply_res.dry_run);
    assert!(apply_res.moved);
    assert!(apply_res.tags_applied);
    assert!(Path::new(&apply_res.new_path).exists());
    assert!(!flac_path.exists());

    // Verify output hashes report
    let hashes = apply_res.output_hashes.expect("Output hashes must be present");
    assert_eq!(hashes.file_hash_before, baseline.input_sha256);
    assert!(hashes.file_hash_after.is_some());
    // Audio content payload is invariant across VorbisComment tagging
    assert_eq!(hashes.audio_content_hash_before, hashes.audio_content_hash_after);

    // Verify applied actions
    assert!(apply_res.applied_actions.contains(&"validated_baseline".to_string()));
    assert!(apply_res.applied_actions.contains(&"tags_applied".to_string()));
    assert!(apply_res.applied_actions.contains(&"audio_payload_invariance_verified".to_string()));
    assert!(apply_res.applied_actions.contains(&"database_updated".to_string()));
}

#[tokio::test]
async fn test_guardrail_changed_file_blocks_repair_with_zero_mutations() {
    let pool = create_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    let audio_dir = temp_dir.path().join("Unknown Artist").join("Unknown Album");
    tokio::fs::create_dir_all(&audio_dir).await.unwrap();
    let flac_path = audio_dir.join("01 - Tidal Track 134683067.flac");
    write_test_flac(&flac_path, b"ORIGINAL_AUDIO_PAYLOAD").await;

    sqlx::query("INSERT INTO folder_settings (id, base_folder) VALUES (1, ?)")
        .bind(temp_dir.path().to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();

    let ghost_track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, track_number) VALUES ('Tidal Track 134683067', 1) RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let dl_id: i64 = sqlx::query_scalar("INSERT INTO downloads (track_id, file_path, file_format, metadata_completeness) VALUES (?, ?, 'FLAC', 0) RETURNING id")
        .bind(ghost_track_id).bind(flac_path.to_string_lossy().to_string()).fetch_one(&pool).await.unwrap();

    // 1. Dry run baseline
    let dry_run_res = reenrich_download_file(&pool, dl_id, true).await.unwrap();
    let baseline = dry_run_res.baseline.unwrap();

    // 2. External tampering: file modified after dry-run
    tokio::fs::write(&flac_path, b"TAMPERED_CONTENT_AFTER_DRY_RUN").await.unwrap();

    // Baseline validation directly
    let val = validate_repair_baseline(&baseline, &flac_path, None).await;
    assert!(!val.is_valid());
    match val {
        RepairValidationStatus::RepairInputChanged { reason } => {
            assert!(reason.contains("File SHA-256 mismatch") || reason.contains("size changed"));
        }
        other => panic!("Expected RepairInputChanged, got {:?}", other),
    }

    // Apply call with pre-recorded baseline must fail with RepairInputChanged
    let apply_err = reenrich_download_file_with_baseline(&pool, dl_id, false, Some(&baseline)).await.unwrap_err();
    assert!(apply_err.contains("RepairInputChanged"));

    // Verify 0 mutations on database
    let db_path: String = sqlx::query_scalar("SELECT file_path FROM downloads WHERE id = ?")
        .bind(dl_id).fetch_one(&pool).await.unwrap();
    assert_eq!(db_path, flac_path.to_string_lossy().to_string());
}

#[tokio::test]
async fn test_guardrail_changed_audio_payload_blocks_repair() {
    let temp_dir = TempDir::new().unwrap();
    let flac_path = temp_dir.path().join("audio.flac");

    write_test_flac(&flac_path, b"ORIGINAL_AUDIO_FRAMES").await;
    let baseline = compute_repair_baseline(&flac_path, None).await.unwrap();

    // Alter only audio frames (same container header length)
    write_test_flac(&flac_path, b"MODIFIED_AUDIO_FRAMES").await;

    let val = validate_repair_baseline(&baseline, &flac_path, None).await;
    assert!(!val.is_valid());
    match val {
        RepairValidationStatus::RepairInputChanged { reason } => {
            assert!(reason.contains("File SHA-256 mismatch") || reason.contains("Audio content payload changed"));
        }
        other => panic!("Expected RepairInputChanged, got {:?}", other),
    }
}

#[tokio::test]
async fn test_guardrail_lrc_changed_blocks_coordinated_move() {
    let (pool, temp) = setup_disambiguation_db().await;

    let music_dir = temp.path().join("Gorillaz").join("Gorillaz");
    tokio::fs::create_dir_all(&music_dir).await.unwrap();
    let flac_path = music_dir.join("17 - 19-2000.flac");
    let lrc_path = music_dir.join("17 - 19-2000.lrc");

    write_test_flac(&flac_path, b"AUDIO_SOULCHILD_REMIX").await;
    tokio::fs::write(&lrc_path, b"[00:01.00] Original Lyrics").await.unwrap();

    sqlx::query(
        "INSERT INTO tracks (id, title, album_id, track_number, isrc) VALUES (2507, '19-2000', 1, 17, 'GBAYE1400480')"
    ).execute(&pool).await.unwrap();

    sqlx::query(
        "INSERT INTO downloads (track_id, file_path, file_format) VALUES (2507, ?, 'FLAC')"
    ).bind(flac_path.to_string_lossy().to_string()).execute(&pool).await.unwrap();

    // 1. Dry run plan
    let plan = plan_disambiguation_repair(&pool).await.unwrap();
    assert_eq!(plan.items.len(), 1);
    let item = &plan.items[0];
    assert!(item.baseline.is_some());
    let baseline = item.baseline.as_ref().unwrap();
    assert!(baseline.lrc_sha256.is_some());

    // 2. Tamper sidecar LRC after dry run
    tokio::fs::write(&lrc_path, b"[00:01.00] Tampered Lyrics").await.unwrap();

    // 3. Execution must abort item with repair_input_changed and 0 mutations
    let result = execute_disambiguation_repair(&pool, plan).await.unwrap();
    assert_eq!(result.total_renamed, 0);
    assert_eq!(result.items[0].status, "repair_input_changed");
    assert!(result.items[0].rollback_state.as_ref().unwrap().contains("Baseline validation failed"));

    // Verify audio and LRC were NOT renamed
    assert!(flac_path.exists());
    assert!(lrc_path.exists());

    // Verify DB was NOT updated
    let db_path: String = sqlx::query_scalar("SELECT file_path FROM downloads WHERE track_id = 2507")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(db_path, flac_path.to_string_lossy().to_string());
}

#[tokio::test]
async fn test_guardrail_db_update_not_run_when_hash_mismatch() {
    let (pool, temp) = setup_disambiguation_db().await;

    let music_dir = temp.path().join("Gorillaz").join("Gorillaz");
    tokio::fs::create_dir_all(&music_dir).await.unwrap();
    let flac_path = music_dir.join("17 - 19-2000.flac");

    write_test_flac(&flac_path, b"AUDIO_SOULCHILD_REMIX").await;

    sqlx::query(
        "INSERT INTO tracks (id, title, album_id, track_number, isrc) VALUES (2507, '19-2000', 1, 17, 'GBAYE1400480')"
    ).execute(&pool).await.unwrap();

    sqlx::query(
        "INSERT INTO downloads (track_id, file_path, file_format) VALUES (2507, ?, 'FLAC')"
    ).bind(flac_path.to_string_lossy().to_string()).execute(&pool).await.unwrap();

    let plan = plan_disambiguation_repair(&pool).await.unwrap();

    // Tamper file before execute
    write_test_flac(&flac_path, b"CORRUPTED_AUDIO_DIFFERENT_HASH").await;

    let exec_res = execute_disambiguation_repair(&pool, plan).await.unwrap();
    assert_eq!(exec_res.total_renamed, 0);

    // Explicit verification that DB track and download tables were untouched
    let track_title: String = sqlx::query_scalar("SELECT title FROM tracks WHERE id = 2507")
        .fetch_one(&pool).await.unwrap();
    let display_title: Option<String> = sqlx::query_scalar("SELECT display_title FROM tracks WHERE id = 2507")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(track_title, "19-2000");
    assert!(display_title.is_none());
}

#[tokio::test]
async fn test_guardrail_rollback_preserves_original_bit_for_bit() {
    let (pool, temp) = setup_disambiguation_db().await;

    let music_dir = temp.path().join("Gorillaz").join("Gorillaz");
    tokio::fs::create_dir_all(&music_dir).await.unwrap();
    let flac_path = music_dir.join("17 - 19-2000.flac");

    write_test_flac(&flac_path, b"EXACT_INITIAL_AUDIO_BYTES_FOR_ROLLBACK").await;
    let initial_hash = compute_file_sha256(&flac_path).await.unwrap();

    sqlx::query(
        "INSERT INTO tracks (id, title, album_id, track_number, isrc) VALUES (2507, '19-2000', 1, 17, 'GBAYE1400480')"
    ).execute(&pool).await.unwrap();

    sqlx::query(
        "INSERT INTO downloads (track_id, file_path, file_format) VALUES (2507, ?, 'FLAC')"
    ).bind(flac_path.to_string_lossy().to_string()).execute(&pool).await.unwrap();

    let mut plan = plan_disambiguation_repair(&pool).await.unwrap();
    // Simulate invalid target path to provoke failure during rename or post-move verification
    plan.items[0].target_audio_path = "/invalid_nonexistent_root_drive:/impossible/path.flac".to_string();

    let _ = execute_disambiguation_repair(&pool, plan).await;

    // Verify original file exists and hash is 100% identical
    assert!(flac_path.exists());
    let current_hash = compute_file_sha256(&flac_path).await.unwrap();
    assert_eq!(initial_hash, current_hash);
}

#[tokio::test]
async fn test_guardrail_output_report_complete() {
    let (pool, temp) = setup_disambiguation_db().await;

    let music_dir = temp.path().join("Gorillaz").join("Gorillaz");
    tokio::fs::create_dir_all(&music_dir).await.unwrap();
    let flac_path = music_dir.join("17 - 19-2000.flac");
    let lrc_path = music_dir.join("17 - 19-2000.lrc");

    write_test_flac(&flac_path, b"AUDIO_SOULCHILD_REMIX").await;
    tokio::fs::write(&lrc_path, b"[00:01.00] Soulchild lyrics").await.unwrap();

    sqlx::query(
        "INSERT INTO tracks (id, title, album_id, track_number, isrc) VALUES (2507, '19-2000', 1, 17, 'GBAYE1400480')"
    ).execute(&pool).await.unwrap();

    sqlx::query(
        "INSERT INTO downloads (track_id, file_path, file_format) VALUES (2507, ?, 'FLAC')"
    ).bind(flac_path.to_string_lossy().to_string()).execute(&pool).await.unwrap();

    let plan = plan_disambiguation_repair(&pool).await.unwrap();
    let report = execute_disambiguation_repair(&pool, plan).await.unwrap();

    assert_eq!(report.total_renamed, 1);
    let item = &report.items[0];

    // Verify all 5 report components: baseline, validation result, applied actions, rollback state, output hashes
    assert!(item.baseline.is_some(), "Baseline must be recorded");
    let base = item.baseline.as_ref().unwrap();
    assert!(!base.input_sha256.is_empty());
    assert!(base.audio_content_hash.is_some());
    assert!(base.lrc_sha256.is_some());

    assert_eq!(item.status, "repaired_success");
    assert!(item.rollback_state.is_none());

    assert!(item.applied_actions.contains(&"validated_baseline".to_string()));
    assert!(item.applied_actions.contains(&"database_updated".to_string()));

    assert!(item.output_hashes.is_some(), "Output hashes must be present");
    let hashes = item.output_hashes.as_ref().unwrap();
    assert_eq!(hashes.file_hash_before, base.input_sha256);
    assert_eq!(hashes.file_hash_after.as_ref().unwrap(), &hashes.file_hash_before);
    assert_eq!(hashes.audio_content_hash_before, hashes.audio_content_hash_after);
    assert_eq!(hashes.lrc_hash_before, hashes.lrc_hash_after);
}

async fn setup_disambiguation_db() -> (sqlx::SqlitePool, TempDir) {
    let _ = syncify_tauri_lib::crypto::init_keychain_crypto()
        .or_else(|_| syncify_tauri_lib::crypto::init_crypto([42u8; 32]));

    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_guardrail.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (2, 'qobuz', 1, 'hires')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO artists (id, name) VALUES (1, 'Gorillaz')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO albums (id, title) VALUES (1, 'Gorillaz')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (1, 1, 1)")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO tracks (id, title, album_id, track_number, isrc) VALUES (2512, '19-2000', 1, 11, 'GBAYE1400474')")
        .execute(&pool)
        .await
        .unwrap();

    (pool, temp_dir)
}
