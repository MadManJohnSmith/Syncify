//! Integration test suite for S138A: Preflight de Descargabilidad y Selección Segura de Lote
//!
//! Tests:
//! 1. Lote Spotify sin mapping: NoDownloadProvider, no encola, mensaje explicativo.
//! 2. Qobuz exacto: ReadyExactSource, cuenta activa, encolable.
//! 3. Fallback ISRC: ReadyFallbackExactIdentity, resuelve proveedor descargable.
//! 4. Ambiguous title: Coincidencia débil por título/artista clasifica como AmbiguousSource y no se encola automáticamente.
//! 5. Tidal AAC con strict: Calidad inferior AAC/MP3 rechazada con strict activo -> RejectedQuality.
//! 6. Already downloaded: Pista ya descargada clasifica como AlreadyDownloaded y se separa en contadores.
//! 7. Already queued: Pista ya en cola clasifica como AlreadyQueued y se separa en contadores.
//! 8. Conteos exactos: Resumen de preflight cuadra exactamente con las clasificaciones individuales.
//! 9. Batch solo encola elegibles: Un lote mixto solo encola ReadyExactSource y ReadyFallbackExactIdentity en download_queue.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use syncify_tauri_lib::commands::{
    evaluate_track_preflight, DownloadPreflightStatus,
};

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
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, email, is_active) VALUES (1, 1, 'Spotify User', 'user@spotify.com', 1)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, email, is_active) VALUES (2, 2, 'Qobuz User', 'user@qobuz.com', 1)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, email, is_active) VALUES (3, 3, 'Tidal User', 'user@tidal.com', 1)")
        .execute(&pool).await.unwrap();

    pool
}

#[tokio::test]
async fn test_1_spotify_track_without_mapping_is_no_download_provider() {
    let db = create_test_db().await;

    // Track imported from Spotify without Qobuz/Tidal source
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Spotify Only Song') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO library_entries (account_id, track_id) VALUES (1, ?)")
        .bind(track_id).execute(&db).await.unwrap();

    let res = evaluate_track_preflight(&db, track_id, None, None, false, true).await.unwrap();

    assert_eq!(res.status, DownloadPreflightStatus::NoDownloadProvider);
    assert!(!res.is_eligible);
    assert!(res.reason.contains("Spotify"));
}

#[tokio::test]
async fn test_2_qobuz_exact_with_active_account_is_ready_exact_source() {
    let db = create_test_db().await;

    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('Qobuz Master Track', 'FR01A2400001') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, 'qobuz_12345', 'FLAC', 24, 96000, 120, 1)")
        .bind(track_id).execute(&db).await.unwrap();

    let res = evaluate_track_preflight(&db, track_id, Some("qobuz"), Some("hires"), false, true).await.unwrap();

    assert_eq!(res.status, DownloadPreflightStatus::ReadyExactSource);
    assert!(res.is_eligible);
    assert_eq!(res.resolved_service_name.as_deref(), Some("qobuz"));
    assert_eq!(res.resolved_service_track_id.as_deref(), Some("qobuz_12345"));
}

#[tokio::test]
async fn test_3_fallback_exact_isrc_resolves_ready_fallback_exact_identity() {
    let db = create_test_db().await;

    let isrc_code = "USRC17605432";
    // Track with stale direct source on Qobuz, but active Tidal source matching exact ISRC
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('Heroes', ?) RETURNING id")
        .bind(isrc_code).fetch_one(&db).await.unwrap();

    // Qobuz direct source is stale 404
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available, availability_status) VALUES (?, 2, 'qobuz_stale_99', 'FLAC', 24, 96000, 120, 1, 'stale_404')")
        .bind(track_id).execute(&db).await.unwrap();

    // Tidal fallback source is active and valid
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 3, 'tidal_heroes_99', 'FLAC', 24, 96000, 120, 1)")
        .bind(track_id).execute(&db).await.unwrap();

    // Requesting Qobuz specifically encounters stale direct source, triggering fallback to Tidal
    let res = evaluate_track_preflight(&db, track_id, Some("qobuz"), Some("lossless"), false, true).await.unwrap();

    assert_eq!(res.status, DownloadPreflightStatus::ReadyFallbackExactIdentity);
    assert!(res.is_eligible);
    assert_eq!(res.resolved_service_name.as_deref(), Some("tidal"));
    assert_eq!(res.resolved_service_track_id.as_deref(), Some("tidal_heroes_99"));
    assert_eq!(res.match_method.as_deref(), Some("exact_isrc"));
}

