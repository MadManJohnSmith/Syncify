//! Test Suite: Sidecar Completeness (LRC & Album Covers) [TASK-111]
//!
//! Validates:
//! 1. Materialization of missing `.lrc` sidecars from SQLite `lyrics` table for tracks in `downloads`.
//! 2. Preservation of synchronized timestamps and text formatting during .lrc materialization.
//! 3. Detection and materialization of missing album covers from embedded audio metadata (M4A covr atom & FLAC PICTURE block).
//! 4. Strict preservation of the Symfonium Invariant: CoverFront (0x03) = image/webp animado is NEVER overwritten or degraded.
//! 5. Multi-disc parent directory cover propagation (Disc 1 -> Album root).
//! 6. Idempotency across multiple consecutive executions.

use mp4ameta::{Data, Fourcc, Tag};
use sqlx::sqlite::SqlitePoolOptions;
use std::path::Path;
use syncify_tauri_lib::commands::lyrics::{
    materialize_missing_covers_pool, materialize_missing_lrc_sidecars_pool,
};
use syncify_tauri_lib::services::mp4_writer::ensure_m4a_sidecars_intact;
use tempfile::TempDir;

/// Helper: creates a synthetic minimal JPEG with SOF0 header encoding exact dimensions.
fn create_synthetic_jpeg(width: u16, height: u16) -> Vec<u8> {
    let mut jpeg = Vec::new();
    jpeg.extend_from_slice(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x08, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01]);
    jpeg.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x0B, 0x08]); // SOF0, len 11, 8-bit precision
    jpeg.extend_from_slice(&height.to_be_bytes()); // height
    jpeg.extend_from_slice(&width.to_be_bytes()); // width
    jpeg.extend_from_slice(&[0x03]); // 3 components (YCbCr)
    jpeg.extend_from_slice(&[0xFF, 0xD9]); // EOI
    jpeg
}

/// Helper: creates a valid synthetic animated WebP container with RIFF, VP8X, ANIM, and ANMF frames.
fn create_synthetic_animated_webp(width: u16, height: u16, frame_count: u16) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&0u32.to_le_bytes()); // placeholder size
    data.extend_from_slice(b"WEBP");
    data.extend_from_slice(b"VP8X");
    data.extend_from_slice(&10u32.to_le_bytes()); // VP8X chunk size
    data.push(0x02); // animation flag set (bit 1)
    data.extend_from_slice(&[0u8; 3]); // reserved
    data.extend_from_slice(&(width as u32 - 1).to_le_bytes()[..3]);
    data.extend_from_slice(&(height as u32 - 1).to_le_bytes()[..3]);

    data.extend_from_slice(b"ANIM");
    data.extend_from_slice(&6u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes()); // bg color
    data.extend_from_slice(&0u16.to_le_bytes()); // loop count

    for _ in 0..frame_count {
        data.extend_from_slice(b"ANMF");
        data.extend_from_slice(&16u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes()[..3]); // frame x
        data.extend_from_slice(&0u32.to_le_bytes()[..3]); // frame y
        data.extend_from_slice(&(width as u32 - 1).to_le_bytes()[..3]);
        data.extend_from_slice(&(height as u32 - 1).to_le_bytes()[..3]);
        data.extend_from_slice(&100u32.to_le_bytes()[..3]); // duration ms
        data.push(0x00); // flags
    }

    let file_size = (data.len() - 8) as u32;
    data[4..8].copy_from_slice(&file_size.to_le_bytes());
    data
}

/// Helper: creates a valid minimal FLAC file header
fn create_test_flac(path: &Path) {
    let mut data = Vec::new();
    data.extend_from_slice(b"fLaC");
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x22]); // STREAMINFO header
    data.extend_from_slice(&[0u8; 34]); // STREAMINFO body (34 bytes)
    data.extend_from_slice(&[0x81, 0x00, 0x00, 0x00]); // PADDING header
    data.extend(vec![0xCC; 1024]);
    std::fs::write(path, &data).expect("Failed to write initial FLAC frame");
}

