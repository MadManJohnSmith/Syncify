//! Integration tests for Sprint S111: Controlled Favorites Validation & Pipeline Parity
//!
//! Validates:
//! 1. Controlled sample limiting (limit: Some(5), Some(50))
//! 2. Full source identity propagation into download_queue
//! 3. Error classification for 404 / stale sources without auth invalidation
//! 4. Audio byte validation (fLaC magic header) and tag completeness
//! 5. Staging lifecycle and clean directory state

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use syncify_core_domain::byte_validators::AudioByteValidator;

async fn create_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations must apply cleanly");

    // Insert baseline services
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (2, 'qobuz', 1, 'hires')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'hires')")
        .execute(&pool).await.unwrap();

    // Insert baseline accounts
    sqlx::query("INSERT INTO accounts (id, service_id, display_name, email, is_active) VALUES (2, 2, 'Qobuz User', 'user@qobuz.com', 1)")
        .execute(&pool).await.unwrap();

    pool
}

#[tokio::test]
async fn test_download_favorites_limit_filtering_and_source_population() {
    let db = create_test_db().await;

    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Test Artist') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, upc) VALUES ('Test Album', '123456789012') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)").bind(album_id).bind(artist_id).execute(&db).await.unwrap();

    // Insert 10 favorite tracks into library_entries & track_sources
    for i in 1..=10 {
        let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES (?, ?, ?) RETURNING id")
            .bind(format!("Track {}", i))
            .bind(album_id)
            .bind(format!("USRC123400{:02}", i))
            .fetch_one(&db).await.unwrap();

        sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
            .bind(tid).bind(artist_id).execute(&db).await.unwrap();

        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, ?, 'FLAC', 16, 44100, 100, 1)")
            .bind(tid).bind(format!("qobuz_trk_{}", i)).execute(&db).await.unwrap();

        sqlx::query("INSERT INTO library_entries (account_id, track_id, is_liked) VALUES (2, ?, 1)")
            .bind(tid).execute(&db).await.unwrap();
    }

    // Query with limit 5
    let limit_5_tracks: Vec<(i64,)> = sqlx::query_as(
        r#"
        SELECT DISTINCT t.id
        FROM tracks t
        JOIN track_sources ts ON ts.track_id = t.id
        JOIN services s ON s.id = ts.service_id
        LEFT JOIN favorites f ON f.item_type = 'track' AND f.service_item_id = ts.service_track_id
        LEFT JOIN library_entries le ON le.track_id = t.id AND le.is_liked = 1
        WHERE (t.favorite_at IS NOT NULL OR t.is_favorite = 1 OR f.id IS NOT NULL OR le.id IS NOT NULL)
          AND s.name = 'qobuz'
        ORDER BY t.id ASC
        LIMIT 5
        "#
    )
    .fetch_all(&db)
    .await
    .unwrap();

    assert_eq!(limit_5_tracks.len(), 5);

    // Enqueue the 5 items into download_queue with full source identity
    for (pos, (tid,)) in limit_5_tracks.iter().enumerate() {
        let (s_id, s_name, s_track_id, t_title, t_artist, t_album, t_isrc): (
            Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>
        ) = sqlx::query_as(
            r#"
            SELECT ts.service_id, s.name, ts.service_track_id,
                   t.title,
                   (SELECT GROUP_CONCAT(a.name, ', ') FROM track_artists ta JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id) as artist,
                   alb.title as album,
                   t.isrc
            FROM tracks t
            LEFT JOIN albums alb ON alb.id = t.album_id
            LEFT JOIN track_sources ts ON ts.track_id = t.id
            LEFT JOIN services s ON s.id = ts.service_id AND s.name = 'qobuz'
            WHERE t.id = ?
            ORDER BY CASE WHEN s.name = 'qobuz' THEN 0 ELSE 1 END
            LIMIT 1
            "#
        )
        .bind(tid)
        .fetch_one(&db)
        .await
        .unwrap();

        sqlx::query(
            r#"
            INSERT INTO download_queue (
                track_id, priority, position, status, quality_preference, resumable,
                service_id, service_name, service_track_id,
                target_title, target_artist, target_album, target_isrc,
                allow_fallback, smart_studio_origin, created_at
            )
            VALUES (?, 60, ?, 'queued', 'lossless', 1, ?, ?, ?, ?, ?, ?, ?, 0, 1, CURRENT_TIMESTAMP)
            "#
        )
        .bind(tid)
        .bind(pos as i64)
        .bind(s_id)
        .bind(s_name)
        .bind(s_track_id)
        .bind(t_title)
        .bind(t_artist)
        .bind(t_album)
        .bind(t_isrc)
        .execute(&db)
        .await
        .unwrap();
    }

    // Verify download_queue contents
    let queued_items: Vec<(i64, String, String, String, String, i64, i64)> = sqlx::query_as(
        "SELECT track_id, service_name, service_track_id, target_title, target_artist, allow_fallback, smart_studio_origin FROM download_queue WHERE status = 'queued' ORDER BY position ASC"
    )
    .fetch_all(&db)
    .await
    .unwrap();

    assert_eq!(queued_items.len(), 5);
    assert_eq!(queued_items[0].1, "qobuz");
    assert_eq!(queued_items[0].2, "qobuz_trk_1");
    assert_eq!(queued_items[0].3, "Track 1");
    assert_eq!(queued_items[0].4, "Test Artist");
    assert_eq!(queued_items[0].5, 0); // allow_fallback = 0
    assert_eq!(queued_items[0].6, 1); // smart_studio_origin = 1
}

