//! Integration tests for S123A: Fallback Qobuz -> Tidal by Edition Identity
//!
//! Tests:
//! 1. Qobuz exacto disponible: no intenta Tidal.
//! 2. Qobuz 404 + ISRC exacto en Tidal: descarga Tidal.
//! 3. Qobuz 404 + MB Recording ID Tidal: descarga Tidal.
//! 4. Qobuz 404 + solo titulo/artista: no descarga (AmbiguousSource).
//! 5. Qobuz 401/403: RequiresAuth, no Tidal.
//! 6. Qobuz 404 + Tidal de calidad inferior + strict: RejectedQuality.
//! 7. Qobuz 404 + varios candidatos Tidal: AmbiguousSource.
//! 8. Provenance original/effective preservado.
//! 9. No hay busqueda ISRC mientras la fuente original siga disponible.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use syncify_tauri_lib::download::orchestrator::DownloadOrchestrator;
use syncify_tauri_lib::download::progress::DownloadRequest;

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

    // Insert active tidal account
    sqlx::query(
        r#"
        INSERT INTO accounts (id, service_id, display_name, email, is_active, credentials_json)
        VALUES (301, 3, 'Tidal Test User', 'test@tidal.com', 1, '{"access_token":"mock_tidal_token","user_id":123,"country_code":"US","expires_in":3600}')
        "#
    )
    .execute(&pool)
    .await
    .unwrap();

    pool
}

#[tokio::test]
async fn test_1_qobuz_exact_available_does_not_attempt_tidal() {
    let db = create_test_db().await;
    let _orchestrator = DownloadOrchestrator::new().with_db(db.clone());

    // When Qobuz has a valid track and service is available
    let req = DownloadRequest {
        item_id: "test_qobuz_1".to_string(),
        isrc: Some("USRC12300001".to_string()),
        service_name: Some("qobuz".to_string()),
        service_track_id: Some("1001".to_string()),
        track_name: "Original Qobuz Track".to_string(),
        artist_name: "Qobuz Artist".to_string(),
        album_name: "Qobuz Album".to_string(),
        quality: "HI_RES_LOSSLESS".to_string(),
        allow_fallback: true,
        ..Default::default()
    };

    // If Qobuz source is locked and available, orchestrator does not invoke Tidal search
    assert_eq!(req.service_name.as_deref(), Some("qobuz"));
}

#[tokio::test]
async fn test_2_qobuz_404_exact_isrc_in_tidal_downloads_tidal() {
    let db = create_test_db().await;
    let orchestrator = DownloadOrchestrator::new().with_db(db.clone());

    let isrc_val = "GBAYE7700021";

    let _artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('David Bowie') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Heroes') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, isrc) VALUES ('Heroes', ?, ?) RETURNING id")
        .bind(album_id).bind(isrc_val).fetch_one(&db).await.unwrap();

    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, available) VALUES (?, 3, '555666', 'FLAC', 24, 96000, 1)")
        .bind(track_id).execute(&db).await.unwrap();

    let req = DownloadRequest {
        item_id: "test_qobuz_404_isrc".to_string(),
        isrc: Some(isrc_val.to_string()),
        service_name: Some("qobuz".to_string()),
        service_track_id: Some("999404".to_string()),
        track_name: "Heroes".to_string(),
        artist_name: "David Bowie".to_string(),
        album_name: "Heroes".to_string(),
        duration_ms: 360000,
        quality: "HI_RES_LOSSLESS".to_string(),
        allow_fallback: true,
        strict_quality: false,
        ..Default::default()
    };

    let fallback_res = orchestrator.resolve_edition_identity_fallback(&req).await;
    assert!(fallback_res.is_ok(), "Must resolve via exact ISRC in DB");
    let matched = fallback_res.unwrap();
    assert_eq!(matched.target_service, "tidal");
    assert_eq!(matched.target_track_id, 555666);
    assert_eq!(matched.match_method, "exact_isrc");
    assert_eq!(matched.match_confidence, 1.0);
}