/// Helper: creates a minimal valid M4A file with optional embedded cover
fn create_test_m4a_with_cover(path: &Path, cover_bytes: Option<&[u8]>) {
    let temp_wav = path.with_extension("wav");
    let mut wav_bytes = Vec::new();
    let num_samples = 44100 / 4; // 0.25s
    let sample_rate = 44100u32;
    let byte_rate = sample_rate * 2;

    wav_bytes.extend_from_slice(b"RIFF");
    wav_bytes.extend_from_slice(&((36 + num_samples * 2) as u32).to_le_bytes());
    wav_bytes.extend_from_slice(b"WAVEfmt ");
    wav_bytes.extend_from_slice(&16u32.to_le_bytes());
    wav_bytes.extend_from_slice(&1u16.to_le_bytes());
    wav_bytes.extend_from_slice(&1u16.to_le_bytes());
    wav_bytes.extend_from_slice(&sample_rate.to_le_bytes());
    wav_bytes.extend_from_slice(&byte_rate.to_le_bytes());
    wav_bytes.extend_from_slice(&2u16.to_le_bytes());
    wav_bytes.extend_from_slice(&16u16.to_le_bytes());
    wav_bytes.extend_from_slice(b"data");
    wav_bytes.extend_from_slice(&((num_samples * 2) as u32).to_le_bytes());
    wav_bytes.extend(vec![0u8; (num_samples * 2) as usize]);

    std::fs::write(&temp_wav, &wav_bytes).expect("Write temp wav");

    let _ = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-i",
            temp_wav.to_str().unwrap(),
            "-c:a",
            "aac",
            "-b:a",
            "128k",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("ffmpeg transcode");

    let _ = std::fs::remove_file(&temp_wav);

    if let Some(cover) = cover_bytes {
        let mut tag = Tag::read_from_path(path).expect("Read M4A tag");
        let data = if cover.starts_with(b"\x89PNG") {
            Data::Png(cover.to_vec())
        } else {
            Data::Jpeg(cover.to_vec())
        };
        tag.set_data(Fourcc(*b"covr"), data);
        tag.write_to_path(path).expect("Write cover to M4A");
    }
}

/// Initialize SQLite in-memory test database with canonical minimal schema
async fn init_test_db() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite");

    sqlx::query(
        r#"
        CREATE TABLE albums (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            cover_art_url TEXT
        );

        CREATE TABLE tracks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            album_id INTEGER REFERENCES albums(id),
            title TEXT NOT NULL
        );

        CREATE TABLE downloads (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER UNIQUE REFERENCES tracks(id),
            file_path TEXT NOT NULL
        );

        CREATE TABLE lyrics (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER REFERENCES tracks(id),
            format TEXT NOT NULL,
            sync_level TEXT,
            source TEXT,
            content TEXT NOT NULL,
            language TEXT,
            embedded_in_file INTEGER DEFAULT 0,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(track_id, format)
        );
        "#
    )
    .execute(&pool)
    .await
    .expect("Failed to initialize test schema");

    pool
}

#[tokio::test]
async fn test_materialize_missing_lrc_sidecar() {
    let pool = init_test_db().await;
    let temp_dir = TempDir::new().expect("temp dir");

    let audio_path = temp_dir.path().join("01 - Test Song.flac");
    create_test_flac(&audio_path);

    // 1. Insert album and track
    sqlx::query("INSERT INTO albums (id, title) VALUES (1, 'Test Album')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO tracks (id, album_id, title) VALUES (1, 1, 'Test Song')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO downloads (id, track_id, file_path) VALUES (1, 1, ?)")
        .bind(audio_path.to_str().unwrap())
        .execute(&pool)
        .await
        .unwrap();

    let synced_lrc_content = "[00:01.00]Line 1 of song\n[00:05.50]Line 2 of song\n[00:10.00]Line 3 of song\n";

    sqlx::query(
        "INSERT INTO lyrics (track_id, format, sync_level, source, content) VALUES (1, 'lrc', 'line', 'lrclib', ?)"
    )
    .bind(synced_lrc_content)
    .execute(&pool)
    .await
    .unwrap();

    let expected_lrc = audio_path.with_extension("lrc");
    assert!(!expected_lrc.exists(), "Sidecar .lrc must not exist before materialization");

    // 2. Run materialization
    let result = materialize_missing_lrc_sidecars_pool(&pool, None)
        .await
        .expect("Materialization failed");

    assert_eq!(result.scanned, 1);
    assert_eq!(result.materialized, 1);
    assert_eq!(result.already_present, 0);
    assert_eq!(result.failed, 0);

    // 3. Verify file on disk
    assert!(expected_lrc.exists(), "Sidecar .lrc must exist on disk after materialization");
    let disk_content = std::fs::read_to_string(&expected_lrc).expect("Read materialized .lrc");
    assert_eq!(disk_content, synced_lrc_content, "Materialized .lrc content and timestamps must match DB exactly");

    // 4. Idempotency: Second execution must skip already materialized sidecar
    let second_run = materialize_missing_lrc_sidecars_pool(&pool, None)
        .await
        .expect("Second run failed");

    assert_eq!(second_run.scanned, 1);
    assert_eq!(second_run.materialized, 0);
    assert_eq!(second_run.already_present, 1);
}

