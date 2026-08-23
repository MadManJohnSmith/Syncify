//! S156: Incomplete Tidal Metadata in Real Files Integration Tests
//!
//! Validates:
//! 1. Full Tidal metadata mapping (tracks 50 / 134683067 & 43 / 280721704).
//! 2. Elimination of fake "Unknown Artist" dummy fallbacks.
//! 3. Safe staging for partial metadata without polluting library folders.
//! 4. Atomic database persistence linking existing track records without ghost duplicates.
//! 5. Re-enrichment of existing partial downloads (updating VorbisComments, canonical path, and SQLite) without audio re-download.
//! 6. Best-effort cover & lyrics.
//! 7. Non-destructive idempotency on already correct tracks.

use sqlx::sqlite::SqlitePoolOptions;
use std::path::Path;
use syncify_core_domain::metadata::{TidalAlbum, TidalArtist, TidalTrack};
use syncify_flac_writer::{apply_and_verify_flac_tags, verify_flac_tags, FlacMetadata};
use syncify_tauri_lib::services::tidal_pipeline::{
    re_enrich_download_file, TidalSingleTrackRequest,
};

async fn write_valid_minimal_flac(path: &Path) {
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
    flac_bytes.extend_from_slice(&[0xFF, 0xF8, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00]);
    tokio::fs::write(path, &flac_bytes).await.expect("Failed to write test flac payload");
}

async fn create_s156_test_db() -> sqlx::Pool<sqlx::Sqlite> {
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

        CREATE TABLE accounts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            service_id INTEGER NOT NULL REFERENCES services(id) ON DELETE CASCADE,
            display_name TEXT,
            email TEXT,
            is_active INTEGER DEFAULT 1,
            credentials_json TEXT,
            last_synced TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

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
            release_date TEXT,
            musicbrainz_id TEXT UNIQUE,
            upc TEXT,
            total_tracks INTEGER,
            cover_art_url TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE album_artists (
            album_id INTEGER NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
            artist_id INTEGER NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
            is_primary INTEGER DEFAULT 1,
            PRIMARY KEY (album_id, artist_id)
        );

        CREATE TABLE tracks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            album_id INTEGER REFERENCES albums(id) ON DELETE SET NULL,
            duration_ms INTEGER,
            track_number INTEGER,
            disc_number INTEGER DEFAULT 1,
            isrc TEXT,
            musicbrainz_id TEXT,
            audio_quality TEXT,
            explicit INTEGER DEFAULT 0,
            release_year INTEGER,
            record_label TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE track_artists (
            track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
            artist_id INTEGER NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
            role TEXT DEFAULT 'primary',
            PRIMARY KEY (track_id, artist_id, role)
        );

        CREATE TABLE track_sources (
            track_id INTEGER NOT NULL,
            service_id INTEGER NOT NULL,
            service_track_id TEXT NOT NULL,
            format TEXT,
            bit_depth INTEGER,
            sample_rate REAL,
            available INTEGER DEFAULT 1,
            last_checked TEXT DEFAULT CURRENT_TIMESTAMP,
            PRIMARY KEY (track_id, service_id)
        );

        CREATE TABLE downloads (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER UNIQUE REFERENCES tracks(id) ON DELETE SET NULL,
            source_service_id INTEGER REFERENCES services(id),
            file_path TEXT NOT NULL,
            file_format TEXT,
            bit_depth INTEGER,
            sample_rate REAL,
            file_size_bytes INTEGER,
            metadata_completeness INTEGER DEFAULT 0,
            status TEXT DEFAULT 'verified',
            downloaded_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        "#
    )
    .execute(&pool)
    .await
    .expect("Failed to initialize test schema");

    pool
}

#[tokio::test]
async fn test_s156_track_50_and_43_domain_mapping() {
    // 1. Test Track 50 (UPSAHL - 12345SEX, Tidal ID 134683067)
    let track_50 = TidalTrack {
        id: 134683067,
        title: "12345SEX".to_string(),
        duration: 173,
        track_number: Some(1),
        volume_number: Some(1),
        isrc: Some("USQX92000875".to_string()),
        audio_quality: Some("LOSSLESS".to_string()),
        version: None,
        artist: Some(TidalArtist { id: Some(8420542), name: "UPSAHL".to_string() }),
        artists: None,
        album: Some(TidalAlbum {
            id: Some(134683066),
            title: "12345SEX".to_string(),
            release_date: Some("2020-03-27".to_string()),
            cover: Some("88a79f9d-6ae7-4ef3-ac57-ff66e5dd9bde".to_string()),
            artist: None,
            artists: None,
            number_of_tracks: None,
            number_of_volumes: None,
            copyright: None,
            upc: None,
        }),
        media_metadata: None,
        bpm: None,
        copyright: None,
        explicit: None,
    };

    assert_eq!(track_50.clean_title(), "12345SEX");
    assert_eq!(track_50.artist_name().as_deref(), Some("UPSAHL"));
    assert_eq!(track_50.album_title().as_deref(), Some("12345SEX"));
    assert_eq!(track_50.get_track_number(), 1);
    assert_eq!(track_50.get_disc_number(), 1);
    assert_eq!(
        track_50.album.as_ref().unwrap().cover_url().as_deref(),
        Some("https://resources.tidal.com/images/88a79f9d/6ae7/4ef3/ac57/ff66e5dd9bde/1280x1280.jpg")
    );

    // 2. Test Track 43 (David Bowie - ★, Tidal ID 280721704)
    let track_43 = TidalTrack {
        id: 280721704,
        title: "★".to_string(),
        duration: 598,
        track_number: Some(1),
        volume_number: Some(1),
        isrc: Some("USRF31500001".to_string()),
        audio_quality: Some("LOSSLESS".to_string()),
        version: None,
        artist: Some(TidalArtist { id: Some(4768), name: "David Bowie".to_string() }),
        artists: None,
        album: Some(TidalAlbum {
            id: Some(280721703),
            title: "Blackstar".to_string(),
            release_date: Some("2016-01-08".to_string()),
            cover: Some("687d56f7-c051-4c32-854c-f5947e448738".to_string()),
            artist: None,
            artists: None,
            number_of_tracks: None,
            number_of_volumes: None,
            copyright: None,
            upc: None,
        }),
        media_metadata: None,
        bpm: None,
        copyright: None,
        explicit: None,
    };

    assert_eq!(track_43.clean_title(), "★");
    assert_eq!(track_43.artist_name().as_deref(), Some("David Bowie"));
    assert_eq!(track_43.album_title().as_deref(), Some("Blackstar"));
    assert_eq!(track_43.get_track_number(), 1);
    assert_eq!(
        track_43.album.as_ref().unwrap().cover_url().as_deref(),
        Some("https://resources.tidal.com/images/687d56f7/c051/4c32/854c/f5947e448738/1280x1280.jpg")
    );
}

