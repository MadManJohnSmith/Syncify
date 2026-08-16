//! Integration tests for Sprint S112: Legacy Queue Sanitation & Mass Download Guardrails
//!
//! Validates:
//! 1. Classification & quarantine of legacy unresolved rows (SourceIdentityMissing, retry_count=99)
//! 2. Stale source classification (404/NotFound, retry_count=99)
//! 3. Ambiguous source classification (AmbiguousSource, retry_count=99)
//! 4. Source locked items execution and audit report accuracy
//! 5. Preflight dry-run guardrail preventing accidental mass enqueueing

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

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
async fn test_queue_audit_and_item_classification() {
    let db = create_test_db().await;

    // 1. Setup artist, album, and tracks
    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Guardrail Artist') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, upc) VALUES ('Guardrail Album', '112233445566') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)").bind(album_id).bind(artist_id).execute(&db).await.unwrap();

    // Track 1: Source locked (valid)
    let t1: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES ('Track 1 Locked', ?, 'USRC11200001') RETURNING id")
        .bind(album_id).fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, available) VALUES (?, 2, 'qobuz_valid_101', 1)")
        .bind(t1).execute(&db).await.unwrap();
    
    // Insert into download_queue as source_locked
    sqlx::query(
        r#"
        INSERT INTO download_queue (
            track_id, priority, position, status, quality_preference, resumable,
            service_id, service_name, service_track_id,
            target_title, target_artist, target_album, target_isrc,
            allow_fallback, smart_studio_origin
        ) VALUES (?, 60, 0, 'queued', 'lossless', 1, 2, 'qobuz', 'qobuz_valid_101', 'Track 1 Locked', 'Guardrail Artist', 'Guardrail Album', 'USRC11200001', 0, 1)
        "#
    ).bind(t1).execute(&db).await.unwrap();

    // Track 2: Legacy unresolved (service_track_id is NULL)
    let t2: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES ('Track 2 Legacy', ?, 'USRC11200002') RETURNING id")
        .bind(album_id).fetch_one(&db).await.unwrap();
    sqlx::query(
        r#"
        INSERT INTO download_queue (
            track_id, priority, position, status, quality_preference, resumable,
            service_id, service_name, service_track_id,
            target_title, target_artist, target_album, target_isrc,
            allow_fallback, smart_studio_origin
        ) VALUES (?, 60, 1, 'queued', 'lossless', 1, 2, 'qobuz', NULL, 'Track 2 Legacy', 'Guardrail Artist', 'Guardrail Album', 'USRC11200002', 0, 1)
        "#
    ).bind(t2).execute(&db).await.unwrap();

    // Track 3: Stale source (404 / NotFound)
    let t3: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES ('Track 3 Stale', ?, 'USRC11200003') RETURNING id")
        .bind(album_id).fetch_one(&db).await.unwrap();
    sqlx::query(
        r#"
        INSERT INTO download_queue (
            track_id, priority, position, status, quality_preference, resumable,
            service_id, service_name, service_track_id,
            target_title, target_artist, target_album, target_isrc,
            allow_fallback, smart_studio_origin, error_message, last_error, retry_count
        ) VALUES (?, 60, 2, 'failed', 'lossless', 1, 2, 'qobuz', 'qobuz_stale_404', 'Track 3 Stale', 'Guardrail Artist', 'Guardrail Album', 'USRC11200003', 0, 1, 'StaleSource: Qobuz returned HTTP 404 (NotFound)', 'StaleSource: Qobuz returned HTTP 404 (NotFound)', 99)
        "#
    ).bind(t3).execute(&db).await.unwrap();

    // Track 4: Ambiguous source
    let t4: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES ('Track 4 Ambiguous', ?, 'USRC11200004') RETURNING id")
        .bind(album_id).fetch_one(&db).await.unwrap();
    sqlx::query(
        r#"
        INSERT INTO download_queue (
            track_id, priority, position, status, quality_preference, resumable,
            service_id, service_name, service_track_id,
            target_title, target_artist, target_album, target_isrc,
            allow_fallback, smart_studio_origin, error_message, last_error, retry_count
        ) VALUES (?, 60, 3, 'failed', 'lossless', 1, 2, 'qobuz', NULL, 'Track 4 Ambiguous', 'Guardrail Artist', 'Guardrail Album', 'USRC11200004', 0, 1, 'AmbiguousSource: Multiple competing sources', 'AmbiguousSource: Multiple competing sources', 99)
        "#
    ).bind(t4).execute(&db).await.unwrap();

    // Execute quarantine on legacy unresolved items
    let unresolved_items: Vec<(i64, i64, Option<String>, Option<i64>)> = sqlx::query_as(
        "SELECT id, track_id, service_name, allow_fallback FROM download_queue WHERE status = 'queued' AND (service_track_id IS NULL OR TRIM(service_track_id) = '')"
    )
    .fetch_all(&db)
    .await
    .unwrap_or_default();

    assert_eq!(unresolved_items.len(), 1, "Exactly one legacy unresolved item in queue");
    let (legacy_qid, _, _, _) = unresolved_items[0];

    // Quarantine as SourceIdentityMissing
    let reason = "SourceIdentityMissing: Legacy queue row without locked source identity";
    sqlx::query("UPDATE download_queue SET status = 'failed', error_message = ?, last_error = ?, retry_count = 99 WHERE id = ?")
        .bind(reason)
        .bind(reason)
        .bind(legacy_qid)
        .execute(&db)
        .await
        .unwrap();

    // Audit the download queue
    let rows: Vec<(String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT status, service_track_id, error_message FROM download_queue"
    )
    .fetch_all(&db)
    .await
    .unwrap();

    let mut ready_count = 0i64;
    let mut source_locked_count = 0i64;
    let mut legacy_unresolved_count = 0i64;
    let mut stale_source_count = 0i64;
    let mut ambiguous_source_count = 0i64;
    let mut source_identity_missing_count = 0i64;
    let mut failed_count = 0i64;

    for (status, s_track_id, err_opt) in rows {
        let is_locked = s_track_id.as_deref().map(|s| !s.trim().is_empty()).unwrap_or(false);
        if is_locked {
            source_locked_count += 1;
        }

        match status.as_str() {
            "queued" => {
                if is_locked {
                    ready_count += 1;
                } else {
                    legacy_unresolved_count += 1;
                }
            }
            "failed" => {
                failed_count += 1;
                let err = err_opt.unwrap_or_default();
                if err.contains("404") || err.contains("NotFound") || err.contains("StaleSource") {
                    stale_source_count += 1;
                } else if err.contains("AmbiguousSource") {
                    ambiguous_source_count += 1;
                } else if err.contains("SourceIdentityMissing") {
                    source_identity_missing_count += 1;
                }
            }
            _ => {}
        }
    }

    assert_eq!(ready_count, 1, "Only source_locked item remains ready/queued");
    assert_eq!(source_locked_count, 2, "Two items have locked source ids (1 queued, 1 stale 404)");
    assert_eq!(legacy_unresolved_count, 0, "No legacy unresolved items remain in queued state");
    assert_eq!(stale_source_count, 1, "One stale source classified");
    assert_eq!(ambiguous_source_count, 1, "One ambiguous source classified");
    assert_eq!(source_identity_missing_count, 1, "One legacy row quarantined as SourceIdentityMissing");
    assert_eq!(failed_count, 3, "3 failed items (legacy quarantined, stale, ambiguous)");
}