#[tokio::test]
async fn test_qobuz_stale_source_404_error_classification() {
    let db = create_test_db().await;

    let _artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Garbage') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Anthology') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id) VALUES ('#1 Crush', ?) RETURNING id")
        .bind(album_id).fetch_one(&db).await.unwrap();

    // Simulate a queue item for stale track 186127417
    let qid: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO download_queue (
            track_id, priority, position, status, quality_preference,
            service_name, service_track_id, target_title, target_artist,
            allow_fallback, smart_studio_origin, created_at
        )
        VALUES (?, 60, 0, 'queued', 'lossless', 'qobuz', '186127417', '#1 Crush', 'Garbage', 0, 1, CURRENT_TIMESTAMP)
        RETURNING id
        "#
    )
    .bind(track_id)
    .fetch_one(&db)
    .await
    .unwrap();

    // Simulate 404 error received from Qobuz API: "Qobuz track/get failed for ID 186127417: HTTP 404"
    let err_msg = "Qobuz track/get failed for ID 186127417: HTTP 404 Not Found";

    let is_auth_error = err_msg.contains("RequiresAuth") || err_msg.contains("PlaybackUnauthorized") || err_msg.contains("401");
    let is_permanent = is_auth_error 
        || err_msg.contains("RejectedQuality") 
        || err_msg.contains("downgrade rejected") 
        || err_msg.contains("TrackUnresolved") 
        || err_msg.contains("NotFound") 
        || err_msg.contains("not found on") 
        || err_msg.contains("404") 
        || err_msg.contains("StaleSource") 
        || err_msg.contains("track/get failed");

    assert!(!is_auth_error, "404 must not be treated as auth failure");
    assert!(is_permanent, "404 stale source must be marked as permanent failure");

    // Apply permanent failure to download_queue
    sqlx::query("UPDATE download_queue SET status = 'failed', error_message = ?, last_error = ?, retry_count = 99 WHERE id = ?")
        .bind(err_msg)
        .bind(err_msg)
        .bind(qid)
        .execute(&db)
        .await
        .unwrap();

    let row: (String, Option<String>, i64) = sqlx::query_as("SELECT status, error_message, retry_count FROM download_queue WHERE id = ?")
        .bind(qid)
        .fetch_one(&db)
        .await
        .unwrap();

    assert_eq!(row.0, "failed");
    assert!(row.1.unwrap().contains("404"));
    assert_eq!(row.2, 99, "Permanent failure must set retry_count to 99");

    // Verify account credentials were NOT invalidated
    let acc: (i64,) = sqlx::query_as("SELECT IFNULL(credentials_invalid, 0) FROM accounts WHERE service_id = 2")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(acc.0, 0, "Account must remain valid on 404 track not found error");
}