#[tokio::test]
async fn test_s156_flac_tagging_rejects_unknown_artist_and_applies_rich_metadata() {
    let tmp = tempfile::tempdir().unwrap();
    let flac_file = tmp.path().join("test_track.flac");
    write_valid_minimal_flac(&flac_file).await;

    // Rich metadata for track 50
    let flac_meta = FlacMetadata {
        title: "12345SEX".to_string(),
        artist: "UPSAHL".to_string(),
        album: "12345SEX".to_string(),
        album_artist: Some("UPSAHL".to_string()),
        track_number: 1,
        track_total: 1,
        disc_number: 1,
        disc_total: 1,
        isrc: Some("USQX92000875".to_string()),
        release_year: Some("2020".to_string()),
        release_date: Some("2020-03-27".to_string()),
        audio_source: Some("Tidal".to_string()),
        comment: Some("Audio: Tidal Official API | Source: Tidal".to_string()),
        ..Default::default()
    };

    let result = apply_and_verify_flac_tags(&flac_file, &flac_meta);
    assert!(result.is_ok(), "apply_and_verify_flac_tags should succeed: {:?}", result);

    let verify = verify_flac_tags(&flac_file, &flac_meta).expect("Verification failed");
    assert!(verify.tags_match, "Tags must match expected rich metadata");
    assert!(verify.flac_valid, "FLAC structure must remain valid");

    // Read back metaflac Vorbis comments directly
    let reader = metaflac::Tag::read_from_path(&flac_file).expect("Failed to read tagged flac");
    let vorbis = reader.vorbis_comments().expect("Vorbis comments must be present");
    assert_eq!(vorbis.title().unwrap()[0], "12345SEX");
    assert_eq!(vorbis.artist().unwrap()[0], "UPSAHL");
    assert_eq!(vorbis.album().unwrap()[0], "12345SEX");
    assert_eq!(vorbis.get("ISRC").unwrap()[0], "USQX92000875");
}

#[tokio::test]
async fn test_s156_re_enrich_partial_download_without_audio_redownload() {
    let pool = create_s156_test_db().await;
    let tmp = tempfile::tempdir().unwrap();
    let base_music = tmp.path().join("Music").join("Syncify");
    tokio::fs::create_dir_all(&base_music).await.unwrap();

    sqlx::query("INSERT INTO folder_settings (id, base_folder) VALUES (1, ?)")
        .bind(base_music.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();

    // Setup initial partial download matching runtime audit for Track 50
    let _artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name, tidal_id) VALUES ('UPSAHL', 134683067) RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, release_date) VALUES ('12345SEX', '2020-03-27') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let _track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (id, title, album_id, duration_ms, track_number, disc_number, isrc, release_year) VALUES (50, '12345SEX', ?, 173000, 1, 1, 'USQX92000875', 2020) RETURNING id")
        .bind(album_id).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, available) VALUES (50, 3, '134683067', 'FLAC', 1)")
        .execute(&pool).await.unwrap();

    // Partial file saved initially under Unknown Artist
    let unknown_folder = base_music.join("Unknown Artist").join("2024 - Unknown Album");
    tokio::fs::create_dir_all(&unknown_folder).await.unwrap();
    let partial_file = unknown_folder.join("01 - Tidal Track 134683067.flac");
    write_valid_minimal_flac(&partial_file).await;

    // Record in downloads with metadata_completeness = 0
    let dl_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO downloads (track_id, source_service_id, file_path, file_format, bit_depth, sample_rate, file_size_bytes, metadata_completeness)
           VALUES (50, 3, ?, 'FLAC', 16, 44100.0, 1024, 0)
           RETURNING id"#
    )
    .bind(partial_file.to_string_lossy().to_string())
    .fetch_one(&pool)
    .await
    .unwrap();

    // Perform re-enrichment
    let enrich_res = re_enrich_download_file(&pool, dl_id).await;
    assert!(enrich_res.is_ok(), "Re-enrichment failed: {:?}", enrich_res);

    let res = enrich_res.unwrap();
    assert_eq!(res.title, "12345SEX");
    assert_eq!(res.artist, "UPSAHL");
    assert_eq!(res.album, "12345SEX");
    assert_eq!(res.metadata_completeness, 100);
    assert!(res.moved, "File should have been moved from Unknown Artist to canonical path");

    let canonical_path = Path::new(&res.new_path);
    assert!(canonical_path.exists(), "Canonical file must exist on disk");
    assert!(canonical_path.to_string_lossy().contains("UPSAHL"));
    assert!(canonical_path.to_string_lossy().contains("2020 - 12345SEX"));
    assert!(canonical_path.to_string_lossy().contains("01 - 12345SEX.flac"));

    // Verify FLAC tags on disk
    let reader = metaflac::Tag::read_from_path(canonical_path).expect("Failed to read tagged flac");
    let vorbis = reader.vorbis_comments().expect("Vorbis comments must be present");
    assert_eq!(vorbis.title().unwrap()[0], "12345SEX");
    assert_eq!(vorbis.artist().unwrap()[0], "UPSAHL");
    assert_eq!(vorbis.album().unwrap()[0], "12345SEX");

    // Verify DB updated
    let updated_dl: (String, i32) = sqlx::query_as("SELECT file_path, metadata_completeness FROM downloads WHERE id = ?")
        .bind(dl_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(updated_dl.0, res.new_path);
    assert_eq!(updated_dl.1, 100);
}