#[tokio::test]
async fn test_materialize_missing_covers_from_embedded_m4a() {
    let pool = init_test_db().await;
    let temp_dir = TempDir::new().expect("temp dir");
    let album_dir = temp_dir.path().join("Artist - Album");
    std::fs::create_dir_all(&album_dir).expect("create album dir");

    let m4a_path = album_dir.join("01 Track.m4a");
    let synthetic_jpeg = create_synthetic_jpeg(600, 600);
    create_test_m4a_with_cover(&m4a_path, Some(&synthetic_jpeg));

    // Confirm cover.jpg does not exist before materialization
    let cover_jpg = album_dir.join("cover.jpg");
    assert!(!cover_jpg.exists(), "cover.jpg must not exist before test");

    // Insert database records
    sqlx::query("INSERT INTO albums (id, title, cover_art_url) VALUES (1, 'Album', NULL)")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO tracks (id, album_id, title) VALUES (1, 1, 'Track')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO downloads (id, track_id, file_path) VALUES (1, 1, ?)")
        .bind(m4a_path.to_str().unwrap())
        .execute(&pool)
        .await
        .unwrap();

    // Run cover materializer
    let result = materialize_missing_covers_pool(&pool, None)
        .await
        .expect("Materialize covers failed");

    assert_eq!(result.scanned_albums, 1);
    assert_eq!(result.materialized_from_embedded, 1);
    assert_eq!(result.already_present, 0);

    // Verify cover.jpg on disk
    assert!(cover_jpg.exists(), "cover.jpg must be materialized from embedded covr atom");
    let disk_cover_bytes = std::fs::read(&cover_jpg).expect("read materialized cover.jpg");
    assert_eq!(disk_cover_bytes, synthetic_jpeg, "Materialized cover bytes must match embedded artwork");

    // Idempotency check: Subsequent run must detect already_present
    let second_run = materialize_missing_covers_pool(&pool, None)
        .await
        .expect("Second run failed");

    assert_eq!(second_run.scanned_albums, 1);
    assert_eq!(second_run.already_present, 1);
    assert_eq!(second_run.materialized_from_embedded, 0);
}