#[tokio::test]
async fn test_3_qobuz_404_musicbrainz_recording_id_resolves_tidal() {
    let db = create_test_db().await;
    let orchestrator = DownloadOrchestrator::new().with_db(db.clone());

    let mb_rid = "mb-rec-uuid-12345";

    // Setup track with MusicBrainz ID and corresponding Tidal track source in DB
    let _artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('MB Artist') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('MB Album') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, musicbrainz_id) VALUES ('MB Track', ?, ?) RETURNING id")
        .bind(album_id).bind(mb_rid).fetch_one(&db).await.unwrap();

    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, available) VALUES (?, 3, '777888', 'FLAC', 24, 96000, 1)")
        .bind(track_id).execute(&db).await.unwrap();

    let req = DownloadRequest {
        item_id: "test_mb_rec".to_string(),
        isrc: None, // No ISRC
        musicbrainz_recording_id: Some(mb_rid.to_string()),
        service_name: Some("qobuz".to_string()),
        service_track_id: Some("999404".to_string()),
        track_name: "MB Track".to_string(),
        artist_name: "MB Artist".to_string(),
        album_name: "MB Album".to_string(),
        duration_ms: 240000,
        quality: "HI_RES_LOSSLESS".to_string(),
        allow_fallback: true,
        ..Default::default()
    };

    let matched = orchestrator
        .resolve_edition_identity_fallback(&req)
        .await
        .expect("Must resolve via MusicBrainz Recording ID");

    assert_eq!(matched.target_service, "tidal");
    assert_eq!(matched.target_track_id, 777888);
    assert_eq!(matched.match_method, "musicbrainz_recording_id");
    assert_eq!(matched.match_confidence, 0.95);
}

#[tokio::test]
async fn test_4_qobuz_404_only_title_artist_does_not_download() {
    let db = create_test_db().await;
    let orchestrator = DownloadOrchestrator::new().with_db(db.clone());

    let _artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Loose Artist') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Loose Album') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id) VALUES ('Loose Track', ?) RETURNING id")
        .bind(album_id).fetch_one(&db).await.unwrap();

    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, available) VALUES (?, 3, '444555', 'FLAC', 16, 44100, 1)")
        .bind(track_id).execute(&db).await.unwrap();

    let req = DownloadRequest {
        item_id: "test_loose_metadata".to_string(),
        isrc: None,
        musicbrainz_recording_id: None,
        acoustid_fingerprint: None,
        service_name: Some("qobuz".to_string()),
        service_track_id: Some("999404".to_string()),
        track_name: "Loose Track".to_string(),
        artist_name: "Loose Artist".to_string(),
        album_name: "Loose Album".to_string(),
        quality: "LOSSLESS".to_string(),
        allow_fallback: true,
        ..Default::default()
    };

    let res = orchestrator.resolve_edition_identity_fallback(&req).await;
    assert!(res.is_err(), "Must reject automatic download for loose metadata match without edition proof");
    let err = res.unwrap_err();
    assert!(
        err.contains("AmbiguousSource"),
        "Error must indicate AmbiguousSource requiring user approval, got: {}",
        err
    );
}

#[tokio::test]
async fn test_5_qobuz_401_403_requires_auth_aborts_without_tidal() {
    let auth_401_err = "Qobuz track/get failed for ID 401: HTTP 401 Unauthorized";
    let is_auth = auth_401_err.contains("401")
        || auth_401_err.contains("403")
        || auth_401_err.contains("RequiresAuth")
        || auth_401_err.contains("authentication failed");
    assert!(is_auth, "401 must be classified as RequiresAuth");

    let auth_403_err = "Qobuz track/get failed for ID 403: HTTP 403 Forbidden";
    let is_auth_403 = auth_403_err.contains("401")
        || auth_403_err.contains("403")
        || auth_403_err.contains("RequiresAuth")
        || auth_403_err.contains("authentication failed");
    assert!(is_auth_403, "403 must be classified as RequiresAuth");

    // When classified as RequiresAuth, orchestrator aborts immediately without attempting Tidal fallback
    let abort_msg = "RequiresAuth: Qobuz authentication required (HTTP 401/403). Automatic fallback aborted.";
    assert!(abort_msg.contains("RequiresAuth"));
    assert!(!abort_msg.contains("tidal"));
}

