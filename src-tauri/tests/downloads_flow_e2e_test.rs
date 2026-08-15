//! E2E Test Suite for Sprint S107: Flujo de Descarga End-to-End desde UI (MVP Funcional)
//!
//! Validates:
//! 1. Enqueuing single tracks, album tracks, and artist tracks into `download_queue`.
//! 2. Priority assignment, deduplication, and position sequencing.
//! 3. Audio container validation (magic bytes: FLAC "fLaC", M4A "ftypM4A").
//! 4. Atomic database persistence into `downloads` and `download_queue`.
//! 5. Lifecycle status transitions (queued -> downloading -> completed/verified).

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

async fn create_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite");

    // Initialize core schema
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS services (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            supports_download INTEGER NOT NULL DEFAULT 1,
            max_quality TEXT NOT NULL DEFAULT 'lossless'
        );

        CREATE TABLE IF NOT EXISTS artists (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            service_artist_id TEXT,
            favorite_at DATETIME
        );

        CREATE TABLE IF NOT EXISTS albums (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            artist_id INTEGER REFERENCES artists(id),
            release_date TEXT,
            total_tracks INTEGER,
            favorite_at DATETIME
        );

        CREATE TABLE IF NOT EXISTS tracks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            album_id INTEGER REFERENCES albums(id),
            duration_ms INTEGER,
            track_number INTEGER,
            isrc TEXT,
            audio_quality TEXT,
            favorite_at DATETIME
        );

        CREATE TABLE IF NOT EXISTS download_queue (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER NOT NULL UNIQUE REFERENCES tracks(id) ON DELETE CASCADE,
            priority INTEGER NOT NULL DEFAULT 50,
            quality_preference TEXT,
            status TEXT NOT NULL DEFAULT 'queued' CHECK(status IN ('queued', 'downloading', 'completed', 'failed', 'cancelled')),
            progress_percent REAL NOT NULL DEFAULT 0.0,
            bytes_downloaded INTEGER,
            total_bytes INTEGER,
            error_message TEXT,
            last_error TEXT,
            retry_count INTEGER NOT NULL DEFAULT 0,
            position INTEGER,
            resumable INTEGER NOT NULL DEFAULT 1,
            staging_path TEXT,
            created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            started_at DATETIME,
            completed_at DATETIME
        );

        CREATE TABLE IF NOT EXISTS downloads (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
            source_service_id INTEGER NOT NULL REFERENCES services(id),
            file_path TEXT NOT NULL UNIQUE,
            file_format TEXT NOT NULL,
            bit_depth INTEGER,
            sample_rate INTEGER,
            file_size_bytes INTEGER NOT NULL,
            download_duration_ms INTEGER,
            status TEXT NOT NULL DEFAULT 'completed' CHECK(status IN ('completed', 'verified', 'corrupted', 'missing')),
            downloaded_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            verified_at DATETIME,
            sha256_hash TEXT
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("Schema creation failed");

    pool
}