#[tokio::test]
async fn test_symfonium_animated_cover_invariant_never_overwritten() {
    let pool = init_test_db().await;
    let temp_dir = TempDir::new().expect("temp dir");
    let album_dir = temp_dir.path().join("Daft Punk - Discovery");
    std::fs::create_dir_all(&album_dir).expect("create album dir");

    // Create an existing animated WebP cover in the directory (Symfonium Invariant CoverFront 0x03)
    let cover_webp = album_dir.join("cover.webp");
    let synthetic_webp = create_synthetic_animated_webp(800, 800, 5);
    std::fs::write(&cover_webp, &synthetic_webp).expect("write cover.webp");

    let m4a_path = album_dir.join("01 One More Time.m4a");
    let embedded_jpeg = create_synthetic_jpeg(500, 500);
    create_test_m4a_with_cover(&m4a_path, Some(&embedded_jpeg));

    // Setup DB
    sqlx::query("INSERT INTO albums (id, title, cover_art_url) VALUES (1, 'Discovery', 'http://example.com/static_cover.jpg')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO tracks (id, album_id, title) VALUES (1, 1, 'One More Time')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO downloads (id, track_id, file_path) VALUES (1, 1, ?)")
        .bind(m4a_path.to_str().unwrap())
        .execute(&pool)
        .await
        .unwrap();

    // Run cover materializer
    let result = materialize_missing_covers_pool(&pool, None)
        .await
        .expect("Materialize covers failed");

    // The album must be marked already_present because a valid cover.webp exists!
    assert_eq!(result.scanned_albums, 1);
    assert_eq!(result.already_present, 1);
    assert_eq!(result.materialized_from_embedded, 0);
    assert_eq!(result.materialized_from_url, 0);

    // Verify that cover.webp was completely UNTOUCHED
    let disk_webp = std::fs::read(&cover_webp).expect("read cover.webp");
    assert_eq!(disk_webp, synthetic_webp, "Symfonium animated cover.webp must NEVER be modified or degraded");

    // Verify that cover.jpg was NOT created, avoiding conflicts with the animated WebP
    let cover_jpg = album_dir.join("cover.jpg");
    assert!(!cover_jpg.exists(), "cover.jpg must NOT be generated when valid animated cover.webp is present");
}

#[tokio::test]
async fn test_multidisc_cover_propagation() {
    let pool = init_test_db().await;
    let temp_dir = TempDir::new().expect("temp dir");
    let album_root = temp_dir.path().join("Pink Floyd - The Wall");
    let disc1_dir = album_root.join("Disc 1");
    std::fs::create_dir_all(&disc1_dir).expect("create disc1 dir");

    let m4a_path = disc1_dir.join("01 In the Flesh.m4a");
    let synthetic_jpeg = create_synthetic_jpeg(500, 500);
    create_test_m4a_with_cover(&m4a_path, Some(&synthetic_jpeg));

    sqlx::query("INSERT INTO albums (id, title, cover_art_url) VALUES (1, 'The Wall', NULL)")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO tracks (id, album_id, title) VALUES (1, 1, 'In the Flesh')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO downloads (id, track_id, file_path) VALUES (1, 1, ?)")
        .bind(m4a_path.to_str().unwrap())
        .execute(&pool)
        .await
        .unwrap();

    let result = materialize_missing_covers_pool(&pool, None)
        .await
        .expect("Materialize covers failed");

    assert_eq!(result.materialized_from_embedded, 1);

    // Both Disc 1 and album root must now possess cover.jpg
    let disc_cover = disc1_dir.join("cover.jpg");
    let root_cover = album_root.join("cover.jpg");

    assert!(disc_cover.exists(), "Disc 1 must have cover.jpg");
    assert!(root_cover.exists(), "Album root must have propagated cover.jpg");

    assert_eq!(std::fs::read(&disc_cover).unwrap(), synthetic_jpeg);
    assert_eq!(std::fs::read(&root_cover).unwrap(), synthetic_jpeg);
}

#[test]
fn test_ensure_m4a_sidecars_intact_standalone() {
    let temp_dir = TempDir::new().expect("temp dir");
    let m4a_path = temp_dir.path().join("song.m4a");
    let synthetic_jpeg = create_synthetic_jpeg(400, 400);
    create_test_m4a_with_cover(&m4a_path, Some(&synthetic_jpeg));

    let cover_jpg = temp_dir.path().join("cover.jpg");
    assert!(!cover_jpg.exists());

    // First call: extracts artwork and creates cover.jpg
    let created = ensure_m4a_sidecars_intact(&m4a_path, temp_dir.path())
        .expect("ensure_m4a_sidecars_intact failed");

    assert_eq!(created.len(), 1);
    assert_eq!(created[0], cover_jpg);
    assert!(cover_jpg.exists());
    assert_eq!(std::fs::read(&cover_jpg).unwrap(), synthetic_jpeg);

    // Second call: already intact, returns empty list
    let second_call = ensure_m4a_sidecars_intact(&m4a_path, temp_dir.path())
        .expect("second call failed");
    assert!(second_call.is_empty());
}