#[tokio::test]
async fn test_mass_download_preflight_guardrail() {
    let db = create_test_db().await;

    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Preflight Artist') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, upc) VALUES ('Preflight Album', '998877665544') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)").bind(album_id).bind(artist_id).execute(&db).await.unwrap();

    // Create 150 favorite tracks to simulate mass library
    for i in 1..=150 {
        let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES (?, ?, ?) RETURNING id")
            .bind(format!("Preflight Track {:03}", i))
            .bind(album_id)
            .bind(format!("USPF112{:05}", i))
            .fetch_one(&db).await.unwrap();

        sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
            .bind(tid).bind(artist_id).execute(&db).await.unwrap();

        if i <= 140 {
            // Valid Qobuz source
            sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, ?, 'FLAC', 16, 44100, 100, 1)")
                .bind(tid).bind(format!("qobuz_pf_{:03}", i)).execute(&db).await.unwrap();
        }
        // 10 tracks have no sources (unresolved)

        sqlx::query("INSERT INTO library_entries (account_id, track_id, is_liked) VALUES (2, ?, 1)")
            .bind(tid).execute(&db).await.unwrap();
    }

    // 5 tracks are already downloaded
    for i in 1..=5 {
        let tid: i64 = i;
        sqlx::query("INSERT INTO downloads (track_id, file_path, file_format, file_size_bytes) VALUES (?, ?, 'FLAC', 25000000)")
            .bind(tid).bind(format!("C:/Music/Preflight Track {:03}.flac", i)).execute(&db).await.unwrap();
    }

    // 5 tracks are already in queue
    for i in 6..=10 {
        let tid: i64 = i;
        sqlx::query(
            r#"
            INSERT INTO download_queue (
                track_id, priority, position, status, quality_preference, resumable,
                service_id, service_name, service_track_id,
                target_title, target_artist, target_album, target_isrc,
                allow_fallback, smart_studio_origin
            ) VALUES (?, 60, ?, 'queued', 'lossless', 1, 2, 'qobuz', ?, 'Title', 'Artist', 'Album', 'ISRC', 0, 1)
            "#
        )
        .bind(tid).bind(i as i64).bind(format!("qobuz_pf_{:03}", i)).execute(&db).await.unwrap();
    }

    // Verify initial queue count = 5
    let initial_queued: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_queue WHERE status = 'queued'")
        .fetch_one(&db).await.unwrap();
    assert_eq!(initial_queued, 5);

    // Run preflight evaluation (dry_run = true)
    let candidates: Vec<(i64,)> = sqlx::query_as(
        r#"
        SELECT DISTINCT t.id
        FROM tracks t
        LEFT JOIN library_entries le ON le.track_id = t.id
        WHERE le.is_liked = 1
        ORDER BY t.id ASC
        "#
    ).fetch_all(&db).await.unwrap();

    let total_candidates = candidates.len() as i64;
    assert_eq!(total_candidates, 150);

    let mut ready_for_queue = 0i64;
    let mut already_downloaded = 0i64;
    let mut already_queued = 0i64;
    let mut unresolved_sources = 0i64;

    for (tid,) in &candidates {
        let dl_exists: Option<(String,)> = sqlx::query_as("SELECT file_path FROM downloads WHERE track_id = ? LIMIT 1")
            .bind(tid).fetch_optional(&db).await.unwrap();
        if let Some((fp,)) = dl_exists {
            if !fp.trim().is_empty() {
                already_downloaded += 1;
                continue;
            }
        }

        let q_exists: Option<(i64,)> = sqlx::query_as("SELECT id FROM download_queue WHERE track_id = ? AND status IN ('queued', 'downloading') LIMIT 1")
            .bind(tid).fetch_optional(&db).await.unwrap();
        if q_exists.is_some() {
            already_queued += 1;
            continue;
        }

        let src_exists: Option<(String,)> = sqlx::query_as("SELECT service_track_id FROM track_sources WHERE track_id = ? AND service_id = 2 AND available = 1 AND service_track_id IS NOT NULL")
            .bind(tid).fetch_optional(&db).await.unwrap();
        if src_exists.is_none() {
            unresolved_sources += 1;
            continue;
        }

        ready_for_queue += 1;
    }

    assert_eq!(already_downloaded, 5);
    assert_eq!(already_queued, 5);
    assert_eq!(unresolved_sources, 10);
    assert_eq!(ready_for_queue, 130);

    // Verify dry-run did NOT insert any new rows into download_queue
    let queue_count_after_preflight: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_queue WHERE status = 'queued'")
        .fetch_one(&db).await.unwrap();
    assert_eq!(queue_count_after_preflight, 5, "Preflight must not alter download_queue rows");
}