#[tokio::test]
async fn test_4_ambiguous_title_artist_without_exact_identity_is_ambiguous_source() {
    let db = create_test_db().await;

    // Track 1: Source track without ISRC or MBID
    let track_id_1: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Loose Title Song') RETURNING id")
        .fetch_one(&db).await.unwrap();

    // Track 2: Tidal track that shares the same title loosely but no ISRC/MBID
    let track_id_2: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Loose Title Song') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 3, 'tidal_loose_44', 'FLAC', 16, 44100, 80, 1)")
        .bind(track_id_2).execute(&db).await.unwrap();

    let res = evaluate_track_preflight(&db, track_id_1, None, Some("lossless"), false, true).await.unwrap();

    assert_eq!(res.status, DownloadPreflightStatus::AmbiguousSource);
    assert!(!res.is_eligible);
    assert!(res.reason.contains("loose"));
}

#[tokio::test]
async fn test_5_tidal_aac_with_strict_quality_returns_rejected_quality() {
    let db = create_test_db().await;

    // Track requested Hi-Res with strict quality, but source is AAC/lossy
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('AAC Only Track') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 3, 'tidal_aac_1', 'AAC', 16, 44100, 40, 1)")
        .bind(track_id).execute(&db).await.unwrap();

    let res = evaluate_track_preflight(&db, track_id, Some("tidal"), Some("hires"), true, true).await.unwrap();

    assert_eq!(res.status, DownloadPreflightStatus::RejectedQuality);
    assert!(!res.is_eligible);
    assert!(res.reason.contains("strict"));
}

#[tokio::test]
async fn test_6_already_downloaded_track_is_separated() {
    let db = create_test_db().await;

    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Downloaded Song') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO downloads (track_id, file_path, source_service_id, file_format) VALUES (?, 'C:/Music/song.flac', 2, 'FLAC')")
        .bind(track_id).execute(&db).await.unwrap();

    let res = evaluate_track_preflight(&db, track_id, None, None, false, true).await.unwrap();

    assert_eq!(res.status, DownloadPreflightStatus::AlreadyDownloaded);
    assert!(!res.is_eligible);
    assert!(res.reason.contains("already downloaded"));
}

#[tokio::test]
async fn test_7_already_queued_track_is_separated() {
    let db = create_test_db().await;

    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('In Queue Song') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO download_queue (track_id, status, position, priority) VALUES (?, 'queued', 1, 50)")
        .bind(track_id).execute(&db).await.unwrap();

    let res = evaluate_track_preflight(&db, track_id, None, None, false, true).await.unwrap();

    assert_eq!(res.status, DownloadPreflightStatus::AlreadyQueued);
    assert!(!res.is_eligible);
    assert!(res.reason.contains("already in download queue"));
}

#[tokio::test]
async fn test_8_exact_summary_counts_match_evaluations() {
    let db = create_test_db().await;

    // 1. Qobuz exact
    let t1: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('T1 Qobuz', 'FR01') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, available) VALUES (?, 2, 'q_1', 'FLAC', 24, 1)")
        .bind(t1).execute(&db).await.unwrap();

    // 2. Fallback ISRC (direct source is stale on Qobuz, active on Tidal)
    let t2: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('T2 Target', 'ISRC222') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, available, availability_status) VALUES (?, 2, 'q_2', 'FLAC', 24, 1, 'stale_404')")
        .bind(t2).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, available) VALUES (?, 3, 'tid_2', 'FLAC', 16, 1)")
        .bind(t2).execute(&db).await.unwrap();

    // 3. Spotify only (NoDownloadProvider)
    let t3: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('T3 Spotify') RETURNING id")
        .fetch_one(&db).await.unwrap();

    // 4. Already downloaded
    let t4: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('T4 DL') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO downloads (track_id, file_path) VALUES (?, 'C:/Music/t4.flac')")
        .bind(t4).execute(&db).await.unwrap();

    // 5. Already queued
    let t5: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('T5 Queue') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO download_queue (track_id, status) VALUES (?, 'downloading')")
        .bind(t5).execute(&db).await.unwrap();

    let batch = vec![t1, t2, t3, t4, t5];
    let mut results = Vec::new();
    for (i, tid) in batch.iter().enumerate() {
        let req_service = if i == 1 { Some("qobuz") } else { None };
        let r = evaluate_track_preflight(&db, *tid, req_service, Some("lossless"), false, true).await.unwrap();
        results.push(r);
    }

    assert_eq!(results.len(), 5);
    assert_eq!(results[0].status, DownloadPreflightStatus::ReadyExactSource);
    assert_eq!(results[1].status, DownloadPreflightStatus::ReadyFallbackExactIdentity);
    assert_eq!(results[2].status, DownloadPreflightStatus::NoDownloadProvider);
    assert_eq!(results[3].status, DownloadPreflightStatus::AlreadyDownloaded);
    assert_eq!(results[4].status, DownloadPreflightStatus::AlreadyQueued);

    let eligible_count = results.iter().filter(|r| r.is_eligible).count();
    assert_eq!(eligible_count, 2);
}

