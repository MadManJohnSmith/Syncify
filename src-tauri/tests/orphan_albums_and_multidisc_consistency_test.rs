//! Integration Test Suite for TASK-70:
//! Limpieza de Álbumes Huérfanos Vacíos (0 Pistas) y Consistencia de Discos Multi-Volumen
//!
//! Validates:
//! 1. Empty orphan albums (0 tracks in `tracks`) are purged cleanly.
//! 2. Associated orphan `album_artists` links are deleted without leaving dangling foreign keys.
//! 3. Legitimate stub albums (`is_stub = 1`) are strictly preserved.
//! 4. Database integrity and foreign key constraints remain 100% valid (`PRAGMA foreign_key_check = 0`).
//! 5. FLAC Vorbis comments for multidisc releases correctly emit `DISCNUMBER`, `TOTALDISCS`,
//!    and `DISCTOTAL` consistently.
//! 6. `TRACKTOTAL` in FLAC Vorbis comments reflects the specific disc's track count rather than
//!    the overall box set track total.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::path::PathBuf;
use syncify_flac_writer::{apply_and_verify_flac_tags, FlacMetadata};
use syncify_tauri_lib::commands::library::perform_purge_orphan_empty_albums;

struct TestFlacFile {
    path: PathBuf,
}

impl Drop for TestFlacFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn create_test_flac_file() -> TestFlacFile {
    let path = std::env::temp_dir().join(format!(
        "test_task70_flac_{}_{}.flac",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut flac_bytes = Vec::new();
    flac_bytes.extend_from_slice(b"fLaC");
    flac_bytes.extend_from_slice(&[
        0x80, 0x00, 0x00, 0x22, // Last metadata block (STREAMINFO), length 34
        0x10, 0x00, 0x10, 0x00, // min/max block size
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // min/max frame size
        0x0A, 0xC4, 0x42, 0xF0, // 44.1kHz, 2 channels, 16 bits, 0 samples
        0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ]);
    std::fs::write(&path, &flac_bytes).expect("Failed to write initial FLAC bytes");
    TestFlacFile { path }
}

async fn create_test_db() -> sqlx::Pool<sqlx::Sqlite> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations must apply cleanly");

    pool
}

#[tokio::test]
async fn test_purge_orphan_empty_albums_preserves_stubs_and_cleans_album_artists() {
    let pool = create_test_db().await;

    // 1. Insert test artists
    let artist1_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Pink Floyd') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let artist2_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Ghost Artist') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    let artist3_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Favorite Band') RETURNING id")
        .fetch_one(&pool).await.unwrap();