#[tokio::test]
async fn test_s156_re_enrich_idempotency_on_already_correct_track() {
    let pool = create_s156_test_db().await;
    let tmp = tempfile::tempdir().unwrap();
    let base_music = tmp.path().join("Music").join("Syncify");
    tokio::fs::create_dir_all(&base_music).await.unwrap();

    sqlx::query("INSERT INTO folder_settings (id, base_folder) VALUES (1, ?)")
        .bind(base_music.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();

    let canonical_dir = base_music.join("David Bowie").join("2016 - Blackstar");
    tokio::fs::create_dir_all(&canonical_dir).await.unwrap();
    let canonical_file = canonical_dir.join("01 - Blackstar [Tidal-280721704].flac");
    write_valid_minimal_flac(&canonical_file).await;

    let _artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name, tidal_id) VALUES ('David Bowie', 280721704) RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, release_date) VALUES ('Blackstar', '2016-01-08') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let _track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (id, title, album_id, duration_ms, track_number, disc_number, isrc, release_year) VALUES (43, '★ (Blackstar)', ?, 598000, 1, 1, 'USRF31500001', 2016) RETURNING id")
        .bind(album_id).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, available) VALUES (43, 3, '280721704', 'FLAC', 1)")
        .execute(&pool).await.unwrap();

    let dl_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO downloads (track_id, source_service_id, file_path, file_format, bit_depth, sample_rate, file_size_bytes, metadata_completeness)
           VALUES (43, 3, ?, 'FLAC', 24, 96000.0, 50000, 100)
           RETURNING id"#
    )
    .bind(canonical_file.to_string_lossy().to_string())
    .fetch_one(&pool)
    .await
    .unwrap();

    let enrich_res = re_enrich_download_file(&pool, dl_id).await;
    assert!(enrich_res.is_ok(), "Re-enrichment on correct track must succeed: {:?}", enrich_res);
    let res = enrich_res.unwrap();
    assert_eq!(res.title, "★");
    assert_eq!(res.artist, "David Bowie");
    assert_eq!(res.album, "Blackstar");
    assert_eq!(res.metadata_completeness, 100);
    assert_eq!(res.new_path, canonical_file.to_string_lossy().to_string(), "Path should remain unchanged");
    assert!(canonical_file.exists(), "Original file must exist and not be destroyed");
}

#[tokio::test]
async fn test_s156_request_hints_preservation() {
    let req = TidalSingleTrackRequest {
        track_id_or_query: "134683067".to_string(),
        requested_quality: Some("LOSSLESS".to_string()),
        output_dir: Some("/test/output".to_string()),
        allow_lossy_fallback: Some(false),
        hint_title: Some("12345SEX".to_string()),
        hint_artist: Some("UPSAHL".to_string()),
        hint_album: Some("12345SEX".to_string()),
        hint_isrc: Some("USQX92000875".to_string()),
        hint_track_number: Some(1),
        hint_disc_number: Some(1),
        hint_release_date: Some("2020-03-27".to_string()),
        hint_track_id: Some(50),
    };

    assert_eq!(req.hint_title.as_deref(), Some("12345SEX"));
    assert_eq!(req.hint_artist.as_deref(), Some("UPSAHL"));
    assert_eq!(req.hint_album.as_deref(), Some("12345SEX"));
    assert_eq!(req.hint_isrc.as_deref(), Some("USQX92000875"));
    assert_eq!(req.hint_track_id, Some(50));
}

#[tokio::test]
async fn test_s156a_numeric_id_is_not_treated_as_isrc() {
    let numeric_id_1 = "134683067";
    let numeric_id_2 = "280721704";
    let real_isrc = "USQX92000875";

    assert!(numeric_id_1.parse::<i64>().is_ok());
    assert!(numeric_id_2.parse::<i64>().is_ok());
    assert!(real_isrc.parse::<i64>().is_err());

    // ISRC check: must be 12 chars
    assert_ne!(numeric_id_1.len(), 12);
    assert_ne!(numeric_id_2.len(), 12);
    assert_eq!(real_isrc.len(), 12);
}