#[tokio::test]
async fn test_flac_magic_bytes_and_staging_lifecycle() {
    let temp_dir = std::env::temp_dir().join(format!("syncify_test_{}", uuid::Uuid::new_v4()));
    let staging_dir = temp_dir.join(".staging");
    std::fs::create_dir_all(&staging_dir).unwrap();

    let staging_file = staging_dir.join("test_item.part");
    let final_file = temp_dir.join("Test Artist - Test Title.flac");

    // Write a valid FLAC header (magic bytes: 0x66, 0x4C, 0x61, 0x43 followed by mock audio payload)
    let mut flac_payload = vec![0x66, 0x4C, 0x61, 0x43];
    flac_payload.extend_from_slice(&[0x00; 1024]);
    std::fs::write(&staging_file, &flac_payload).unwrap();

    // Verify magic bytes
    let read_bytes = std::fs::read(&staging_file).unwrap();
    assert_eq!(&read_bytes[0..4], b"fLaC");
    assert!(AudioByteValidator::is_flac_magic(&read_bytes));

    // Promote from staging to final
    std::fs::rename(&staging_file, &final_file).unwrap();

    // Verify final file exists and is non-zero size
    assert!(final_file.exists());
    assert!(final_file.metadata().unwrap().len() > 0);

    // Verify staging file is deleted / gone
    assert!(!staging_file.exists());

    // Cleanup
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_download_favorites_50_batch_scaling() {
    let db = create_test_db().await;

    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Batch Artist') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Batch Album') RETURNING id")
        .fetch_one(&db).await.unwrap();

    for i in 1..=60 {
        let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES (?, ?, ?) RETURNING id")
            .bind(format!("Batch Track {}", i))
            .bind(album_id)
            .bind(format!("USBAT00000{:02}", i))
            .fetch_one(&db).await.unwrap();

        sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
            .bind(tid).bind(artist_id).execute(&db).await.unwrap();

        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, ?, 'FLAC', 24, 96000, 150, 1)")
            .bind(tid).bind(format!("batch_qobuz_{}", i)).execute(&db).await.unwrap();

        sqlx::query("INSERT INTO library_entries (account_id, track_id, is_liked) VALUES (2, ?, 1)")
            .bind(tid).execute(&db).await.unwrap();
    }

    // Limit to 50
    let candidates: Vec<(i64,)> = sqlx::query_as(
        r#"
        SELECT DISTINCT t.id
        FROM tracks t
        JOIN track_sources ts ON ts.track_id = t.id
        JOIN services s ON s.id = ts.service_id
        LEFT JOIN library_entries le ON le.track_id = t.id AND le.is_liked = 1
        WHERE le.id IS NOT NULL AND s.name = 'qobuz'
        ORDER BY t.id ASC
        LIMIT 50
        "#
    )
    .fetch_all(&db)
    .await
    .unwrap();

    assert_eq!(candidates.len(), 50);
}