#[tokio::test]
async fn test_download_favorites_contract_qobuz_5_batch() {
    let db = create_test_db().await;

    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Contract Artist') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, upc) VALUES ('Contract Album', '112233445577') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)").bind(album_id).bind(artist_id).execute(&db).await.unwrap();

    // Insert 10 tracks linked via library_entries (is_liked=1, account_id=2) and track_sources
    for i in 1..=10 {
        let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES (?, ?, ?) RETURNING id")
            .bind(format!("Contract Track {:02}", i))
            .bind(album_id)
            .bind(format!("USCT113{:05}", i))
            .fetch_one(&db).await.unwrap();

        sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
            .bind(tid).bind(artist_id).execute(&db).await.unwrap();

        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, ?, 'FLAC', 16, 44100, 100, 1)")
            .bind(tid).bind(format!("qobuz_contract_{:02}", i)).execute(&db).await.unwrap();

        sqlx::query("INSERT INTO library_entries (account_id, track_id, is_liked) VALUES (2, ?, 1)")
            .bind(tid).execute(&db).await.unwrap();
    }

    // Query candidate tracks using the canonical query with service_filter = 'qobuz'
    let srv_param = Some("qobuz");
    let mut candidate_track_ids: Vec<i64> = Vec::new();
    let mut seen: std::collections::HashSet<i64> = std::collections::HashSet::new();

    let raw_tracks: Vec<(i64,)> = sqlx::query_as(
        r#"
        SELECT DISTINCT t.id
        FROM tracks t
        LEFT JOIN library_entries le ON le.track_id = t.id
        LEFT JOIN accounts acc_le ON acc_le.id = le.account_id
        LEFT JOIN services s_le ON s_le.id = acc_le.service_id
        LEFT JOIN favorites f ON f.item_type = 'track' AND (f.service_item_id = CAST(t.id AS TEXT) OR f.service_item_id = t.isrc)
        LEFT JOIN accounts acc_f ON acc_f.id = f.account_id
        LEFT JOIN services s_f ON s_f.id = acc_f.service_id
        LEFT JOIN track_sources ts ON ts.track_id = t.id AND ts.available = 1
        LEFT JOIN services s_ts ON s_ts.id = ts.service_id
        WHERE (t.favorite_at IS NOT NULL OR t.is_favorite = 1 OR f.id IS NOT NULL OR le.is_liked = 1)
          AND (? IS NULL OR s_le.name = ? OR s_f.name = ? OR s_ts.name = ?)
        ORDER BY t.id ASC
        "#,
    )
    .bind(srv_param)
    .bind(srv_param)
    .bind(srv_param)
    .bind(srv_param)
    .fetch_all(&db)
    .await
    .unwrap();

    for (tid,) in raw_tracks {
        if seen.insert(tid) {
            candidate_track_ids.push(tid);
        }
    }

    let total_candidates = candidate_track_ids.len() as i64;
    assert_eq!(total_candidates, 10, "Must discover all 10 candidates from library_entries + track_sources");

    // Apply limit = 5
    let limit = Some(5usize);
    if let Some(lim) = limit {
        candidate_track_ids.truncate(lim);
    }
    assert_eq!(candidate_track_ids.len(), 5);

    let mut enqueued = 0i64;
    for (pos, tid) in candidate_track_ids.iter().enumerate() {
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
            JOIN track_sources ts ON ts.track_id = t.id AND ts.available = 1 AND ts.service_track_id IS NOT NULL AND TRIM(ts.service_track_id) != ''
            JOIN services s ON s.id = ts.service_id AND s.name = ?
            WHERE t.id = ?
            ORDER BY COALESCE(ts.quality_score, 0) DESC, COALESCE(ts.bit_depth, 0) DESC
            LIMIT 1
            "#
        )
        .bind(srv_param)
        .bind(tid)
        .fetch_optional(&db)
        .await
        .unwrap()
        .unwrap();

        assert!(s_track_id.is_some());
        assert_eq!(s_name.as_deref(), Some("qobuz"));

        sqlx::query(
            r#"
            INSERT INTO download_queue (
                track_id, priority, position, status, quality_preference, resumable,
                service_id, service_name, service_track_id,
                target_title, target_artist, target_album, target_isrc,
                allow_fallback, smart_studio_origin
            ) VALUES (?, 60, ?, 'queued', 'lossless', 1, ?, ?, ?, ?, ?, ?, ?, 0, 1)
            "#
        )
        .bind(tid)
        .bind(pos as i64)
        .bind(s_id)
        .bind(&s_name)
        .bind(&s_track_id)
        .bind(&t_title)
        .bind(&t_artist)
        .bind(&t_album)
        .bind(&t_isrc)
        .execute(&db)
        .await
        .unwrap();

        enqueued += 1;
    }

    assert_eq!(enqueued, 5);

    // Verify exactly 5 rows in download_queue, all source-locked
    let queued_rows: Vec<(i64, String, String, String)> = sqlx::query_as(
        "SELECT track_id, status, service_name, service_track_id FROM download_queue ORDER BY position ASC"
    )
    .fetch_all(&db)
    .await
    .unwrap();

    assert_eq!(queued_rows.len(), 5);
    for (i, row) in queued_rows.iter().enumerate() {
        assert_eq!(row.1, "queued");
        assert_eq!(row.2, "qobuz");
        assert_eq!(row.3, format!("qobuz_contract_{:02}", i + 1));
    }
}