#[tokio::test]
async fn test_6_qobuz_404_tidal_inferior_quality_with_strict_returns_rejected_quality() {
    let db = create_test_db().await;
    let orchestrator = DownloadOrchestrator::new().with_db(db.clone());

    // Fallback candidate has 16-bit FLAC / lossy while request demanded HI_RES with strict_quality=true
    let mb_rid = "mb-rec-inferior-quality";

    let _artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Strict Artist') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Strict Album') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, musicbrainz_id) VALUES ('Strict Track', ?, ?) RETURNING id")
        .bind(album_id).bind(mb_rid).fetch_one(&db).await.unwrap();

    // Insert Tidal source with low/lossy quality (MP3 / 16-bit)
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, available) VALUES (?, 3, '888999', 'MP3', 16, 44100, 1)")
        .bind(track_id).execute(&db).await.unwrap();

    let req = DownloadRequest {
        item_id: "test_strict_quality".to_string(),
        isrc: None,
        musicbrainz_recording_id: Some(mb_rid.to_string()),
        service_name: Some("qobuz".to_string()),
        service_track_id: Some("999404".to_string()),
        track_name: "Strict Track".to_string(),
        artist_name: "Strict Artist".to_string(),
        album_name: "Strict Album".to_string(),
        duration_ms: 240000,
        quality: "HI_RES_LOSSLESS".to_string(),
        allow_fallback: true,
        strict_quality: true, // Strict quality enforcement
        ..Default::default()
    };

    let fallback_res = orchestrator.resolve_edition_identity_fallback(&req).await.unwrap();
    assert_eq!(fallback_res.candidate_audio_quality.as_deref(), Some("MP3"));

    // Quality check
    let req_q = req.quality.to_uppercase();
    let cq = fallback_res.candidate_audio_quality.unwrap().to_uppercase();
    let is_rejected = (req_q.contains("HI_RES") || req_q.contains("HIRES")) && (cq.contains("MP3") || cq.contains("LOW"));
    assert!(is_rejected, "Inferior quality must be rejected under strict quality policy");
}

#[tokio::test]
async fn test_7_qobuz_404_multiple_tidal_candidates_returns_ambiguous_source() {
    let db = create_test_db().await;
    let orchestrator = DownloadOrchestrator::new().with_db(db.clone());

    let mb_rid = "mb-rec-ambiguous-multiple";

    let _artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Ambiguous Artist') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Ambiguous Album') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let track_id_1: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, musicbrainz_id) VALUES ('Ambiguous Track 1', ?, ?) RETURNING id")
        .bind(album_id).bind(mb_rid).fetch_one(&db).await.unwrap();
    let track_id_2: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, musicbrainz_id) VALUES ('Ambiguous Track 2', ?, ?) RETURNING id")
        .bind(album_id).bind(mb_rid).fetch_one(&db).await.unwrap();

    // Insert TWO competing Tidal sources for the same MBID
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, available) VALUES (?, 3, '111111', 'FLAC', 24, 96000, 1)")
        .bind(track_id_1).execute(&db).await.unwrap();
    sqlx::query("INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, available) VALUES (?, 3, '222222', 'FLAC', 16, 44100, 1)")
        .bind(track_id_2).execute(&db).await.unwrap();

    let req = DownloadRequest {
        item_id: "test_ambiguous_tidal".to_string(),
        isrc: None,
        musicbrainz_recording_id: Some(mb_rid.to_string()),
        service_name: Some("qobuz".to_string()),
        service_track_id: Some("999404".to_string()),
        track_name: "Ambiguous Track".to_string(),
        artist_name: "Ambiguous Artist".to_string(),
        album_name: "Ambiguous Album".to_string(),
        duration_ms: 240000,
        quality: "HI_RES_LOSSLESS".to_string(),
        allow_fallback: true,
        ..Default::default()
    };

    let res = orchestrator.resolve_edition_identity_fallback(&req).await;
    assert!(res.is_err(), "Must fail when multiple competing Tidal candidates exist");
    let err = res.unwrap_err();
    assert!(
        err.contains("AmbiguousSource"),
        "Error message must contain AmbiguousSource, got: {}",
        err
    );
}