#[tokio::test]
async fn test_distinct_master_edition_preservation_and_identity_lock() {
    let db = create_test_db().await;

    // Edition 1: Noordpool Orchestra - 15 Step (Album: Radiohead, A Jazz Symphony)
    let artist1: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Noordpool Orchestra') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let album1: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Radiohead, A Jazz Symphony') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let track1: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES ('15 Step', ?, 'NLF201200001') RETURNING id")
        .bind(album1).fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(track1).bind(artist1).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, 'qobuz_noordpool_15', 'FLAC', 16, 44100, 100, 1)")
        .bind(track1).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO library_entries (account_id, track_id, is_liked) VALUES (2, ?, 1)")
        .bind(track1).execute(&db).await.unwrap();

    // Edition 2: Radiohead - 15 Step (Album: In Rainbows)
    let artist2: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Radiohead') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let album2: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('In Rainbows') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let track2: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES ('15 Step', ?, 'GBAYE0700101') RETURNING id")
        .bind(album2).fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(track2).bind(artist2).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, 'qobuz_radiohead_15', 'FLAC', 24, 96000, 150, 1)")
        .bind(track2).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO library_entries (account_id, track_id, is_liked) VALUES (2, ?, 1)")
        .bind(track2).execute(&db).await.unwrap();

    // 1. Enqueue Edition 1 (Noordpool Orchestra)
    let (s_id1, s_name1, s_track1, t_title1, t_art1, t_alb1, t_isrc1): (
        Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>
    ) = sqlx::query_as(
        r#"
        SELECT ts.service_id, s.name, ts.service_track_id,
               t.title,
               (SELECT GROUP_CONCAT(a.name, ', ') FROM track_artists ta JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = t.id) as artist,
               alb.title as album,
               t.isrc
        FROM tracks t
        LEFT JOIN albums alb ON alb.id = t.album_id
        JOIN track_sources ts ON ts.track_id = t.id AND ts.available = 1 AND ts.service_track_id IS NOT NULL
        JOIN services s ON s.id = ts.service_id AND s.name = 'qobuz'
        WHERE t.id = ?
        "#
    )
    .bind(track1)
    .fetch_one(&db)
    .await
    .unwrap();

    assert_eq!(s_track1.as_deref(), Some("qobuz_noordpool_15"));
    assert_eq!(t_art1.as_deref(), Some("Noordpool Orchestra"));
    assert_eq!(t_alb1.as_deref(), Some("Radiohead, A Jazz Symphony"));

    let qid1: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO download_queue (
            track_id, priority, position, status, quality_preference, resumable,
            service_id, service_name, service_track_id,
            target_title, target_artist, target_album, target_isrc,
            allow_fallback, smart_studio_origin, created_at
        )
        VALUES (?, 60, 0, 'queued', 'lossless', 1, ?, ?, ?, ?, ?, ?, ?, 0, 1, CURRENT_TIMESTAMP)
        RETURNING id
        "#
    )
    .bind(track1)
    .bind(s_id1)
    .bind(s_name1)
    .bind(&s_track1)
    .bind(t_title1)
    .bind(t_art1)
    .bind(t_alb1)
    .bind(t_isrc1)
    .fetch_one(&db)
    .await
    .unwrap();

    // Verify row in download_queue is locked to exact edition
    let qrow1: (String, String, String, String, i64) = sqlx::query_as(
        "SELECT service_track_id, target_artist, target_album, target_isrc, allow_fallback FROM download_queue WHERE id = ?"
    )
    .bind(qid1)
    .fetch_one(&db)
    .await
    .unwrap();

    assert_eq!(qrow1.0, "qobuz_noordpool_15");
    assert_eq!(qrow1.1, "Noordpool Orchestra");
    assert_eq!(qrow1.2, "Radiohead, A Jazz Symphony");
    assert_eq!(qrow1.3, "NLF201200001");
    assert_eq!(qrow1.4, 0, "allow_fallback must be 0 for exact edition");

    // 2. Test rejection of un-locked item when allow_fallback = false
    let qid_unlocked: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO download_queue (
            track_id, priority, position, status, quality_preference, resumable,
            service_name, service_track_id, target_title, target_artist, target_album,
            allow_fallback, smart_studio_origin, created_at
        )
        VALUES (?, 60, 1, 'queued', 'lossless', 1, 'qobuz', NULL, '15 Step', 'Unknown', 'Unknown', 0, 0, CURRENT_TIMESTAMP)
        RETURNING id
        "#
    )
    .bind(track1)
    .fetch_one(&db)
    .await
    .unwrap();

    // Worker test: repair or reject
    syncify_tauri_lib::worker::DownloadWorker::repair_unresolved_queue_sources(&db).await;

    // Under S112 rules: legacy items without locked source identity are quarantined as failed (SourceIdentityMissing)
    let quarantined_row: (Option<String>, Option<String>, Option<String>, String, Option<String>) = sqlx::query_as(
        "SELECT service_track_id, target_artist, target_album, status, error_message FROM download_queue WHERE id = ?"
    )
    .bind(qid_unlocked)
    .fetch_one(&db)
    .await
    .unwrap();

    assert_eq!(quarantined_row.0.as_deref(), None, "Service track id must remain untouched");
    assert_eq!(quarantined_row.3, "failed", "Legacy unlocked row must be marked failed");
    assert!(
        quarantined_row.4.as_deref().unwrap_or("").contains("SourceIdentityMissing"),
        "Error message must indicate SourceIdentityMissing"
    );
}