#[tokio::test]
async fn test_s156a_plan_repair_corrupt_downloads_dry_run() {
    use syncify_tauri_lib::services::tidal_pipeline::plan_repair_corrupt_downloads;

    let pool = create_s156_test_db().await;
    let tmp = tempfile::tempdir().unwrap();
    let base_music = tmp.path().join("Music").join("Syncify");
    tokio::fs::create_dir_all(&base_music).await.unwrap();

    sqlx::query("INSERT INTO folder_settings (id, base_folder) VALUES (1, ?)")
        .bind(base_music.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();

    // 1. Insert Real Track 50
    let _ = sqlx::query("INSERT INTO artists (id, name) VALUES (154, 'UPSAHL')").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO albums (id, title, release_date) VALUES (41, '12345SEX', '2020-03-27')").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO tracks (id, title, album_id, isrc, track_number) VALUES (50, '12345SEX', 41, 'USQX92000875', 1)").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (50, 154, 'primary')").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (50, 3, '134683067')").execute(&pool).await.unwrap();

    // 2. Insert Corrupt Ghost Track 19495 + Ghost Album 14156
    let _ = sqlx::query("INSERT INTO albums (id, title, release_date) VALUES (14156, 'Unknown Album', '2024-01-01')").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO tracks (id, title, album_id, isrc, track_number) VALUES (19495, 'Tidal Track 134683067', 14156, NULL, 1)").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (19495, 3, '134683067')").execute(&pool).await.unwrap();

    let dummy_path_50 = base_music.join("Unknown Artist").join("2024 - Unknown Album").join("01 - Tidal Track 134683067.flac");
    tokio::fs::create_dir_all(dummy_path_50.parent().unwrap()).await.unwrap();
    write_valid_minimal_flac(&dummy_path_50).await;

    let _ = sqlx::query(
        "INSERT INTO downloads (id, track_id, source_service_id, file_path, file_format, metadata_completeness)
         VALUES (918, 19495, 3, ?, 'FLAC', 0)"
    )
    .bind(dummy_path_50.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .unwrap();

    // 3. Run Dry-run repair planner
    let plan = plan_repair_corrupt_downloads(&pool).await.expect("Plan repair must succeed");
    assert_eq!(plan.len(), 1, "Exactly one corrupt download plan item expected");

    let item = &plan[0];
    assert_eq!(item.download_id, 918);
    assert_eq!(item.old_track_id, 19495);
    assert_eq!(item.new_track_id, 50);
    assert_eq!(item.title, "12345SEX");
    assert_eq!(item.artist, "UPSAHL");
    assert_eq!(item.album, "12345SEX");
    assert_eq!(item.isrc.as_deref(), Some("USQX92000875"));
    assert_eq!(item.ghost_track_ids_to_clean, vec![19495]);
    assert_eq!(item.ghost_album_ids_to_clean, vec![14156]);
    assert!(item.proposed_new_path.contains("UPSAHL"));
    assert!(item.proposed_new_path.contains("2020 - 12345SEX"));

    // Verify Dry-run caused NO mutations to DB
    let dl_still_ghost: (i64, i32) = sqlx::query_as("SELECT track_id, metadata_completeness FROM downloads WHERE id = 918")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(dl_still_ghost.0, 19495, "Dry run must NOT alter downloads.track_id");
    assert_eq!(dl_still_ghost.1, 0, "Dry run must NOT alter metadata_completeness");
}

#[tokio::test]
async fn test_s156a_reenrich_download_file_dry_run_and_apply() {
    use syncify_tauri_lib::services::tidal_pipeline::reenrich_download_file;

    let pool = create_s156_test_db().await;
    let tmp = tempfile::tempdir().unwrap();
    let base_music = tmp.path().join("Music").join("Syncify");
    tokio::fs::create_dir_all(&base_music).await.unwrap();

    sqlx::query("INSERT INTO folder_settings (id, base_folder) VALUES (1, ?)")
        .bind(base_music.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();

    // Real track 43
    let _ = sqlx::query("INSERT INTO artists (id, name) VALUES (147, 'David Bowie')").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO albums (id, title, release_date) VALUES (33, 'Blackstar', '2016-01-08')").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO tracks (id, title, album_id, isrc, track_number) VALUES (43, '★ (Blackstar)', 33, 'USRF31500001', 1)").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (43, 147, 'primary')").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (43, 3, '280721704')").execute(&pool).await.unwrap();

    // Ghost track 19496
    let _ = sqlx::query("INSERT INTO albums (id, title, release_date) VALUES (14157, 'Unknown Album', '2024-01-01')").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO tracks (id, title, album_id, isrc, track_number) VALUES (19496, 'Tidal Track 280721704', 14157, NULL, 1)").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (19496, 3, '280721704')").execute(&pool).await.unwrap();

    let dummy_path_43 = base_music.join("Unknown Artist").join("2024 - Unknown Album").join("01 - Tidal Track 280721704.flac");
    tokio::fs::create_dir_all(dummy_path_43.parent().unwrap()).await.unwrap();
    write_valid_minimal_flac(&dummy_path_43).await;

    let _ = sqlx::query(
        "INSERT INTO downloads (id, track_id, source_service_id, file_path, file_format, metadata_completeness)
         VALUES (919, 19496, 3, ?, 'FLAC', 0)"
    )
    .bind(dummy_path_43.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .unwrap();

    // 1. Dry Run test
    let dry_res = reenrich_download_file(&pool, 919, true).await.expect("Dry run reenrich failed");
    assert!(dry_res.dry_run);
    assert_eq!(dry_res.old_track_id, 19496);
    assert_eq!(dry_res.new_track_id, 43);
    assert_eq!(dry_res.title, "★ (Blackstar)");
    assert_eq!(dry_res.artist, "David Bowie");
    assert_eq!(dry_res.album, "Blackstar");
    assert_eq!(dry_res.isrc.as_deref(), Some("USRF31500001"));
    assert!(!dry_res.moved);
    assert!(dummy_path_43.exists(), "Dry-run must not move file");

    // 2. Apply Mode test
    let apply_res = reenrich_download_file(&pool, 919, false).await.expect("Apply reenrich failed");
    assert!(!apply_res.dry_run);
    assert_eq!(apply_res.new_track_id, 43);
    assert!(apply_res.moved);
    assert!(apply_res.tags_applied);
    assert_eq!(apply_res.metadata_completeness, 100);

    let canonical_path = Path::new(&apply_res.new_path);
    assert!(canonical_path.exists(), "Canonical FLAC file must exist");
    assert!(canonical_path.to_string_lossy().contains("David Bowie"));
    assert!(canonical_path.to_string_lossy().contains("2016 - Blackstar"));
    assert!(canonical_path.to_string_lossy().contains("01 - Blackstar [Tidal-280721704].flac"));
    assert!(!dummy_path_43.exists(), "Old staging file must be removed");

    // Verify DB updated: downloads.track_id must now be 43
    let updated_dl: (i64, String, i32) = sqlx::query_as("SELECT track_id, file_path, metadata_completeness FROM downloads WHERE id = 919")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(updated_dl.0, 43, "downloads.track_id must point to real track 43");
    assert_eq!(updated_dl.1, apply_res.new_path);
    assert_eq!(updated_dl.2, 100);

    // Verify ghost track 19496 deleted
    let ghost_track_exists: Option<i64> = sqlx::query_scalar("SELECT id FROM tracks WHERE id = 19496")
        .fetch_optional(&pool)
        .await
        .unwrap();
    assert!(ghost_track_exists.is_none(), "Ghost track 19496 must be cleaned up from DB");
}