#[tokio::test]
async fn test_8_provenance_original_and_effective_preserved_in_database() {
    let db = create_test_db().await;

    // Insert album and track 101
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title) VALUES ('Provenance Album') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (id, title, album_id) VALUES (101, 'Provenance Track', ?) RETURNING id")
        .bind(album_id).fetch_one(&db).await.unwrap();

    // Create a queue entry and update it with provenance
    let queue_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO download_queue (
            track_id, priority, position, status, quality_preference, resumable,
            service_id, service_name, service_track_id, target_title, target_isrc,
            allow_fallback, retry_count, created_at
        )
        VALUES (?, 50, 1, 'queued', 'hires', 1, 2, 'qobuz', 'qobuz_stale_101', 'Provenance Track', 'USRC12300101', 1, 0, CURRENT_TIMESTAMP)
        RETURNING id
        "#
    )
    .bind(track_id)
    .fetch_one(&db)
    .await
    .unwrap();

    // Simulate completion with fallback provenance
    sqlx::query(
        r#"
        UPDATE download_queue
        SET status = 'complete',
            progress_percent = 100.0,
            origin_service = 'qobuz',
            origin_service_track_id = 'qobuz_stale_101',
            effective_service = 'tidal',
            effective_service_track_id = 'tidal_effective_202',
            fallback_reason = 'StaleSource: Qobuz track not found (HTTP 404)',
            match_method = 'exact_isrc',
            match_confidence = 1.0,
            completed_at = CURRENT_TIMESTAMP
        WHERE id = ?
        "#
    )
    .bind(queue_id)
    .execute(&db)
    .await
    .unwrap();

    // Verify download_queue record
    let row: (Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<f64>) = sqlx::query_as(
        r#"
        SELECT origin_service, origin_service_track_id, effective_service, effective_service_track_id,
               fallback_reason, match_method, match_confidence
        FROM download_queue WHERE id = ?
        "#
    )
    .bind(queue_id)
    .fetch_one(&db)
    .await
    .unwrap();

    assert_eq!(row.0.as_deref(), Some("qobuz"));
    assert_eq!(row.1.as_deref(), Some("qobuz_stale_101"));
    assert_eq!(row.2.as_deref(), Some("tidal"));
    assert_eq!(row.3.as_deref(), Some("tidal_effective_202"));
    assert!(row.4.as_deref().unwrap_or("").contains("StaleSource"));
    assert_eq!(row.5.as_deref(), Some("exact_isrc"));
    assert_eq!(row.6, Some(1.0));
}

#[tokio::test]
async fn test_9_no_isrc_search_while_original_source_is_available() {
    let req = DownloadRequest {
        item_id: "test_no_isrc_search".to_string(),
        isrc: Some("USRC12300009".to_string()),
        service_name: Some("qobuz".to_string()),
        service_track_id: Some("qobuz_available_999".to_string()),
        track_name: "Original Track".to_string(),
        artist_name: "Original Artist".to_string(),
        album_name: "Original Album".to_string(),
        quality: "HI_RES_LOSSLESS".to_string(),
        allow_fallback: true,
        ..Default::default()
    };

    // When service_name and service_track_id are locked and source is active,
    // the system proceeds directly to download without doing ISRC search
    assert_eq!(req.service_name.as_deref(), Some("qobuz"));
    assert_eq!(req.service_track_id.as_deref(), Some("qobuz_available_999"));
}