#[tokio::test]
async fn test_9_batch_enqueues_only_eligible_tracks_into_download_queue() {
    let db = create_test_db().await;

    // Track 1: Qobuz exact -> ELIGIBLE
    let t1: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Batch Track 1') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, available) VALUES (?, 2, 'q_batch_1', 'FLAC', 24, 1)")
        .bind(t1).execute(&db).await.unwrap();

    // Track 2: Spotify only -> INELIGIBLE (NoDownloadProvider)
    let t2: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Batch Track 2') RETURNING id")
        .fetch_one(&db).await.unwrap();

    // Track 3: Already downloaded -> INELIGIBLE (AlreadyDownloaded)
    let t3: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Batch Track 3') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO downloads (track_id, file_path) VALUES (?, 'C:/Music/t3.flac')")
        .bind(t3).execute(&db).await.unwrap();

    // Track 4: Fallback ISRC -> ELIGIBLE (stale Qobuz, active Tidal)
    let t4: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('Batch Track 4', 'ISRC444') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, available, availability_status) VALUES (?, 2, 'q_batch_4', 'FLAC', 24, 1, 'stale_404')")
        .bind(t4).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, available) VALUES (?, 3, 'tid_batch_4', 'FLAC', 16, 1)")
        .bind(t4).execute(&db).await.unwrap();

    let batch = vec![t1, t2, t3, t4];

    // Evaluate preflight for each track and only insert eligible ones
    let mut enqueued = 0i64;
    for (i, tid) in batch.iter().enumerate() {
        let req_service = if i == 3 { Some("qobuz") } else { None };
        let pf = evaluate_track_preflight(&db, *tid, req_service, Some("lossless"), false, true).await.unwrap();
        if pf.is_eligible {
            sqlx::query(
                r#"
                INSERT INTO download_queue (
                    track_id, priority, position, status, quality_preference,
                    service_id, service_name, service_track_id, target_title,
                    allow_fallback
                ) VALUES (?, 50, ?, 'queued', ?, ?, ?, ?, ?, 1)
                "#
            )
            .bind(pf.track_id)
            .bind(enqueued)
            .bind(&pf.resolved_quality)
            .bind(pf.resolved_service_id)
            .bind(&pf.resolved_service_name)
            .bind(&pf.resolved_service_track_id)
            .bind(&pf.title)
            .execute(&db)
            .await
            .unwrap();

            enqueued += 1;
        }
    }

    assert_eq!(enqueued, 2, "Only the 2 eligible tracks (t1 and t4) must be enqueued");

    // Verify exactly 2 rows exist in download_queue
    let queued_rows: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT track_id, service_name, service_track_id FROM download_queue ORDER BY position ASC"
    )
    .fetch_all(&db)
    .await
    .unwrap();

    assert_eq!(queued_rows.len(), 2);
    assert_eq!(queued_rows[0].0, t1);
    assert_eq!(queued_rows[0].1, "qobuz");
    assert_eq!(queued_rows[0].2, "q_batch_1");

    assert_eq!(queued_rows[1].0, t4);
    assert_eq!(queued_rows[1].1, "tidal");
    assert_eq!(queued_rows[1].2, "tid_batch_4");
}