    // 2. Album A: Normal album with tracks (is_stub = 0)
    let album_a_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, total_tracks, is_stub) VALUES ('The Dark Side of the Moon', 10, 0) RETURNING id"
    )
    .fetch_one(&pool).await.unwrap();

    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)")
        .bind(album_a_id).bind(artist1_id).execute(&pool).await.unwrap();

    sqlx::query("INSERT INTO tracks (title, album_id, track_number, duration_ms, isrc) VALUES ('Speak to Me', ?, 1, 65000, 'GBAYE7300001')")
        .bind(album_a_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO tracks (title, album_id, track_number, duration_ms, isrc) VALUES ('Breathe', ?, 2, 163000, 'GBAYE7300002')")
        .bind(album_a_id).execute(&pool).await.unwrap();

    // 3. Album B: Empty orphan album with 0 tracks (is_stub = 0)
    let album_b_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, total_tracks, is_stub) VALUES ('Failed Import Album', 0, 0) RETURNING id"
    )
    .fetch_one(&pool).await.unwrap();

    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)")
        .bind(album_b_id).bind(artist2_id).execute(&pool).await.unwrap();

    // 4. Album C: Legitimate empty stub album (0 tracks, is_stub = 1)
    let album_c_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, total_tracks, is_stub) VALUES ('Wishlist Stub Album', 0, 1) RETURNING id"
    )
    .fetch_one(&pool).await.unwrap();

    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)")
        .bind(album_c_id).bind(artist3_id).execute(&pool).await.unwrap();

    // 5. Album D: Another empty orphan album with 0 tracks (default is_stub = 0)
    let album_d_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, total_tracks) VALUES ('Dangling Album', NULL) RETURNING id"
    )
    .fetch_one(&pool).await.unwrap();

    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)")
        .bind(album_d_id).bind(artist2_id).execute(&pool).await.unwrap();

    // Pre-check verification
    let count_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM albums").fetch_one(&pool).await.unwrap();
    assert_eq!(count_before, 4, "Should have 4 albums before purge");

    let aa_count_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM album_artists").fetch_one(&pool).await.unwrap();
    assert_eq!(aa_count_before, 4, "Should have 4 album_artists links before purge");

    // Execute purge
    let report = perform_purge_orphan_empty_albums(&pool)
        .await
        .expect("perform_purge_orphan_empty_albums should succeed");

    // Verify report
    assert_eq!(report.purged_albums_count, 2, "Album B and D must be purged (2 albums)");
    assert_eq!(report.purged_album_artists_count, 2, "2 album_artists rows must be purged");
    assert_eq!(report.preserved_stubs_count, 1, "Album C stub must be preserved");

    // Post-check verification
    let count_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM albums").fetch_one(&pool).await.unwrap();
    assert_eq!(count_after, 2, "Should have 2 albums remaining (Album A and Album C)");

    // Album A check
    let a_exists: bool = sqlx::query_scalar("SELECT COUNT(*) > 0 FROM albums WHERE id = ?")
        .bind(album_a_id).fetch_one(&pool).await.unwrap();
    assert!(a_exists, "Album A must exist");

    // Album B check (purged)
    let b_exists: bool = sqlx::query_scalar("SELECT COUNT(*) > 0 FROM albums WHERE id = ?")
        .bind(album_b_id).fetch_one(&pool).await.unwrap();
    assert!(!b_exists, "Album B must have been purged");

    // Album C check (stub preserved)
    let c_exists: bool = sqlx::query_scalar("SELECT COUNT(*) > 0 FROM albums WHERE id = ? AND is_stub = 1")
        .bind(album_c_id).fetch_one(&pool).await.unwrap();
    assert!(c_exists, "Album C must exist and remain is_stub = 1");

    // Album D check (purged)
    let d_exists: bool = sqlx::query_scalar("SELECT COUNT(*) > 0 FROM albums WHERE id = ?")
        .bind(album_d_id).fetch_one(&pool).await.unwrap();
    assert!(!d_exists, "Album D must have been purged");

    // Verify album_artists integrity
    let aa_count_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM album_artists").fetch_one(&pool).await.unwrap();
    assert_eq!(aa_count_after, 2, "Should have 2 album_artists remaining (Album A and Album C)");

    let b_aa_exists: bool = sqlx::query_scalar("SELECT COUNT(*) > 0 FROM album_artists WHERE album_id = ?")
        .bind(album_b_id).fetch_one(&pool).await.unwrap();
    assert!(!b_aa_exists, "Album B album_artists must have been deleted");

    let d_aa_exists: bool = sqlx::query_scalar("SELECT COUNT(*) > 0 FROM album_artists WHERE album_id = ?")
        .bind(album_d_id).fetch_one(&pool).await.unwrap();
    assert!(!d_aa_exists, "Album D album_artists must have been deleted");

    // Verify foreign key integrity
    let fk_violations: Vec<(String, Option<i64>, String, i64)> = sqlx::query_as("PRAGMA foreign_key_check;")
        .fetch_all(&pool).await.unwrap();
    assert!(fk_violations.is_empty(), "Foreign key check must return 0 violations: {:?}", fk_violations);

    // Idempotency: Second run should do nothing
    let report2 = perform_purge_orphan_empty_albums(&pool)
        .await
        .expect("Second purge must succeed");
    assert_eq!(report2.purged_albums_count, 0, "Second run should purge 0 albums");
    assert_eq!(report2.purged_album_artists_count, 0, "Second run should purge 0 album_artists");
    assert_eq!(report2.preserved_stubs_count, 1, "Stub Album C must still be preserved");
}

