//! Regression tests for audit findings BD-1, BD-2 and BD-3 (SQL against non-existent columns).
//!
//! Before the fix, three queries referenced columns that no migration creates
//! (`downloads.service`, `downloads.status`, `accounts.access_token`), so they failed on
//! EVERY run and the error was swallowed with `unwrap_or(None)`:
//! - BD-1 `perform_force_redownload_tracks` always re-queued without the previous service
//!   identity (queue.rs:2483-2493).
//! - BD-2 incremental enrichment could never locate the downloaded file, so acoustic
//!   analysis (bpm/musical_key/energy) never ran (incremental_enrichment.rs:595-600).
//! - BD-3 `perform_check_track_availability` never validated the account token and marked
//!   every existing account "available" (library.rs:3451-3456).
//!
//! Each test below fails against the broken queries and passes against the real schema.

use std::path::PathBuf;
use std::sync::Arc;

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use tempfile::TempDir;

use syncify_tauri_lib::commands::{
    perform_check_track_availability, perform_force_redownload_tracks,
};
use syncify_tauri_lib::crypto;
use syncify_tauri_lib::enrichment_worker::EnrichmentWorkerState;
use syncify_tauri_lib::services::incremental_enrichment::{
    EnrichmentMode, IncrementalEnrichmentService,
};
use syncify_tauri_lib::worker::DownloadWorkerState;
use syncify_tauri_lib::AppState;

async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}

fn make_app_state(db: SqlitePool) -> (AppState, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create tempdir");
    let state = AppState {
        db,
        worker_state: DownloadWorkerState::new(2),
        enrichment_state: EnrichmentWorkerState::new(),
        concurrency_manager: Arc::new(syncify_tauri_lib::services::ConcurrencyManager::new()),
    };
    (state, temp_dir)
}

/// Creates an artist/album/track triple and returns the track id.
async fn seed_track(db: &SqlitePool, title: &str, isrc: Option<&str>) -> i64 {
    let artist_id: i64 = sqlx::query_scalar(
        "INSERT INTO artists (name) VALUES ('BD Test Artist ' || ?) RETURNING id",
    )
    .bind(title)
    .fetch_one(db)
    .await
    .unwrap();
    let album_id: i64 =
        sqlx::query_scalar("INSERT INTO albums (title) VALUES ('BD Test Album') RETURNING id")
            .fetch_one(db)
            .await
            .unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(album_id)
        .bind(artist_id)
        .execute(db)
        .await
        .unwrap();
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, isrc) VALUES (?, ?, ?) RETURNING id",
    )
    .bind(title)
    .bind(album_id)
    .bind(isrc)
    .fetch_one(db)
    .await
    .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(track_id)
        .bind(artist_id)
        .execute(db)
        .await
        .unwrap();
    track_id
}

// ============================================================================
// BD-1: force re-download must preserve the previous service identity
// ============================================================================

#[tokio::test]
async fn test_bd1_force_redownload_preserves_downloads_ledger_identity() {
    let db = setup_test_db().await;
    let track_id = seed_track(&db, "BD1 Ledger Song", Some("USBD10000001")).await;

    // Candidate sources: qobuz would win priority-based resolution...
    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id, format, quality_score, available) VALUES (?, 2, 'qobuz-candidate', 'FLAC', 95, 1), (?, 3, 'tidal-candidate', 'FLAC', 60, 1)",
    )
    .bind(track_id)
    .bind(track_id)
    .execute(&db)
    .await
    .unwrap();

    // ...but the downloads ledger records that tidal actually delivered the file.
    sqlx::query(
        "INSERT INTO downloads (track_id, source_service_id, file_path, effective_service, effective_service_track_id) VALUES (?, 3, '/music/ledger.flac', 'tidal', 'tidal-ledger-id')",
    )
    .bind(track_id)
    .execute(&db)
    .await
    .unwrap();

    let (state, _temp) = make_app_state(db.clone());
    let requeued = perform_force_redownload_tracks(&state, vec![track_id], None, None)
        .await
        .expect("force redownload must succeed");
    assert_eq!(requeued, 1);

    let (service_name, service_track_id): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT service_name, service_track_id FROM download_queue WHERE track_id = ?",
    )
    .bind(track_id)
    .fetch_one(&db)
    .await
    .unwrap();

    assert_eq!(
        service_name.as_deref(),
        Some("tidal"),
        "BD-1: the ledger's effective_service identity must be preserved for the re-download"
    );
    assert_eq!(
        service_track_id.as_deref(),
        Some("tidal-ledger-id"),
        "BD-1: the ledger's effective_service_track_id must be preserved for the re-download"
    );
}