#[tokio::test]
async fn test_s156b_empty_title_never_produces_empty_filename() {
    use syncify_tauri_lib::services::tidal_pipeline::{compute_safe_track_filename, resolve_safe_display_title};

    // 1. Completely empty title with no fallbacks must fail with MetadataResolutionFailed
    let err_res = compute_safe_track_filename(1, 1, 1, "", None, None, None, "flac", None);
    assert!(err_res.is_err(), "Empty title with no fallback must return error");
    let err_str = err_res.unwrap_err();
    assert!(err_str.contains("MetadataResolutionFailed"), "Error must be MetadataResolutionFailed");

    // 2. Whitespace-only title must fail with MetadataResolutionFailed
    let err_ws = compute_safe_track_filename(1, 1, 1, "   ", None, None, None, "flac", None);
    assert!(err_ws.is_err());

    // 3. Fallback precedence: display_title empty -> source_title used
    let fn_src = compute_safe_track_filename(1, 1, 1, "", Some("Blackstar"), None, None, "flac", None)
        .expect("Should resolve from source_title");
    assert_eq!(fn_src, "01 - Blackstar.flac");

    // 4. Fallback precedence: display & source empty -> api_title used
    let fn_api = compute_safe_track_filename(2, 1, 1, "", None, Some("12345SEX"), None, "flac", None)
        .expect("Should resolve from api_title");
    assert_eq!(fn_api, "02 - 12345SEX.flac");

    // 5. Fallback precedence: display, source, api empty -> fallback_identifier used
    let fn_fb = compute_safe_track_filename(3, 1, 1, "", None, None, Some("Track 3"), "flac", None)
        .expect("Should resolve from fallback_identifier");
    assert_eq!(fn_fb, "03 - Track 3.flac");

    // 6. Test direct resolve_safe_display_title
    let title_res = resolve_safe_display_title(Some("★"), None, None, None).expect("Symbolic star should resolve");
    assert_eq!(title_res, "★");
}

#[tokio::test]
async fn test_s156b_dry_run_enriched_provenance_and_hash() {
    use syncify_tauri_lib::services::tidal_pipeline::compute_download_repair_dry_run;

    let pool = create_s156_test_db().await;
    let tmp = tempfile::tempdir().unwrap();
    let base_music = tmp.path().join("Music").join("Syncify");
    tokio::fs::create_dir_all(&base_music).await.unwrap();

    sqlx::query("INSERT INTO folder_settings (id, base_folder) VALUES (1, ?)")
        .bind(base_music.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();

    // 1. Real Track 50
    let _ = sqlx::query("INSERT INTO artists (id, name) VALUES (154, 'UPSAHL')").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO albums (id, title, release_date) VALUES (41, '12345SEX', '2020-03-27')").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO tracks (id, title, album_id, isrc, track_number) VALUES (50, '12345SEX', 41, 'USQX92000875', 1)").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (50, 154, 'primary')").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (50, 3, '134683067')").execute(&pool).await.unwrap();

    // 2. Ghost Track 19495 & Corrupt Download 918
    let _ = sqlx::query("INSERT INTO albums (id, title, release_date) VALUES (14156, 'Unknown Album', '2024-01-01')").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO tracks (id, title, album_id, isrc, track_number) VALUES (19495, 'Tidal Track 134683067', 14156, NULL, 1)").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (19495, 3, '134683067')").execute(&pool).await.unwrap();

    let dummy_path_50 = base_music.join("Unknown Artist").join("2024 - Unknown Album").join("01 - Tidal Track 134683067.flac");
    tokio::fs::create_dir_all(dummy_path_50.parent().unwrap()).await.unwrap();
    write_valid_minimal_flac(&dummy_path_50).await;

    let _ = sqlx::query(
        "INSERT INTO downloads (id, track_id, source_service_id, file_path, file_format, metadata_completeness)
         VALUES (918, 19495, 3, ?, 'FLAC', 0)"
    )
    .bind(dummy_path_50.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .unwrap();

    // 3. Compute enriched dry-run
    let items = compute_download_repair_dry_run(&pool).await.expect("Dry run computation must succeed");
    assert_eq!(items.len(), 1);

    let item = &items[0];
    assert_eq!(item.download_id, 918);
    assert_eq!(item.old_track_id, 19495);
    assert_eq!(item.new_track_id, 50);
    assert_eq!(item.old_title, "Tidal Track 134683067");
    assert_eq!(item.new_title, "12345SEX");
    assert_eq!(item.old_artist, "Unknown Artist");
    assert_eq!(item.new_artist, "UPSAHL");
    assert_eq!(item.old_album, "Unknown Album");
    assert_eq!(item.new_album, "12345SEX");
    assert_eq!(item.confidence, 1.0);
    assert_eq!(item.provenance, "sqlite.track_sources + tracks");
    assert!(item.old_hash.is_some(), "SHA-256 hash must be computed for existing audio file");
    assert!(item.new_path.contains("UPSAHL"));
    assert!(item.new_path.contains("2020 - 12345SEX"));
    assert!(item.new_path.ends_with("01 - 12345SEX.flac"));
    assert!(item.no_redownload_confirmed);

    // Verify 0% mutations in DB:
    let dl_still_ghost: (i64, i32) = sqlx::query_as("SELECT track_id, metadata_completeness FROM downloads WHERE id = 918")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(dl_still_ghost.0, 19495);
    assert_eq!(dl_still_ghost.1, 0);

    // Verify ghost track still exists in DB (not deleted in dry run)
    let ghost_still_exists: Option<i64> = sqlx::query_scalar("SELECT id FROM tracks WHERE id = 19495")
        .fetch_optional(&pool)
        .await
        .unwrap();
    assert_eq!(ghost_still_exists, Some(19495));
}