#[tokio::test]
async fn test_purge_cleans_dangling_album_artists_without_album() {
    let pool = create_test_db().await;

    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Dangling Artist') RETURNING id")
        .fetch_one(&pool).await.unwrap();

    // Insert a dangling album_artists row referencing non-existent album id 999999
    // Disable FK temporarily to simulate pre-existing corruption
    sqlx::query("PRAGMA foreign_keys = OFF;").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (999999, ?, 1)")
        .bind(artist_id).execute(&pool).await.unwrap();
    sqlx::query("PRAGMA foreign_keys = ON;").execute(&pool).await.unwrap();

    let dangling_before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM album_artists WHERE album_id NOT IN (SELECT id FROM albums)"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(dangling_before, 1, "Should have 1 dangling album_artists before purge");

    let report = perform_purge_orphan_empty_albums(&pool)
        .await
        .expect("Purge must succeed even with dangling album_artists");

    assert_eq!(report.purged_album_artists_count, 1, "Dangling album_artists row must be purged");

    let dangling_after: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM album_artists WHERE album_id NOT IN (SELECT id FROM albums)"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(dangling_after, 0, "Should have 0 dangling album_artists after purge");

    let fk_violations: Vec<(String, Option<i64>, String, i64)> = sqlx::query_as("PRAGMA foreign_key_check;")
        .fetch_all(&pool).await.unwrap();
    assert!(fk_violations.is_empty(), "Foreign key check must return 0 violations");
}

#[test]
fn test_flac_vorbis_multidisc_tags_emission_disc_tracktotal_and_totaldiscs() {
    let temp_file = create_test_flac_file();
    let path = &temp_file.path;

    // Multidisc release: 3 CDs, total 41 tracks across box set.
    // Disc 2 has 14 tracks. Current track is track 7 on Disc 2.
    let meta = FlacMetadata {
        title: "Comfortably Numb".to_string(),
        artist: "Pink Floyd".to_string(),
        album: "The Wall (Experience Edition)".to_string(),
        album_artist: Some("Pink Floyd".to_string()),
        track_number: 7,
        track_total: 41,              // Overall boxset track count
        disc_track_total: Some(14),   // Local Disc 2 track count
        disc_number: 2,
        total_discs: Some(3),         // 3 CDs
        disc_total: 3,
        ..Default::default()
    };

    let ver = apply_and_verify_flac_tags(path, &meta).expect("apply_and_verify_flac_tags must succeed");
    assert!(ver.tags_match, "Tags must match expected: {:?}", ver.mismatches);

    let read_tag = metaflac::Tag::read_from_path(path).expect("Read FLAC tags");
    let comments = read_tag.vorbis_comments().expect("Vorbis comments");

    // 1. DISCNUMBER and DISCTOTAL / TOTALDISCS consistency
    assert_eq!(
        comments.get("DISCNUMBER"),
        Some(&vec!["2".to_string()]),
        "DISCNUMBER must be '2'"
    );
    assert_eq!(
        comments.get("DISCTOTAL"),
        Some(&vec!["3".to_string()]),
        "DISCTOTAL must be '3'"
    );
    assert_eq!(
        comments.get("TOTALDISCS"),
        Some(&vec!["3".to_string()]),
        "TOTALDISCS must be '3'"
    );

    // 2. TRACKNUMBER and TRACKTOTAL consistency
    assert_eq!(
        comments.get("TRACKNUMBER"),
        Some(&vec!["7".to_string()]),
        "TRACKNUMBER must be '7'"
    );
    // TRACKTOTAL must reflect local disc total (14), NOT box set total (41)
    assert_eq!(
        comments.get("TRACKTOTAL"),
        Some(&vec!["14".to_string()]),
        "TRACKTOTAL must reflect local disc track count (14) rather than box set total (41)"
    );
}

#[test]
fn test_flac_vorbis_multidisc_default_disc_number_when_zero() {
    let temp_file = create_test_flac_file();
    let path = &temp_file.path;

    // Disc number not explicitly passed (0), but total_discs indicates multi-disc
    let meta = FlacMetadata {
        title: "Track on Multi-Disc Set".to_string(),
        artist: "Various Artists".to_string(),
        album: "Greatest Box Set".to_string(),
        track_number: 1,
        track_total: 20,
        disc_track_total: Some(10),
        disc_number: 0,               // Unset/0
        total_discs: Some(2),         // 2 discs
        disc_total: 2,
        ..Default::default()
    };

    let ver = apply_and_verify_flac_tags(path, &meta).expect("apply_and_verify_flac_tags must succeed");
    assert!(ver.tags_match, "Tags must match expected: {:?}", ver.mismatches);

    let read_tag = metaflac::Tag::read_from_path(path).expect("Read FLAC tags");
    let comments = read_tag.vorbis_comments().expect("Vorbis comments");

    // When disc_number was 0 but total_discs > 1, DISCNUMBER defaults to 1
    assert_eq!(
        comments.get("DISCNUMBER"),
        Some(&vec!["1".to_string()]),
        "DISCNUMBER must default to '1' when multi-disc metadata is present"
    );
    assert_eq!(
        comments.get("DISCTOTAL"),
        Some(&vec!["2".to_string()]),
        "DISCTOTAL must be '2'"
    );
    assert_eq!(
        comments.get("TOTALDISCS"),
        Some(&vec!["2".to_string()]),
        "TOTALDISCS must be '2'"
    );
    assert_eq!(
        comments.get("TRACKTOTAL"),
        Some(&vec!["10".to_string()]),
        "TRACKTOTAL must reflect disc_track_total (10)"
    );
}