#[tokio::test]
async fn test_bd1_force_redownload_prefers_latest_queue_identity_over_ledger() {
    let db = setup_test_db().await;
    let track_id = seed_track(&db, "BD1 Queue Song", Some("USBD10000002")).await;

    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id, format, quality_score, available) VALUES (?, 2, 'qobuz-candidate', 'FLAC', 95, 1)",
    )
    .bind(track_id)
    .execute(&db)
    .await
    .unwrap();

    // Ledger says tidal; the most recent queue attempt locked qobuz with its own id.
    sqlx::query(
        "INSERT INTO downloads (track_id, source_service_id, file_path, effective_service, effective_service_track_id) VALUES (?, 3, '/music/queue-vs-ledger.flac', 'tidal', 'tidal-ledger-id')",
    )
    .bind(track_id)
    .execute(&db)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO download_queue (track_id, status, service_name, service_track_id) VALUES (?, 'complete', 'qobuz', 'qb-from-queue')",
    )
    .bind(track_id)
    .execute(&db)
    .await
    .unwrap();

    let (state, _temp) = make_app_state(db.clone());
    perform_force_redownload_tracks(&state, vec![track_id], None, None)
        .await
        .expect("force redownload must succeed");

    let (service_name, service_track_id): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT service_name, service_track_id FROM download_queue WHERE track_id = ?",
    )
    .bind(track_id)
    .fetch_one(&db)
    .await
    .unwrap();

    assert_eq!(service_name.as_deref(), Some("qobuz"));
    assert_eq!(
        service_track_id.as_deref(),
        Some("qb-from-queue"),
        "BD-1: the latest queue record's locked identity must win over the ledger"
    );
}

#[tokio::test]
async fn test_bd1_force_redownload_skips_identityless_queue_rows() {
    let db = setup_test_db().await;
    let track_id = seed_track(&db, "BD1 Fallback Song", Some("USBD10000003")).await;

    // A failed retry row without any service identity must not shadow the ledger identity.
    sqlx::query("INSERT INTO download_queue (track_id, status) VALUES (?, 'failed')")
        .bind(track_id)
        .execute(&db)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO downloads (track_id, source_service_id, file_path, origin_service, origin_service_track_id, effective_service, effective_service_track_id) VALUES (?, 2, '/music/fallback.flac', 'qobuz', 'qb-origin-id', NULL, NULL)",
    )
    .bind(track_id)
    .execute(&db)
    .await
    .unwrap();

    let (state, _temp) = make_app_state(db.clone());
    perform_force_redownload_tracks(&state, vec![track_id], None, None)
        .await
        .expect("force redownload must succeed");

    let (service_name, service_track_id): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT service_name, service_track_id FROM download_queue WHERE track_id = ? AND status = 'queued'",
    )
    .bind(track_id)
    .fetch_one(&db)
    .await
    .unwrap();

    assert_eq!(service_name.as_deref(), Some("qobuz"));
    assert_eq!(
        service_track_id.as_deref(),
        Some("qb-origin-id"),
        "BD-1: with no queue identity the origin fallback from the ledger must be used"
    );
}