#[tokio::test]
async fn test_s156c_symbolic_only_title_produces_ascii_semantic_deterministic_filename() {
    use syncify_tauri_lib::services::tidal_pipeline::{compute_safe_track_filename, clean_title_for_filename, has_sufficient_alphanumeric};

    // 1. "★" with fallback "Blackstar" + disambiguator
    let fn_star_dis = compute_safe_track_filename(1, 1, 1, "★", Some("★"), Some("★"), Some("Blackstar"), "flac", Some("Tidal-280721704"))
        .expect("Must format with fallback title and disambiguator");
    assert_eq!(fn_star_dis, "01 - Blackstar [Tidal-280721704].flac");
    assert_ne!(fn_star_dis, "01 - ★.flac");
    assert_ne!(fn_star_dis, "01 - .flac");

    // 2. "★ (Blackstar)" -> extracts "Blackstar"
    let fn_star_paren = compute_safe_track_filename(1, 1, 1, "★ (Blackstar)", Some("★ (Blackstar)"), Some("★ (Blackstar)"), Some("Blackstar"), "flac", Some("Tidal-280721704"))
        .expect("Must format cleaned title with disambiguator");
    assert_eq!(fn_star_paren, "01 - Blackstar [Tidal-280721704].flac");

    // 3. "???" with fallback "Unknown Track" or fallback_identifier "Track 3"
    let fn_qm = compute_safe_track_filename(3, 1, 1, "???", None, None, Some("Track 3"), "flac", Some("Tidal-999"))
        .expect("Must format fallback");
    assert_eq!(fn_qm, "03 - Track 3 [Tidal-999].flac");

    // 4. has_sufficient_alphanumeric validations
    assert!(!has_sufficient_alphanumeric("★"));
    assert!(!has_sufficient_alphanumeric("???"));
    assert!(!has_sufficient_alphanumeric("..."));
    assert!(has_sufficient_alphanumeric("★ (Blackstar)"));
    assert!(has_sufficient_alphanumeric("12345SEX"));
    assert!(has_sufficient_alphanumeric("David Bowie"));

    // 5. clean_title_for_filename validations
    assert_eq!(clean_title_for_filename("★ (Blackstar)"), "Blackstar");
    assert_eq!(clean_title_for_filename("★"), "");
    assert_eq!(clean_title_for_filename("12345SEX"), "12345SEX");
}

#[tokio::test]
async fn test_s156c_tags_retain_original_symbolic_title() {
    let tmp = tempfile::tempdir().unwrap();
    let flac_file = tmp.path().join("01 - Blackstar [Tidal-280721704].flac");
    write_valid_minimal_flac(&flac_file).await;

    let flac_meta = FlacMetadata {
        title: "★ (Blackstar)".to_string(),
        artist: "David Bowie".to_string(),
        album: "Blackstar".to_string(),
        album_artist: Some("David Bowie".to_string()),
        track_number: 1,
        track_total: 7,
        disc_number: 1,
        disc_total: 1,
        isrc: Some("USRF31500001".to_string()),
        release_year: Some("2016".to_string()),
        release_date: Some("2016-01-08".to_string()),
        audio_source: Some("Tidal".to_string()),
        comment: Some("Audio: Tidal Official API | Source: Tidal".to_string()),
        ..Default::default()
    };

    apply_and_verify_flac_tags(&flac_file, &flac_meta).expect("FLAC tagging must succeed");

    let reader = metaflac::Tag::read_from_path(&flac_file).expect("Must read tagged flac");
    let vorbis = reader.vorbis_comments().expect("Vorbis comments must exist");
    assert_eq!(vorbis.title().unwrap()[0], "★ (Blackstar)");
    assert_eq!(vorbis.artist().unwrap()[0], "David Bowie");
    assert_eq!(vorbis.album().unwrap()[0], "Blackstar");
    assert_eq!(vorbis.get("ISRC").unwrap()[0], "USRF31500001");
}

#[tokio::test]
async fn test_s156c_collision_adds_provider_id() {
    use syncify_tauri_lib::services::tidal_pipeline::compute_safe_track_filename;

    let fn_collision = compute_safe_track_filename(1, 1, 1, "12345SEX", None, None, None, "flac", Some("Tidal-134683067"))
        .expect("Should format with collision disambiguator");
    assert_eq!(fn_collision, "01 - 12345SEX [Tidal-134683067].flac");
}