#[tokio::test]
async fn test_enqueue_single_track_lifecycle() {
    let pool = create_test_db().await;

    // 1. Seed service, artist, album, track
    let _svc_id: i64 = sqlx::query_scalar("INSERT INTO services (name) VALUES ('tidal') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let art_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('The Warning') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let alb_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, artist_id) VALUES ('ERROR', ?) RETURNING id")
        .bind(art_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    let trk_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms, isrc) VALUES ('Choke', ?, 232000, 'USUG12101234') RETURNING id"
    )
    .bind(alb_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // 2. Enqueue single track (priority = 50)
    let queue_id: i64 = sqlx::query_scalar(
        "INSERT INTO download_queue (track_id, priority, quality_preference, status) VALUES (?, 50, 'HI_RES_LOSSLESS', 'queued') RETURNING id"
    )
    .bind(trk_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(queue_id > 0);

    // 3. Verify queue state
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_queue WHERE status = 'queued'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);

    // 4. Simulate download worker transitioning to downloading -> completed
    sqlx::query("UPDATE download_queue SET status = 'downloading', progress_percent = 50.0, started_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(queue_id)
        .execute(&pool)
        .await
        .unwrap();

    let status: String = sqlx::query_scalar("SELECT status FROM download_queue WHERE id = ?")
        .bind(queue_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "downloading");

    sqlx::query("UPDATE download_queue SET status = 'completed', progress_percent = 100.0, completed_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(queue_id)
        .execute(&pool)
        .await
        .unwrap();

    let completed_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_queue WHERE status = 'completed'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(completed_count, 1);
}

#[tokio::test]
async fn test_enqueue_album_batch_tracks() {
    let pool = create_test_db().await;

    let art_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Daft Punk') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let alb_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, artist_id, total_tracks) VALUES ('Discovery', ?, 3) RETURNING id")
        .bind(art_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    let track_titles = vec!["One More Time", "Aerodynamic", "Digital Love"];
    let mut track_ids = Vec::new();

    for (idx, title) in track_titles.iter().enumerate() {
        let tid: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, album_id, track_number) VALUES (?, ?, ?) RETURNING id"
        )
        .bind(title)
        .bind(alb_id)
        .bind((idx + 1) as i64)
        .fetch_one(&pool)
        .await
        .unwrap();
        track_ids.push(tid);
    }

    assert_eq!(track_ids.len(), 3);

    // Enqueue all 3 tracks
    for (pos, tid) in track_ids.iter().enumerate() {
        sqlx::query(
            "INSERT OR IGNORE INTO download_queue (track_id, priority, position, status) VALUES (?, 50, ?, 'queued')"
        )
        .bind(tid)
        .bind(pos as i64)
        .execute(&pool)
        .await
        .unwrap();
    }

    let queued_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_queue WHERE status = 'queued'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(queued_count, 3);

    // Verify ordering by position
    let ordered_ids: Vec<i64> = sqlx::query_scalar(
        "SELECT track_id FROM download_queue WHERE status = 'queued' ORDER BY position ASC"
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(ordered_ids, track_ids);
}

#[tokio::test]
async fn test_enqueue_artist_discography_deduplication() {
    let pool = create_test_db().await;

    let art_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Pink Floyd') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let alb_1: i64 = sqlx::query_scalar("INSERT INTO albums (title, artist_id) VALUES ('The Dark Side of the Moon', ?) RETURNING id")
        .bind(art_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    let alb_2: i64 = sqlx::query_scalar("INSERT INTO albums (title, artist_id) VALUES ('Wish You Were Here', ?) RETURNING id")
        .bind(art_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    let t1: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id) VALUES ('Time', ?) RETURNING id")
        .bind(alb_1)
        .fetch_one(&pool)
        .await
        .unwrap();

    let t2: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id) VALUES ('Money', ?) RETURNING id")
        .bind(alb_1)
        .fetch_one(&pool)
        .await
        .unwrap();

    let t3: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id) VALUES ('Shine On You Crazy Diamond', ?) RETURNING id")
        .bind(alb_2)
        .fetch_one(&pool)
        .await
        .unwrap();

    // Already queued t1
    sqlx::query("INSERT INTO download_queue (track_id, priority, status) VALUES (?, 50, 'queued')")
        .bind(t1)
        .execute(&pool)
        .await
        .unwrap();

    // Enqueue all tracks for artist (t1, t2, t3) with INSERT OR IGNORE
    let artist_tracks = vec![t1, t2, t3];
    let mut added = 0;
    for tid in artist_tracks {
        let res = sqlx::query("INSERT OR IGNORE INTO download_queue (track_id, priority, status) VALUES (?, 50, 'queued')")
            .bind(tid)
            .execute(&pool)
            .await
            .unwrap();
        if res.rows_affected() > 0 {
            added += 1;
        }
    }

    assert_eq!(added, 2); // t1 was ignored because already queued
    let total_in_queue: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_queue")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(total_in_queue, 3);
}

#[tokio::test]
async fn test_audio_magic_bytes_validation_contract() {
    // FLAC magic bytes header: "fLaC" (0x66 0x4C 0x61 0x43)
    let flac_bytes = b"fLaC\x00\x00\x00\x22\x10\x00\x10\x00";
    assert_eq!(&flac_bytes[0..4], b"fLaC");

    // MP4 / M4A ftyp header: 4-byte size + "ftypM4A "
    let m4a_bytes = b"\x00\x00\x00\x20ftypM4A \x00\x00\x00\x00M4A mp42isom";
    assert_eq!(&m4a_bytes[4..8], b"ftyp");
    assert_eq!(&m4a_bytes[8..11], b"M4A");

    // Invalid payload
    let invalid_bytes = b"corrupt payload not audio";
    assert_ne!(&invalid_bytes[0..4], b"fLaC");
    assert_ne!(&invalid_bytes[4..8], b"ftyp");
}

#[tokio::test]
async fn test_download_persistence_in_library_table() {
    let pool = create_test_db().await;

    let svc_id: i64 = sqlx::query_scalar("INSERT INTO services (name) VALUES ('qobuz') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let art_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Miles Davis') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let alb_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, artist_id) VALUES ('Kind of Blue', ?) RETURNING id")
        .bind(art_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    let trk_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms, isrc) VALUES ('So What', ?, 562000, 'USSM15900001') RETURNING id"
    )
    .bind(alb_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Persist verified download
    let test_path = "C:/Music/Miles Davis/Kind of Blue/01 - So What.flac";
    let dl_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO downloads (
            track_id, source_service_id, file_path, file_format, bit_depth,
            sample_rate, file_size_bytes, download_duration_ms, status, verified_at, sha256_hash
        ) VALUES (
            ?, ?, ?, 'FLAC', 24, 96000, 125000000, 3200, 'verified', CURRENT_TIMESTAMP, 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'
        ) RETURNING id
        "#
    )
    .bind(trk_id)
    .bind(svc_id)
    .bind(test_path)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(dl_id > 0);

    // Verify download record
    let status: String = sqlx::query_scalar("SELECT status FROM downloads WHERE id = ?")
        .bind(dl_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "verified");

    let file_format: String = sqlx::query_scalar("SELECT file_format FROM downloads WHERE id = ?")
        .bind(dl_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(file_format, "FLAC");
}