#[tokio::test]
async fn test_bd1_force_redownload_without_identity_falls_back_to_resolution() {
    let db = setup_test_db().await;
    let track_id = seed_track(&db, "BD1 Resolve Song", Some("USBD10000004")).await;

    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id, format, quality_score, available) VALUES (?, 2, 'qobuz-resolved', 'FLAC', 95, 1)",
    )
    .bind(track_id)
    .execute(&db)
    .await
    .unwrap();

    // Ledger row carrying no identity at all.
    sqlx::query(
        "INSERT INTO downloads (track_id, source_service_id, file_path) VALUES (?, NULL, '/music/no-identity.flac')",
    )
    .bind(track_id)
    .execute(&db)
    .await
    .unwrap();

    let (state, _temp) = make_app_state(db.clone());
    perform_force_redownload_tracks(&state, vec![track_id], None, None)
        .await
        .expect("force redownload must succeed even without a previous identity");

    let (service_name, service_track_id): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT service_name, service_track_id FROM download_queue WHERE track_id = ? AND status = 'queued'",
    )
    .bind(track_id)
    .fetch_one(&db)
    .await
    .unwrap();

    assert_eq!(service_name.as_deref(), Some("qobuz"));
    assert_eq!(
        service_track_id.as_deref(),
        Some("qobuz-resolved"),
        "BD-1: with no previous identity anywhere the normal source resolution must apply"
    );
}

// ============================================================================
// BD-2: incremental enrichment must find the downloaded file in the ledger
// ============================================================================

fn generate_rhythmic_audio_pcm(bpm: f64, sample_rate: u32, duration_sec: f64) -> Vec<f32> {
    let total_samples = (sample_rate as f64 * duration_sec) as usize;
    let mut samples = vec![0.0f32; total_samples];
    let beat_interval_samples = (sample_rate as f64 * 60.0 / bpm) as usize;

    for beat_start in (0..total_samples).step_by(beat_interval_samples) {
        let pulse_len = (sample_rate as usize / 10).min(total_samples - beat_start);
        for i in 0..pulse_len {
            let t = i as f32 / sample_rate as f32;
            let decay = (-35.0 * t).exp();
            let freq = 120.0 - (60.0 * t);
            let sine = (2.0 * std::f32::consts::PI * freq * t).sin();
            samples[beat_start + i] += sine * decay * 0.8;
        }
    }

    samples
}

fn create_flac_from_pcm(path: &PathBuf, samples: &[f32], sample_rate: u32) {
    let temp_wav = path.with_extension("wav");

    let mut wav_bytes = Vec::new();
    let num_samples = samples.len() as u32;
    let byte_rate = sample_rate * 2;
    let block_align = 2u16;

    wav_bytes.extend_from_slice(b"RIFF");
    wav_bytes.extend_from_slice(&(36 + num_samples * 2).to_le_bytes());
    wav_bytes.extend_from_slice(b"WAVEfmt ");
    wav_bytes.extend_from_slice(&16u32.to_le_bytes());
    wav_bytes.extend_from_slice(&1u16.to_le_bytes());
    wav_bytes.extend_from_slice(&1u16.to_le_bytes());
    wav_bytes.extend_from_slice(&sample_rate.to_le_bytes());
    wav_bytes.extend_from_slice(&byte_rate.to_le_bytes());
    wav_bytes.extend_from_slice(&block_align.to_le_bytes());
    wav_bytes.extend_from_slice(&16u16.to_le_bytes());
    wav_bytes.extend_from_slice(b"data");
    wav_bytes.extend_from_slice(&(num_samples * 2).to_le_bytes());

    for &s in samples {
        let i16_sample = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        wav_bytes.extend_from_slice(&i16_sample.to_le_bytes());
    }

    std::fs::write(&temp_wav, &wav_bytes).expect("Write temp WAV");

    let _ = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-i",
            temp_wav.to_str().unwrap(),
            "-c:a",
            "flac",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("Spawn ffmpeg");

    let _ = std::fs::remove_file(&temp_wav);
}