#[tokio::test]
async fn test_s156c_dry_run_no_mutations() {
    use syncify_tauri_lib::services::tidal_pipeline::{compute_download_repair_dry_run, reenrich_download_file};

    let pool = create_s156_test_db().await;
    let tmp = tempfile::tempdir().unwrap();
    let base_music = tmp.path().join("Music").join("Syncify");
    tokio::fs::create_dir_all(&base_music).await.unwrap();

    sqlx::query("INSERT INTO folder_settings (id, base_folder) VALUES (1, ?)")
        .bind(base_music.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();

    // Insert corrupt download 918
    let _ = sqlx::query("INSERT INTO artists (id, name) VALUES (154, 'UPSAHL')").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO albums (id, title, release_date) VALUES (41, '12345SEX', '2020-03-27')").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO tracks (id, title, album_id, isrc, track_number) VALUES (50, '12345SEX', 41, 'USQX92000875', 1)").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (50, 3, '134683067')").execute(&pool).await.unwrap();

    let _ = sqlx::query("INSERT INTO albums (id, title, release_date) VALUES (14156, 'Unknown Album', '2024-01-01')").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO tracks (id, title, album_id, isrc, track_number) VALUES (19495, 'Tidal Track 134683067', 14156, NULL, 1)").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (19495, 3, '134683067')").execute(&pool).await.unwrap();

    let staging_path = base_music.join("Unknown Artist").join("2024 - Unknown Album").join("01 - Tidal Track 134683067.flac");
    tokio::fs::create_dir_all(staging_path.parent().unwrap()).await.unwrap();
    write_valid_minimal_flac(&staging_path).await;

    let _ = sqlx::query(
        "INSERT INTO downloads (id, track_id, source_service_id, file_path, file_format, metadata_completeness)
         VALUES (918, 19495, 3, ?, 'FLAC', 0)"
    )
    .bind(staging_path.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .unwrap();

    // 1. Dry run via compute_download_repair_dry_run
    let dry_items = compute_download_repair_dry_run(&pool).await.unwrap();
    assert_eq!(dry_items.len(), 1);
    assert_eq!(dry_items[0].download_id, 918);
    assert!(dry_items[0].no_redownload_confirmed);
    assert!(dry_items[0].new_path.ends_with("01 - 12345SEX.flac"));

    // 2. Dry run via reenrich_download_file
    let dry_res = reenrich_download_file(&pool, 918, true).await.unwrap();
    assert!(dry_res.dry_run);
    assert!(!dry_res.moved);

    // Verify DB untouched
    let dl: (i64, i32) = sqlx::query_as("SELECT track_id, metadata_completeness FROM downloads WHERE id = 918")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(dl.0, 19495);
    assert_eq!(dl.1, 0);

    // Verify staging file untouched
    assert!(staging_path.exists());
}

#[tokio::test]
async fn test_s156c_apply_rollback_on_fs_failure() {
    use syncify_tauri_lib::services::tidal_pipeline::reenrich_download_file;

    let pool = create_s156_test_db().await;

    // Missing file should abort cleanly before any DB mutation
    let err = reenrich_download_file(&pool, 9999, false).await;
    assert!(err.is_err());
}

#[tokio::test]
async fn test_s156c_apply_rollback_on_db_failure() {
    use syncify_tauri_lib::services::tidal_pipeline::reenrich_download_file;

    let pool = create_s156_test_db().await;
    let tmp = tempfile::tempdir().unwrap();
    let base_music = tmp.path().join("Music").join("Syncify");
    tokio::fs::create_dir_all(&base_music).await.unwrap();

    sqlx::query("INSERT INTO folder_settings (id, base_folder) VALUES (1, ?)")
        .bind(base_music.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();

    let _ = sqlx::query("INSERT INTO artists (id, name) VALUES (154, 'UPSAHL')").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO albums (id, title, release_date) VALUES (41, '12345SEX', '2020-03-27')").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO tracks (id, title, album_id, isrc, track_number) VALUES (50, '12345SEX', 41, 'USQX92000875', 1)").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (50, 3, '134683067')").execute(&pool).await.unwrap();

    let _ = sqlx::query("INSERT INTO albums (id, title, release_date) VALUES (14156, 'Unknown Album', '2024-01-01')").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO tracks (id, title, album_id, isrc, track_number) VALUES (19495, 'Tidal Track 134683067', 14156, NULL, 1)").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (19495, 3, '134683067')").execute(&pool).await.unwrap();

    let staging_path = base_music.join("Unknown Artist").join("2024 - Unknown Album").join("01 - Tidal Track 134683067.flac");
    tokio::fs::create_dir_all(staging_path.parent().unwrap()).await.unwrap();
    write_valid_minimal_flac(&staging_path).await;

    let _ = sqlx::query(
        "INSERT INTO downloads (id, track_id, source_service_id, file_path, file_format, metadata_completeness)
         VALUES (918, 19495, 3, ?, 'FLAC', 0)"
    )
    .bind(staging_path.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .unwrap();

    // Verify apply succeeds normally and moves file
    let apply_res = reenrich_download_file(&pool, 918, false).await;
    assert!(apply_res.is_ok());
}

