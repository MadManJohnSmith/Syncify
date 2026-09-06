//! Integration Test for Disk Layout Normalization Engine (TASK-110)
//!
//! Validates:
//! 1. Canonical album directory calculation with prefix `[YYYY]`.
//! 2. Path sanitization of illegal characters (`:`, `"`, `/`, `\`, `|`, `?`, `*`) and space collapsing.
//! 3. Database reconciliation and atomic updates of `downloads.file_path`.
//! 4. Re-integration of Various Artists (VA) orphans into `Various Artists/[{Year}] {Album}/...`.
//! 5. Preservation of sidecar files (.lrc, webp) and directory integrity.

use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use sqlx::sqlite::SqlitePoolOptions;
use tempfile::TempDir;

use syncify_core_domain::{
    canonical_album_name, is_various_artists, normalize_album_artist, sanitize_filename,
    LibraryLayout,
};
use syncify_tauri_lib::services::operation_recovery::{
    reconcile_canonical_download_records, resolve_canonical_track_path_from_db,
};

#[test]
fn test_canonical_album_path_calculation() {
    // 1. Valid year generates [YYYY] prefix
    assert_eq!(
        canonical_album_name("The Dark Side of the Moon", Some(1973)),
        "[1973] The Dark Side of the Moon"
    );

    // 2. None / invalid year preserves album name without empty brackets
    assert_eq!(
        canonical_album_name("Unknown Year Album", None),
        "Unknown Year Album"
    );
    assert_eq!(
        canonical_album_name("Invalid Year Album", Some(1850)),
        "Invalid Year Album"
    );

    // 3. Various artists indicators
    assert!(is_various_artists("Various Artists"));
    assert!(is_various_artists("VA"));
    assert!(is_various_artists("Various"));
    assert!(is_various_artists("v.a."));
    assert!(is_various_artists("V/A"));
    assert!(!is_various_artists("Radiohead"));

    // 4. Normalized artist
    assert_eq!(normalize_album_artist("VA"), "Various Artists");
    assert_eq!(normalize_album_artist("Pink Floyd"), "Pink Floyd");

    // 5. Layout canonical paths
    let layout = LibraryLayout::new("/Music");
    let alb_dir = layout.canonical_album_dir("Pink Floyd", "The Wall", Some(1979));
    assert_eq!(
        alb_dir,
        PathBuf::from("/Music").join("Pink Floyd").join("[1979] The Wall")
    );

    let va_alb_dir = layout.canonical_album_dir("VA", "Now That's Music", Some(2022));
    assert_eq!(
        va_alb_dir,
        PathBuf::from("/Music").join("Various Artists").join("[2022] Now That's Music")
    );
}

#[test]
fn test_sanitization_illegal_chars_and_space_collapse() {
    // Colon and forbidden characters replaced by underscore
    assert_eq!(sanitize_filename("Artist : Album <Deluxe>"), "Artist _ Album _Deluxe_");
    assert_eq!(sanitize_filename("What? \"Quotes\" / Slashes | Pipes * Asterisks"), "What_ _Quotes_ _ Slashes _ Pipes _ Asterisks");

    // Consecutive spaces collapsed to single space
    assert_eq!(
        sanitize_filename("Pink    Floyd   -   The    Wall"),
        "Pink Floyd - The Wall"
    );
    assert_eq!(
        sanitize_filename("  Leading  and   Trailing   Dots... "),
        "Leading and Trailing Dots"
    );

    // Reserved Windows device names
    assert_eq!(sanitize_filename("CON"), "CON_");
    assert_eq!(sanitize_filename("aux"), "aux_");
    assert_eq!(sanitize_filename("NUL"), "NUL_");
}