#[tokio::test]
async fn test_bd2_incremental_enrichment_runs_acoustic_analysis_on_downloaded_file() {
    let db = setup_test_db().await;
    let temp_dir = TempDir::new().unwrap();

    // Track with every field present except bpm/musical_key, so the enrichment run requests
    // only acoustic fields and never hits MusicBrainz (offline test).
    let track_id = seed_track(&db, "BD2 Acoustic Song", Some("USBD20000001")).await;
    sqlx::query(
        "UPDATE tracks SET musicbrainz_id = 'mb-bd2-1', release_year = 2020, genre = 'Rock', record_label = 'BD Records', enrichment_status = 'partial' WHERE id = ?",
    )
    .bind(track_id)
    .execute(&db)
    .await
    .unwrap();

    // Real rhythmic audio (120 BPM). The analyzer decodes from offset 10s, hence >= 20s.
    let flac_path = temp_dir.path().join("bd2_rhythmic.flac");
    let samples = generate_rhythmic_audio_pcm(120.0, 22050, 24.0);
    create_flac_from_pcm(&flac_path, &samples, 22050);
    assert!(
        flac_path.exists(),
        "ffmpeg must be available to create the fixture"
    );

    // The ledger row is the ONLY way to locate the file — before the fix the query
    // referenced downloads.status (non-existent) and the file was never found.
    sqlx::query(
        "INSERT INTO downloads (track_id, source_service_id, file_path, effective_service) VALUES (?, 2, ?, 'qobuz')",
    )
    .bind(track_id)
    .bind(flac_path.to_string_lossy().to_string())
    .execute(&db)
    .await
    .unwrap();

    let service = IncrementalEnrichmentService::new();
    let summary = service
        .run_enrichment(&db, EnrichmentMode::Selection, Some(vec![track_id]), |_| {})
        .await
        .expect("enrichment run must succeed");

    assert_eq!(summary.processed_tracks, 1);
    let item = &summary.items[0];
    assert!(
        item.modified_fields.iter().any(|f| f == "bpm"),
        "BD-2: bpm must be derived from the downloaded file; modified_fields = {:?} (status {:?})",
        item.modified_fields,
        item.status
    );

    let (bpm, energy): (Option<f64>, Option<f64>) =
        sqlx::query_as("SELECT bpm, energy FROM tracks WHERE id = ?")
            .bind(track_id)
            .fetch_one(&db)
            .await
            .unwrap();

    let bpm = bpm.expect("BD-2: bpm must be persisted from the acoustic analysis");
    assert!(
        (bpm - 120.0).abs() <= 6.0,
        "Detected bpm {} must be near the 120 BPM fixture",
        bpm
    );
    assert!(energy.is_some(), "BD-2: energy must be persisted too");
}

#[tokio::test]
async fn test_bd2_incremental_enrichment_survives_missing_file_without_false_data() {
    let db = setup_test_db().await;

    let track_id = seed_track(&db, "BD2 Missing File Song", Some("USBD20000002")).await;
    sqlx::query(
        "UPDATE tracks SET musicbrainz_id = 'mb-bd2-2', release_year = 2020, genre = 'Rock', record_label = 'BD Records' WHERE id = ?",
    )
    .bind(track_id)
    .execute(&db)
    .await
    .unwrap();

    // Ledger row whose file is gone: the existence check is the real guard.
    sqlx::query(
        "INSERT INTO downloads (track_id, source_service_id, file_path) VALUES (?, 2, '/definitely/missing/bd2.flac')",
    )
    .bind(track_id)
    .execute(&db)
    .await
    .unwrap();

    let service = IncrementalEnrichmentService::new();
    let summary = service
        .run_enrichment(&db, EnrichmentMode::Selection, Some(vec![track_id]), |_| {})
        .await
        .expect("enrichment run must not fail when the file is missing");

    assert_eq!(summary.processed_tracks, 1);
    assert!(
        !summary.items[0].modified_fields.iter().any(|f| f == "bpm"),
        "No bpm may be invented when the physical file is gone"
    );

    let bpm: Option<f64> = sqlx::query_scalar("SELECT bpm FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(bpm.is_none(), "bpm must stay NULL without a real file");
}

// ============================================================================
// BD-3: availability check must read credentials_json and validate the token
// ============================================================================

async fn seed_qobuz_source(db: &SqlitePool, track_id: i64) {
    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id, format, quality_score, available) VALUES (?, 2, ?, 'FLAC', 95, 1)",
    )
    .bind(track_id)
    .bind(format!("qb-clean-{}", track_id))
    .execute(db)
    .await
    .unwrap();
}