#[tokio::test]
async fn test_10_50_mixed_tracks_comprehensive_preflight_and_enqueue_audit() {
    let db = create_test_db().await;

    let mut all_tracks = Vec::new();

    // 1. 15 Qobuz exact (ReadyExactSource)
    for i in 1..=15 {
        let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES (?, ?) RETURNING id")
            .bind(format!("Qobuz Track {}", i))
            .bind(format!("FRQBZ00000{:02}", i))
            .fetch_one(&db).await.unwrap();
        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, ?, 'FLAC', 24, 96000, 120, 1)")
            .bind(tid).bind(format!("q_track_{}", i)).execute(&db).await.unwrap();
        all_tracks.push((tid, "qobuz_exact"));
    }

    // 2. 10 Tidal exact (ReadyExactSource)
    for i in 1..=10 {
        let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES (?, ?) RETURNING id")
            .bind(format!("Tidal Track {}", i))
            .bind(format!("USTID00000{:02}", i))
            .fetch_one(&db).await.unwrap();
        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 3, ?, 'FLAC', 16, 44100, 100, 1)")
            .bind(tid).bind(format!("t_track_{}", i)).execute(&db).await.unwrap();
        all_tracks.push((tid, "tidal_exact"));
    }

    // 3. 5 Fallback ISRC (ReadyFallbackExactIdentity)
    for i in 1..=5 {
        let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES (?, ?) RETURNING id")
            .bind(format!("Fallback Track {}", i))
            .bind(format!("USFBK00000{:02}", i))
            .fetch_one(&db).await.unwrap();
        // Stale Qobuz direct source
        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, available, availability_status) VALUES (?, 2, ?, 'FLAC', 24, 1, 'stale_404')")
            .bind(tid).bind(format!("q_stale_{}", i)).execute(&db).await.unwrap();
        // Active Tidal fallback source
        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, available) VALUES (?, 3, ?, 'FLAC', 16, 1)")
            .bind(tid).bind(format!("t_fbk_{}", i)).execute(&db).await.unwrap();
        all_tracks.push((tid, "fallback_isrc"));
    }

    // 4. 5 Spotify only without mapping (NoDownloadProvider)
    for i in 1..=5 {
        let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES (?) RETURNING id")
            .bind(format!("Spotify Only Track {}", i))
            .fetch_one(&db).await.unwrap();
        sqlx::query("INSERT INTO library_entries (account_id, track_id) VALUES (1, ?)")
            .bind(tid).execute(&db).await.unwrap();
        all_tracks.push((tid, "spotify_only"));
    }

    // 5. 5 Already downloaded (AlreadyDownloaded)
    for i in 1..=5 {
        let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES (?) RETURNING id")
            .bind(format!("Downloaded Track {}", i))
            .fetch_one(&db).await.unwrap();
        sqlx::query("INSERT INTO downloads (track_id, file_path) VALUES (?, ?)")
            .bind(tid).bind(format!("C:/Music/dl_{}.flac", i)).execute(&db).await.unwrap();
        all_tracks.push((tid, "already_downloaded"));
    }

    // 6. 5 Already queued (AlreadyQueued)
    for i in 1..=5 {
        let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES (?) RETURNING id")
            .bind(format!("Queued Track {}", i))
            .fetch_one(&db).await.unwrap();
        sqlx::query("INSERT INTO download_queue (track_id, status) VALUES (?, 'downloading')")
            .bind(tid).execute(&db).await.unwrap();
        all_tracks.push((tid, "already_queued"));
    }

    // 7. 3 Ambiguous multiple sources without service override (AmbiguousSource)
    for i in 1..=3 {
        let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES (?, ?) RETURNING id")
            .bind(format!("Ambiguous Track {}", i))
            .bind(format!("USAMB00000{:02}", i))
            .fetch_one(&db).await.unwrap();
        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, available) VALUES (?, 2, ?, 'FLAC', 24, 1)")
            .bind(tid).bind(format!("q_amb_{}", i)).execute(&db).await.unwrap();
        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, available) VALUES (?, 3, ?, 'FLAC', 16, 1)")
            .bind(tid).bind(format!("t_amb_{}", i)).execute(&db).await.unwrap();
        all_tracks.push((tid, "ambiguous"));
    }

    // 8. 2 Rejected Quality inferior to requested strict lossless (RejectedQuality)
    for i in 1..=2 {
        let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES (?) RETURNING id")
            .bind(format!("Low Quality Track {}", i))
            .fetch_one(&db).await.unwrap();
        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 3, ?, 'AAC', 16, 44100, 40, 1)")
            .bind(tid).bind(format!("t_lossy_{}", i)).execute(&db).await.unwrap();
        all_tracks.push((tid, "rejected_quality"));
    }

    assert_eq!(all_tracks.len(), 50, "Must have exactly 50 mixed test tracks");

    // Execute preflight over the 50 tracks
    let mut exact_count = 0;
    let mut fallback_count = 0;
    let mut no_provider_count = 0;
    let mut downloaded_count = 0;
    let mut queued_count = 0;
    let mut ambiguous_count = 0;
    let mut rejected_quality_count = 0;
    let mut eligible_tracks = Vec::new();

    for (tid, category) in &all_tracks {
        let req_service = if *category == "fallback_isrc" { Some("qobuz") } else { None };
        let strict = *category == "rejected_quality";
        let pf = evaluate_track_preflight(&db, *tid, req_service, Some("lossless"), strict, true).await.unwrap();

        match pf.status {
            DownloadPreflightStatus::ReadyExactSource => exact_count += 1,
            DownloadPreflightStatus::ReadyFallbackExactIdentity => fallback_count += 1,
            DownloadPreflightStatus::NoDownloadProvider => no_provider_count += 1,
            DownloadPreflightStatus::AlreadyDownloaded => downloaded_count += 1,
            DownloadPreflightStatus::AlreadyQueued => queued_count += 1,
            DownloadPreflightStatus::AmbiguousSource => ambiguous_count += 1,
            DownloadPreflightStatus::RejectedQuality => rejected_quality_count += 1,
            _ => {}
        }

        if pf.is_eligible {
            eligible_tracks.push(pf);
        }
    }

    // Verify exact breakdown (Dual provider tracks resolve as ReadyExactSource with primary preference)
    assert_eq!(exact_count, 28, "15 Qobuz + 10 Tidal + 3 Dual = 28 ReadyExactSource");
    assert_eq!(fallback_count, 5, "5 Fallback ISRC = 5 ReadyFallbackExactIdentity");
    assert_eq!(no_provider_count, 5, "5 Spotify = 5 NoDownloadProvider");
    assert_eq!(downloaded_count, 5, "5 Already downloaded = 5 AlreadyDownloaded");
    assert_eq!(queued_count, 5, "5 Already queued = 5 AlreadyQueued");
    assert_eq!(ambiguous_count, 0, "Dual-provider tracks are never excluded as AmbiguousSource");
    assert_eq!(rejected_quality_count, 2, "2 Low quality = 2 RejectedQuality");

    assert_eq!(eligible_tracks.len(), 33, "Exactly 33 out of 50 tracks must be eligible (28 exact + 5 fallback)");

    // Enqueue only eligible tracks
    let mut enqueued = 0i64;
    for pf in eligible_tracks {
        sqlx::query(
            r#"
            INSERT INTO download_queue (
                track_id, priority, position, status, quality_preference,
                service_id, service_name, service_track_id, target_title,
                allow_fallback
            ) VALUES (?, 50, ?, 'queued', ?, ?, ?, ?, ?, 1)
            "#
        )
        .bind(pf.track_id)
        .bind(enqueued)
        .bind(&pf.resolved_quality)
        .bind(pf.resolved_service_id)
        .bind(&pf.resolved_service_name)
        .bind(&pf.resolved_service_track_id)
        .bind(&pf.title)
        .execute(&db)
        .await
        .unwrap();

        enqueued += 1;
    }

    assert_eq!(enqueued, 33, "All eligible tracks must be enqueued: zero silent exclusions (S176Q)");

    // Verify database queue contents
    let queued_in_db: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_queue WHERE status = 'queued'")
        .fetch_one(&db).await.unwrap();
    assert_eq!(queued_in_db, 33); // the 5 preexisting are status='downloading', excluded from this count
}