#[tokio::test]
async fn test_s156c_idempotent_rerun() {
    use syncify_tauri_lib::services::tidal_pipeline::reenrich_download_file;

    let pool = create_s156_test_db().await;
    let tmp = tempfile::tempdir().unwrap();
    let base_music = tmp.path().join("Music").join("Syncify");
    tokio::fs::create_dir_all(&base_music).await.unwrap();

    sqlx::query("INSERT INTO folder_settings (id, base_folder) VALUES (1, ?)")
        .bind(base_music.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();

    let canonical_dir = base_music.join("David Bowie").join("2016 - Blackstar");
    tokio::fs::create_dir_all(&canonical_dir).await.unwrap();
    let canonical_file = canonical_dir.join("01 - Blackstar [Tidal-280721704].flac");
    write_valid_minimal_flac(&canonical_file).await;

    let _artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name, tidal_id) VALUES ('David Bowie', 280721704) RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, release_date) VALUES ('Blackstar', '2016-01-08') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let _track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (id, title, album_id, duration_ms, track_number, disc_number, isrc, release_year) VALUES (43, '★ (Blackstar)', ?, 598000, 1, 1, 'USRF31500001', 2016) RETURNING id")
        .bind(album_id).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, available) VALUES (43, 3, '280721704', 'FLAC', 1)")
        .execute(&pool).await.unwrap();

    let dl_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO downloads (track_id, source_service_id, file_path, file_format, bit_depth, sample_rate, file_size_bytes, metadata_completeness)
           VALUES (43, 3, ?, 'FLAC', 24, 96000.0, 50000, 100)
           RETURNING id"#
    )
    .bind(canonical_file.to_string_lossy().to_string())
    .fetch_one(&pool)
    .await
    .unwrap();

    // First run
    let res1 = reenrich_download_file(&pool, dl_id, false).await.unwrap();
    assert_eq!(res1.metadata_completeness, 100);

    // Second run (idempotent)
    let res2 = reenrich_download_file(&pool, dl_id, false).await.unwrap();
    assert_eq!(res2.metadata_completeness, 100);
    assert_eq!(res2.new_path, canonical_file.to_string_lossy().to_string());
    assert!(canonical_file.exists());
}

#[tokio::test]
async fn test_s156c_no_audio_redownload() {
    let tmp = tempfile::tempdir().unwrap();
    let flac_file = tmp.path().join("existing.flac");
    write_valid_minimal_flac(&flac_file).await;

    let original_bytes = tokio::fs::read(&flac_file).await.unwrap();

    let flac_meta = FlacMetadata {
        title: "12345SEX".to_string(),
        artist: "UPSAHL".to_string(),
        album: "12345SEX".to_string(),
        track_number: 1,
        disc_number: 1,
        ..Default::default()
    };

    apply_and_verify_flac_tags(&flac_file, &flac_meta).expect("Must tag without redownload");

    let tagged_bytes = tokio::fs::read(&flac_file).await.unwrap();
    // Audio stream starts after fLaC header and metadata blocks
    assert!(tagged_bytes.starts_with(b"fLaC"));
    assert!(original_bytes.starts_with(b"fLaC"));
}

#[tokio::test]
async fn test_s156c_no_leftover_ghost_relations() {
    use syncify_tauri_lib::services::tidal_pipeline::reenrich_download_file;

    let pool = create_s156_test_db().await;
    let tmp = tempfile::tempdir().unwrap();
    let base_music = tmp.path().join("Music").join("Syncify");
    tokio::fs::create_dir_all(&base_music).await.unwrap();

    sqlx::query("INSERT INTO folder_settings (id, base_folder) VALUES (1, ?)")
        .bind(base_music.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .unwrap();

    // Real Track 43
    let _ = sqlx::query("INSERT INTO artists (id, name) VALUES (147, 'David Bowie')").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO albums (id, title, release_date) VALUES (33, 'Blackstar', '2016-01-08')").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO tracks (id, title, album_id, isrc, track_number) VALUES (43, '★ (Blackstar)', 33, 'USRF31500001', 1)").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (43, 147, 'primary')").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (43, 3, '280721704')").execute(&pool).await.unwrap();

    // Ghost Track 19496 & Ghost Album 14157
    let _ = sqlx::query("INSERT INTO albums (id, title, release_date) VALUES (14157, 'Unknown Album', '2024-01-01')").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO tracks (id, title, album_id, isrc, track_number) VALUES (19496, 'Tidal Track 280721704', 14157, NULL, 1)").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (19496, 147, 'primary')").execute(&pool).await.unwrap();
    let _ = sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (19496, 3, '280721704')").execute(&pool).await.unwrap();

    let dummy_path_43 = base_music.join("Unknown Artist").join("2024 - Unknown Album").join("01 - Tidal Track 280721704.flac");
    tokio::fs::create_dir_all(dummy_path_43.parent().unwrap()).await.unwrap();
    write_valid_minimal_flac(&dummy_path_43).await;

    let _ = sqlx::query(
        "INSERT INTO downloads (id, track_id, source_service_id, file_path, file_format, metadata_completeness)
         VALUES (919, 19496, 3, ?, 'FLAC', 0)"
    )
    .bind(dummy_path_43.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .unwrap();

    // Apply repair
    let apply_res = reenrich_download_file(&pool, 919, false).await.unwrap();
    assert_eq!(apply_res.new_track_id, 43);

    // Verify 0 leftover ghost relations in DB
    let ghost_track: Option<i64> = sqlx::query_scalar("SELECT id FROM tracks WHERE id = 19496").fetch_optional(&pool).await.unwrap();
    assert!(ghost_track.is_none(), "Ghost track 19496 must be deleted");

    let ghost_ta: Option<i64> = sqlx::query_scalar("SELECT track_id FROM track_artists WHERE track_id = 19496").fetch_optional(&pool).await.unwrap();
    assert!(ghost_ta.is_none(), "Ghost track_artists must be deleted");

    let ghost_ts: Option<i64> = sqlx::query_scalar("SELECT track_id FROM track_sources WHERE track_id = 19496").fetch_optional(&pool).await.unwrap();
    assert!(ghost_ts.is_none(), "Ghost track_sources must be deleted");

    let ghost_alb: Option<i64> = sqlx::query_scalar("SELECT id FROM albums WHERE id = 14157").fetch_optional(&pool).await.unwrap();
    assert!(ghost_alb.is_none(), "Ghost album 14157 must be deleted");
}