#[tokio::test]
async fn test_bd3_active_account_with_valid_token_is_available() {
    let _ = crypto::init_crypto([42u8; 32]);
    let db = setup_test_db().await;
    let track_id = seed_track(&db, "BD3 Valid Token Song", None).await;
    seed_qobuz_source(&db, track_id).await;

    let encrypted = crypto::encrypt(r#"{"user_auth_token":"qobuz-token-1234567890"}"#).unwrap();
    sqlx::query(
        "INSERT INTO accounts (service_id, credentials_json, credentials_invalid, is_active) VALUES (2, ?, 0, 1)",
    )
    .bind(&encrypted)
    .execute(&db)
    .await
    .unwrap();

    let results = perform_check_track_availability(&db, track_id, None)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].availability_status, "available");
    assert_eq!(results[0].available, 1);
}

#[tokio::test]
async fn test_bd3_active_qobuz_account_without_credentials_requires_auth() {
    let _ = crypto::init_crypto([42u8; 32]);
    let db = setup_test_db().await;
    let track_id = seed_track(&db, "BD3 No Creds Song", None).await;
    seed_qobuz_source(&db, track_id).await;

    // Active account but the encrypted credentials blob is NULL — before the fix the
    // swallowed SQL error made this account report "available".
    sqlx::query(
        "INSERT INTO accounts (service_id, credentials_json, is_active) VALUES (2, NULL, 1)",
    )
    .execute(&db)
    .await
    .unwrap();

    let results = perform_check_track_availability(&db, track_id, None)
        .await
        .unwrap();
    assert_eq!(results[0].availability_status, "requires_auth");
    assert_eq!(results[0].available, 0);
}