#[tokio::test]
async fn test_10_large_500_track_batch_preflight() {
    let db = create_test_db().await;

    // Build 500-track library
    // 300 Qobuz/Tidal exact tracks (ReadyExactSource)
    // 50 ISRC fallback tracks (ReadyFallbackExactIdentity)
    // 50 Spotify unmapped (NoDownloadProvider)
    // 40 Already downloaded (AlreadyDownloaded)
    // 30 Already queued (AlreadyQueued)
    // 20 Ambiguous tracks (AmbiguousSource)
    // 10 Rejected quality tracks (RejectedQuality)

    let mut all_tracks = Vec::with_capacity(500);

    // 1. 300 Exact tracks (150 Qobuz, 150 Tidal)
    for i in 1..=150 {
        let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES (?, ?) RETURNING id")
            .bind(format!("Qobuz Exact Song {}", i))
            .bind(format!("USQOB500{:04}", i))
            .fetch_one(&db).await.unwrap();
        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, ?, 'FLAC', 24, 96000, 120, 1)")
            .bind(tid).bind(format!("q_exact_{}", i)).execute(&db).await.unwrap();
        all_tracks.push((tid, "exact"));
    }
    for i in 1..=150 {
        let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES (?, ?) RETURNING id")
            .bind(format!("Tidal Exact Song {}", i))
            .bind(format!("USTID500{:04}", i))
            .fetch_one(&db).await.unwrap();
        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 3, ?, 'FLAC', 16, 44100, 80, 1)")
            .bind(tid).bind(format!("t_exact_{}", i)).execute(&db).await.unwrap();
        all_tracks.push((tid, "exact"));
    }

    // 2. 50 Fallback ISRC tracks (Qobuz stale, Tidal fallback)
    for i in 1..=50 {
        let isrc = format!("USFALL500{:04}", i);
        let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES (?, ?) RETURNING id")
            .bind(format!("Fallback Song {}", i))
            .bind(&isrc)
            .fetch_one(&db).await.unwrap();
        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available, availability_status) VALUES (?, 2, ?, 'FLAC', 24, 96000, 120, 1, 'stale_404')")
            .bind(tid).bind(format!("q_stale_{}", i)).execute(&db).await.unwrap();
        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 3, ?, 'FLAC', 24, 96000, 120, 1)")
            .bind(tid).bind(format!("t_fallback_{}", i)).execute(&db).await.unwrap();
        all_tracks.push((tid, "fallback_isrc"));
    }

    // 3. 50 Spotify unmapped tracks (NoDownloadProvider)
    for i in 1..=50 {
        let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES (?) RETURNING id")
            .bind(format!("Spotify Only Song {}", i))
            .fetch_one(&db).await.unwrap();
        sqlx::query("INSERT INTO library_entries (account_id, track_id) VALUES (1, ?)")
            .bind(tid).execute(&db).await.unwrap();
        all_tracks.push((tid, "spotify_only"));
    }

    // 4. 40 Already downloaded tracks (AlreadyDownloaded)
    for i in 1..=40 {
        let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES (?) RETURNING id")
            .bind(format!("Downloaded Song {}", i))
            .fetch_one(&db).await.unwrap();
        sqlx::query("INSERT INTO downloads (track_id, file_path) VALUES (?, ?)")
            .bind(tid).bind(format!("C:/Music/dl_{}.flac", i)).execute(&db).await.unwrap();
        all_tracks.push((tid, "already_downloaded"));
    }

    // 5. 30 Already queued tracks (AlreadyQueued)
    for i in 1..=30 {
        let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES (?) RETURNING id")
            .bind(format!("Queued Song {}", i))
            .fetch_one(&db).await.unwrap();
        sqlx::query("INSERT INTO download_queue (track_id, status) VALUES (?, 'queued')")
            .bind(tid).execute(&db).await.unwrap();
        all_tracks.push((tid, "already_queued"));
    }

    // 6. 20 Ambiguous tracks (AmbiguousSource)
    for i in 1..=20 {
        let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES (?, ?) RETURNING id")
            .bind(format!("Ambiguous Song {}", i))
            .bind(format!("USAMB5000{:02}", i))
            .fetch_one(&db).await.unwrap();
        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, available) VALUES (?, 2, ?, 'FLAC', 24, 1)")
            .bind(tid).bind(format!("q_amb_{}", i)).execute(&db).await.unwrap();
        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, available) VALUES (?, 3, ?, 'FLAC', 16, 1)")
            .bind(tid).bind(format!("t_amb_{}", i)).execute(&db).await.unwrap();
        all_tracks.push((tid, "ambiguous"));
    }

    // 7. 10 Low quality tracks (RejectedQuality)
    for i in 1..=10 {
        let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES (?) RETURNING id")
            .bind(format!("Low Quality Song {}", i))
            .fetch_one(&db).await.unwrap();
        sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 3, ?, 'AAC', 16, 44100, 40, 1)")
            .bind(tid).bind(format!("t_lossy_{}", i)).execute(&db).await.unwrap();
        all_tracks.push((tid, "rejected_quality"));
    }

    assert_eq!(all_tracks.len(), 500, "Total batch must contain exactly 500 tracks");

    // Perform Preflight Evaluation on all 500 tracks
    let start = std::time::Instant::now();
    let mut exact_count = 0;
    let mut fallback_count = 0;
    let mut no_provider_count = 0;
    let mut downloaded_count = 0;
    let mut queued_count = 0;
    let mut ambiguous_count = 0;
    let mut rejected_quality_count = 0;

    let mut eligible_tracks = Vec::new();

    for (tid, category) in &all_tracks {
        let req_service = if *category == "fallback_isrc" { Some("qobuz") } else { None };
        let strict = *category == "rejected_quality";
        let pf = evaluate_track_preflight(&db, *tid, req_service, Some("lossless"), strict, true).await.unwrap();

        match pf.status {
            DownloadPreflightStatus::ReadyExactSource => exact_count += 1,
            DownloadPreflightStatus::ReadyFallbackExactIdentity => fallback_count += 1,
            DownloadPreflightStatus::NoDownloadProvider => no_provider_count += 1,
            DownloadPreflightStatus::AlreadyDownloaded => downloaded_count += 1,
            DownloadPreflightStatus::AlreadyQueued => queued_count += 1,
            DownloadPreflightStatus::AmbiguousSource => ambiguous_count += 1,
            DownloadPreflightStatus::RejectedQuality => rejected_quality_count += 1,
            _ => {}
        }

        if pf.is_eligible {
            eligible_tracks.push(pf);
        }
    }
    let elapsed = start.elapsed();
    println!(
        "=== 500-TRACK PREFLIGHT SUMMARY ===\nTotal Elapsed: {:?}\nExact: {}\nFallback: {}\nNoProvider: {}\nDownloaded: {}\nQueued: {}\nAmbiguous: {}\nRejectedQuality: {}\nEligible: {}",
        elapsed, exact_count, fallback_count, no_provider_count, downloaded_count, queued_count, ambiguous_count, rejected_quality_count, eligible_tracks.len()
    );

    // Verify exact breakdown across all 500 tracks (Dual provider tracks resolve as ReadyExactSource)
    assert_eq!(exact_count, 320, "300 single + 20 dual exact sources = 320 ReadyExactSource");
    assert_eq!(fallback_count, 50, "50 ISRC fallback sources");
    assert_eq!(no_provider_count, 50, "50 Spotify unmapped");
    assert_eq!(downloaded_count, 40, "40 already downloaded");
    assert_eq!(queued_count, 30, "30 already queued");
    assert_eq!(ambiguous_count, 0, "Dual-provider tracks are never excluded as AmbiguousSource");
    assert_eq!(rejected_quality_count, 10, "10 rejected quality");

    assert_eq!(eligible_tracks.len(), 370, "Exactly 370 out of 500 tracks must be eligible (320 exact + 50 fallback)");

    // Enqueue eligible tracks
    let mut enqueued = 0i64;
    for pf in eligible_tracks {
        sqlx::query(
            r#"
            INSERT INTO download_queue (
                track_id, priority, position, status, quality_preference,
                service_id, service_name, service_track_id, target_title,
                allow_fallback
            ) VALUES (?, 50, ?, 'queued', ?, ?, ?, ?, ?, 1)
            "#
        )
        .bind(pf.track_id)
        .bind(enqueued)
        .bind(&pf.resolved_quality)
        .bind(pf.resolved_service_id)
        .bind(&pf.resolved_service_name)
        .bind(&pf.resolved_service_track_id)
        .bind(&pf.title)
        .execute(&db)
        .await
        .unwrap();

        enqueued += 1;
    }

    assert_eq!(enqueued, 370, "All eligible tracks must be enqueued: zero silent exclusions (S176Q)");

    // Verify database queue contents (30 preexisting 'queued' + 370 newly enqueued = 400 queued)
    let total_queued_in_db: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_queue WHERE status = 'queued'")
        .fetch_one(&db).await.unwrap();
    assert_eq!(total_queued_in_db, 400);
}