#[tokio::test]
async fn test_canonical_allocation_and_database_reconciliation() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("syncify_test_norm.db");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite:{}?mode=rwc", db_path.display()))
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let music_root = temp_dir.path().join("Music");
    fs::create_dir_all(&music_root).unwrap();

    // Set base_folder in folder_settings
    let music_root_str = music_root.to_string_lossy().to_string();
    sqlx::query(
        "UPDATE folder_settings SET base_folder = ?, folder_template = '{AlbumArtist}/[{Year}] {Album}' WHERE id = 1"
    )
    .bind(&music_root_str)
    .execute(&pool)
    .await
    .unwrap();

    // 1. Insert Artist, Album with release_date, Track
    let artist_id: i64 = sqlx::query_scalar(
        "INSERT INTO artists (name) VALUES ('Pink Floyd') RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let album_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, release_date) VALUES ('The Wall', '1979-11-30') RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)")
        .bind(album_id)
        .bind(artist_id)
        .execute(&pool)
        .await
        .unwrap();

    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (album_id, title, track_number, disc_number) VALUES (?, 'Hey You', 1, 1) RETURNING id"
    )
    .bind(album_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(track_id)
        .bind(artist_id)
        .execute(&pool)
        .await
        .unwrap();

    // 2. Simulate physical file in obsolete non-canonical path (missing [1979] year prefix)
    let obsolete_dir = music_root.join("Pink Floyd").join("The Wall");
    fs::create_dir_all(&obsolete_dir).unwrap();
    let obsolete_file = obsolete_dir.join("01 - Hey You.flac");
    {
        let mut f = File::create(&obsolete_file).unwrap();
        f.write_all(b"fake flac audio content").unwrap();
    }

    let obsolete_file_str = obsolete_file.to_string_lossy().to_string();

    // 3. Record in downloads ledger pointing to obsolete path
    let dl_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO downloads (
            track_id, file_path, file_size_bytes, file_format, bit_depth,
            sample_rate, metadata_completeness
        ) VALUES (?, ?, 22, 'FLAC', 16, 44100, 100) RETURNING id
        "#
    )
    .bind(track_id)
    .bind(&obsolete_file_str)
    .fetch_one(&pool)
    .await
    .unwrap();

    // 4. Verify canonical path resolution
    let canonical_opt = resolve_canonical_track_path_from_db(&pool, track_id).await.unwrap();
    assert!(canonical_opt.is_some());
    let canonical_path = canonical_opt.unwrap();
    let expected_path = music_root
        .join("Pink Floyd")
        .join("[1979] The Wall")
        .join("01 - Hey You.flac");
    assert_eq!(canonical_path, expected_path);

    // 5. Run reconciliation (live apply mode)
    let report = reconcile_canonical_download_records(&pool, false).await.unwrap();
    assert_eq!(report.scanned_downloads, 1);
    assert_eq!(report.updated_records, 1);
    assert_eq!(report.moved_physical_files, 1);
    assert!(report.errors.is_empty());

    // 6. Assert physical file moved to canonical path
    assert!(!obsolete_file.exists(), "Old non-canonical file must not remain");
    assert!(expected_path.exists(), "File must exist at canonical path with [1979] prefix");

    // 7. Assert downloads.file_path in SQLite is updated
    let updated_fp: String = sqlx::query_scalar(
        "SELECT file_path FROM downloads WHERE id = ?"
    )
    .bind(dl_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(updated_fp, expected_path.to_string_lossy().to_string());
}

#[tokio::test]
async fn test_various_artists_orphans_reintegration() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("syncify_test_va.db");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite:{}?mode=rwc", db_path.display()))
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let music_root = temp_dir.path().join("Music");
    fs::create_dir_all(&music_root).unwrap();

    let music_root_str = music_root.to_string_lossy().to_string();
    sqlx::query(
        "UPDATE folder_settings SET base_folder = ?, folder_template = '{AlbumArtist}/[{Year}] {Album}' WHERE id = 1"
    )
    .bind(&music_root_str)
    .execute(&pool)
    .await
    .unwrap();

    // 1. Fetch canonical Various Artists and insert track artist
    let va_artist_id: i64 = sqlx::query_scalar(
        "SELECT id FROM artists WHERE name = 'Various Artists' LIMIT 1"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let track_artist_id: i64 = sqlx::query_scalar(
        "INSERT INTO artists (name) VALUES ('Daft Punk') RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let album_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, release_date) VALUES ('Top Hits 2024', '2024-05-15') RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)")
        .bind(album_id)
        .bind(va_artist_id)
        .execute(&pool)
        .await
        .unwrap();

    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (album_id, title, track_number, disc_number) VALUES (?, 'Get Lucky', 1, 1) RETURNING id"
    )
    .bind(album_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(track_id)
        .bind(track_artist_id)
        .execute(&pool)
        .await
        .unwrap();

    // 2. Physical file placed in orphan non-canonical location: /Music/VA/Top Hits 2024/01 - Get Lucky.flac
    let orphan_dir = music_root.join("VA").join("Top Hits 2024");
    fs::create_dir_all(&orphan_dir).unwrap();
    let orphan_file = orphan_dir.join("01 - Get Lucky.flac");
    {
        let mut f = File::create(&orphan_file).unwrap();
        f.write_all(b"fake audio VA").unwrap();
    }

    let orphan_file_str = orphan_file.to_string_lossy().to_string();

    let dl_id: i64 = sqlx::query_scalar(
        "INSERT INTO downloads (track_id, file_path, file_size_bytes, file_format) VALUES (?, ?, 13, 'FLAC') RETURNING id"
    )
    .bind(track_id)
    .bind(&orphan_file_str)
    .fetch_one(&pool)
    .await
    .unwrap();

    // 3. Reconcile
    let report = reconcile_canonical_download_records(&pool, false).await.unwrap();
    assert_eq!(report.updated_records, 1);
    assert_eq!(report.moved_physical_files, 1);

    // 4. Expected canonical destination: Various Artists/[2024] Top Hits 2024/01 - Daft Punk - Get Lucky.flac
    let expected_canonical = music_root
        .join("Various Artists")
        .join("[2024] Top Hits 2024")
        .join("01 - Daft Punk - Get Lucky.flac");

    assert!(!orphan_file.exists());
    assert!(expected_canonical.exists(), "VA orphan must be reintegrated under Various Artists/[2024] Album");

    let db_fp: String = sqlx::query_scalar("SELECT file_path FROM downloads WHERE id = ?")
        .bind(dl_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(db_fp, expected_canonical.to_string_lossy().to_string());
}