#[tokio::test]
async fn test_bd3_invalid_flagged_credentials_require_auth() {
    let _ = crypto::init_crypto([42u8; 32]);
    let db = setup_test_db().await;
    let track_id = seed_track(&db, "BD3 Invalid Flag Song", None).await;
    seed_qobuz_source(&db, track_id).await;

    let encrypted = crypto::encrypt(r#"{"user_auth_token":"expired-token"}"#).unwrap();
    sqlx::query(
        "INSERT INTO accounts (service_id, credentials_json, credentials_invalid, is_active) VALUES (2, ?, 1, 1)",
    )
    .bind(&encrypted)
    .execute(&db)
    .await
    .unwrap();

    let results = perform_check_track_availability(&db, track_id, None)
        .await
        .unwrap();
    assert_eq!(results[0].availability_status, "requires_auth");
    assert_eq!(results[0].available, 0);
    assert!(results[0]
        .availability_reason
        .as_ref()
        .unwrap()
        .to_lowercase()
        .contains("credentials"));
}

#[tokio::test]
async fn test_bd3_credentials_without_usable_token_require_auth() {
    let _ = crypto::init_crypto([42u8; 32]);
    let db = setup_test_db().await;
    let track_id = seed_track(&db, "BD3 No Token Song", None).await;
    seed_qobuz_source(&db, track_id).await;

    // Payload decrypts fine but carries no token key for the provider.
    let encrypted = crypto::encrypt(r#"{"password":"only-a-password"}"#).unwrap();
    sqlx::query("INSERT INTO accounts (service_id, credentials_json, is_active) VALUES (2, ?, 1)")
        .bind(&encrypted)
        .execute(&db)
        .await
        .unwrap();

    let results = perform_check_track_availability(&db, track_id, None)
        .await
        .unwrap();
    assert_eq!(results[0].availability_status, "requires_auth");
    assert_eq!(results[0].available, 0);
}

#[tokio::test]
async fn test_bd3_undecryptable_credentials_require_auth() {
    let _ = crypto::init_crypto([42u8; 32]);
    let db = setup_test_db().await;
    let track_id = seed_track(&db, "BD3 Garbage Song", None).await;
    seed_qobuz_source(&db, track_id).await;

    sqlx::query(
        "INSERT INTO accounts (service_id, credentials_json, is_active) VALUES (2, 'not-an-encrypted-blob', 1)",
    )
    .execute(&db)
    .await
    .unwrap();

    let results = perform_check_track_availability(&db, track_id, None)
        .await
        .unwrap();
    assert_eq!(results[0].availability_status, "requires_auth");
    assert_eq!(results[0].available, 0);
}

#[tokio::test]
async fn test_bd3_oauth_services_validate_access_token_key() {
    let _ = crypto::init_crypto([42u8; 32]);
    let db = setup_test_db().await;

    // Spotify (service_id 1) and Tidal (service_id 3) both authenticate via access_token.
    for (service_id, title) in [(1i64, "BD3 Spotify Song"), (3i64, "BD3 Tidal Song")] {
        let track_id = seed_track(&db, title, None).await;
        sqlx::query(
            "INSERT INTO track_sources (track_id, service_id, service_track_id, format, quality_score, available) VALUES (?, ?, ?, 'FLAC', 95, 1)",
        )
        .bind(track_id)
        .bind(service_id)
        .bind(format!("svc-{}-{}", service_id, track_id))
        .execute(&db)
        .await
        .unwrap();

        let encrypted = crypto::encrypt(r#"{"access_token":"oauth-token-1234567890"}"#).unwrap();
        sqlx::query(
            "INSERT INTO accounts (service_id, credentials_json, is_active) VALUES (?, ?, 1)",
        )
        .bind(service_id)
        .bind(&encrypted)
        .execute(&db)
        .await
        .unwrap();

        let results = perform_check_track_availability(&db, track_id, None)
            .await
            .unwrap();
        assert_eq!(results[0].availability_status, "available");
        assert_eq!(results[0].available, 1);
    }
}

#[tokio::test]
async fn test_bd3_non_token_service_keeps_presence_based_check() {
    let db = setup_test_db().await;
    let track_id = seed_track(&db, "BD3 Deezer Song", None).await;

    sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id, format, quality_score, available) VALUES (?, 4, 'dz-clean-1', 'FLAC', 95, 1)",
    )
    .bind(track_id)
    .execute(&db)
    .await
    .unwrap();

    // Deezer has no credentials stored; only the token-authenticated services (qobuz,
    // tidal, spotify) hard-fail on missing credentials.
    sqlx::query("INSERT INTO accounts (service_id, is_active) VALUES (4, 1)")
        .execute(&db)
        .await
        .unwrap();

    let results = perform_check_track_availability(&db, track_id, None)
        .await
        .unwrap();
    assert_eq!(results[0].availability_status, "available");
    assert_eq!(results[0].available, 1);
}

#[tokio::test]
async fn test_bd3_inactive_account_still_reports_available_legacy_fallback() {
    let db = setup_test_db().await;
    let track_id = seed_track(&db, "BD3 Inactive Song", None).await;
    seed_qobuz_source(&db, track_id).await;

    // Only an inactive account exists. The legacy presence fallback is preserved on
    // purpose; this pins the boundary so future tightening is a conscious decision.
    sqlx::query("INSERT INTO accounts (service_id, is_active) VALUES (2, 0)")
        .execute(&db)
        .await
        .unwrap();

    let results = perform_check_track_availability(&db, track_id, None)
        .await
        .unwrap();
    assert_eq!(results[0].availability_status, "available");
    assert_eq!(results[0].available, 1);
}