#[tokio::test]
async fn test_python_purge_script_execution() {
    let temp_dir = std::env::temp_dir().join(format!("syncify_test_py_purge_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_dir);
    let db_path = temp_dir.join("test_orphan.db");
    let backup_dir = temp_dir.join("backups");
    let _ = std::fs::create_dir_all(&backup_dir);

    let connect_opts = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(connect_opts)
        .await
        .expect("Connect to file DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Migrate file DB");

    // Insert artist, album with tracks, orphan album, stub album
    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Test Artist') RETURNING id")
        .fetch_one(&pool).await.unwrap();

    let alb_tracks: i64 = sqlx::query_scalar("INSERT INTO albums (title, total_tracks, is_stub) VALUES ('Keep Alb', 1, 0) RETURNING id")
        .fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(alb_tracks).bind(artist_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO tracks (title, album_id) VALUES ('Track 1', ?)")
        .bind(alb_tracks).execute(&pool).await.unwrap();

    let alb_orphan: i64 = sqlx::query_scalar("INSERT INTO albums (title, total_tracks, is_stub) VALUES ('Orphan Alb', 0, 0) RETURNING id")
        .fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(alb_orphan).bind(artist_id).execute(&pool).await.unwrap();

    let alb_stub: i64 = sqlx::query_scalar("INSERT INTO albums (title, total_tracks, is_stub) VALUES ('Stub Alb', 0, 1) RETURNING id")
        .fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(alb_stub).bind(artist_id).execute(&pool).await.unwrap();

    pool.close().await;

    let script_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("scripts/purge_orphan_empty_albums.py");

    // Run python script with --dry-run first
    let output_dry = std::process::Command::new("python3")
        .args([
            script_path.to_str().unwrap(),
            "--db-path",
            db_path.to_str().unwrap(),
            "--backup-dir",
            backup_dir.to_str().unwrap(),
            "--dry-run",
        ])
        .output()
        .expect("Execute python script dry-run");
    if !output_dry.status.success() {
        panic!(
            "Dry run failed: stdout={}\nstderr={}",
            String::from_utf8_lossy(&output_dry.stdout),
            String::from_utf8_lossy(&output_dry.stderr)
        );
    }

    // Run python script live
    let output_live = std::process::Command::new("python3")
        .args([
            script_path.to_str().unwrap(),
            "--db-path",
            db_path.to_str().unwrap(),
            "--backup-dir",
            backup_dir.to_str().unwrap(),
        ])
        .output()
        .expect("Execute python script live");
    if !output_live.status.success() {
        panic!(
            "Live run failed: stdout={}\nstderr={}",
            String::from_utf8_lossy(&output_live.stdout),
            String::from_utf8_lossy(&output_live.stderr)
        );
    }

    // Reopen DB and verify
    let connect_opts2 = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(false);

    let pool2 = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(connect_opts2)
        .await
        .expect("Reconnect file DB");

    let orphan_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM albums WHERE id = ?")
        .bind(alb_orphan).fetch_one(&pool2).await.unwrap();
    assert_eq!(orphan_count, 0, "Orphan album must be purged by python script");

    let stub_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM albums WHERE id = ? AND is_stub = 1")
        .bind(alb_stub).fetch_one(&pool2).await.unwrap();
    assert_eq!(stub_count, 1, "Stub album must be preserved by python script");

    let keep_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM albums WHERE id = ?")
        .bind(alb_tracks).fetch_one(&pool2).await.unwrap();
    assert_eq!(keep_count, 1, "Album with tracks must be preserved by python script");

    pool2.close().await;

    let _ = std::fs::remove_dir_all(&temp_dir);
}
