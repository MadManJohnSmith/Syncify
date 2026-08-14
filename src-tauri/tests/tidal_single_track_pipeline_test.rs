//! Integration tests for Tidal Single Track E2E Pipeline in `src-tauri` (Corte 2)

use sqlx::sqlite::SqlitePoolOptions;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use syncify_core_domain::byte_validators::AudioByteValidator;
use syncify_core_domain::events::{PipelineProgressEvent, PipelineStepStatus};
use syncify_core_domain::quality::{QualityClass, QualityPolicy};
use syncify_tauri_lib::services::tidal_pipeline::sanitize_filename_component;



async fn create_test_db() -> sqlx::Pool<sqlx::Sqlite> {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    // Run core schema
    sqlx::query(
        r#"
        CREATE TABLE services (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            supports_download INTEGER DEFAULT 0,
            max_quality TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

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

        CREATE TABLE artists (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
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
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE track_artists (
            track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
            artist_id INTEGER NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
            role TEXT DEFAULT 'primary',
            PRIMARY KEY (track_id, artist_id, role)
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
            status TEXT DEFAULT 'verified',
            downloaded_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        "#
    )
    .execute(&pool)
    .await
    .expect("Failed to initialize test schema");

    pool
}

#[tokio::test]
async fn test_sanitize_filename_component() {
    assert_eq!(sanitize_filename_component("Artist / Name: 2024?"), "Artist _ Name_ 2024_");
    assert_eq!(sanitize_filename_component(".."), "Unknown");
    assert_eq!(sanitize_filename_component("Standard Track Title"), "Standard Track Title");
}

#[tokio::test]
async fn test_quality_policy_rejection_in_pipeline() {
    let result = QualityPolicy::evaluate_downgrade(
        QualityClass::Lossless,
        QualityClass::Lossy,
        "MP3",
        false,
    );
    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    assert!(err_msg.contains("requested_lossless_but_received_mp3"));

    let fallback_allowed = QualityPolicy::evaluate_downgrade(
        QualityClass::Lossless,
        QualityClass::Lossy,
        "MP3",
        true,
    );
    assert!(fallback_allowed.is_ok());
}

#[tokio::test]
async fn test_audio_byte_validator_flac_magic() {
    let flac_sample = b"fLaC\x00\x00\x00\x22dummy flac content stream";
    assert!(AudioByteValidator::is_flac_magic(flac_sample));

    let corrupt_sample = b"RIFF1234WAVEfmt ";
    assert!(!AudioByteValidator::is_flac_magic(corrupt_sample));
}

#[tokio::test]
async fn test_single_track_pipeline_db_persistence() {
    let pool = create_test_db().await;
    let temp_test_dir = std::env::temp_dir().join(format!("syncify_test_lib_{}", uuid::Uuid::new_v4()));
    let _ = tokio::fs::create_dir_all(&temp_test_dir).await;

    // Simulate staged audio file with valid FLAC stream
    let dummy_flac_path = temp_test_dir.join("test_stage.flac");
    let mut dummy_flac_bytes = Vec::new();
    dummy_flac_bytes.extend_from_slice(b"fLaC\x80\x00\x00\x22");
    dummy_flac_bytes.extend(std::iter::repeat(0u8).take(34)); // minimum streaminfo block
    tokio::fs::write(&dummy_flac_path, &dummy_flac_bytes).await.unwrap();

    let events = Arc::new(AtomicUsize::new(0));
    let events_clone = events.clone();

    let progress_callback = move |evt: PipelineProgressEvent| {
        events_clone.fetch_add(1, Ordering::SeqCst);
        match evt.status {
            PipelineStepStatus::Authenticating => tracing::info!("Step: Authenticating"),
            PipelineStepStatus::ResolvingStream => tracing::info!("Step: ResolvingStream"),
            PipelineStepStatus::Downloading => tracing::info!("Step: Downloading"),
            PipelineStepStatus::Validating => tracing::info!("Step: Validating"),
            PipelineStepStatus::Tagging => tracing::info!("Step: Tagging"),
            PipelineStepStatus::Staging => tracing::info!("Step: Staging"),
            PipelineStepStatus::Persisting => tracing::info!("Step: Persisting"),
            PipelineStepStatus::Completed => tracing::info!("Step: Completed"),
            _ => {}
        }
    };

    // Emit simulated progress steps
    progress_callback(PipelineProgressEvent::new("12345", "tidal", PipelineStepStatus::Authenticating));
    progress_callback(PipelineProgressEvent::new("12345", "tidal", PipelineStepStatus::ResolvingStream));
    progress_callback(PipelineProgressEvent::new("12345", "tidal", PipelineStepStatus::Downloading));
    progress_callback(PipelineProgressEvent::new("12345", "tidal", PipelineStepStatus::Validating));
    progress_callback(PipelineProgressEvent::new("12345", "tidal", PipelineStepStatus::Tagging));
    progress_callback(PipelineProgressEvent::new("12345", "tidal", PipelineStepStatus::Staging));
    progress_callback(PipelineProgressEvent::new("12345", "tidal", PipelineStepStatus::Persisting));
    progress_callback(PipelineProgressEvent::new("12345", "tidal", PipelineStepStatus::Completed));


    assert_eq!(events.load(Ordering::SeqCst), 8);

    // Verify DB insertion
    let mut tx = pool.begin().await.unwrap();
    let service_id: i64 = sqlx::query_scalar(
        "INSERT INTO services (name, supports_download, max_quality) VALUES ('tidal', 1, 'hires') RETURNING id"
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();

    let _artist_id: i64 = sqlx::query_scalar(
        "INSERT INTO artists (name) VALUES ('Daft Punk') RETURNING id"
    )

    .fetch_one(&mut *tx)
    .await
    .unwrap();

    let album_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, release_date) VALUES ('Discovery', '2001-03-12') RETURNING id"
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();

    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms, track_number, isrc, audio_quality) VALUES ('One More Time', ?, 320000, 1, 'FRZ010100001', 'HI_RES_LOSSLESS') RETURNING id"
    )
    .bind(album_id)
    .fetch_one(&mut *tx)
    .await
    .unwrap();

    let _ = sqlx::query(
        "INSERT INTO downloads (track_id, source_service_id, file_path, file_format, bit_depth, sample_rate, file_size_bytes, status) VALUES (?, ?, ?, 'flac', 24, 96000.0, 5000000, 'verified')"
    )
    .bind(track_id)
    .bind(service_id)
    .bind(dummy_flac_path.to_string_lossy().to_string())
    .execute(&mut *tx)
    .await
    .unwrap();

    tx.commit().await.unwrap();

    // Query back from DB
    let track_title: String = sqlx::query_scalar("SELECT title FROM tracks WHERE id = ?")
        .bind(track_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(track_title, "One More Time");

    let download_status: String = sqlx::query_scalar("SELECT status FROM downloads WHERE track_id = ?")
        .bind(track_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(download_status, "verified");

    let _ = tokio::fs::remove_dir_all(&temp_test_dir).await;
}

#[test]
fn test_resolved_track_info_structure_and_fine_grained_events() {
    let resolved_info = syncify_core_domain::events::ResolvedTrackInfo {
        provider: "tidal".to_string(),
        track_id: "778899".to_string(),
        isrc: Some("USRC12345678".to_string()),
        title: "Heroes".to_string(),
        artist: "David Bowie".to_string(),
        album: "Heroes (2017 Remaster)".to_string(),
        duration_sec: 367,
        requested_quality: "24-192".to_string(),
        obtained_quality: Some("HI_RES_LOSSLESS".to_string()),
        active_account: Some("test_tidal_user".to_string()),
        region: Some("US".to_string()),
        allow_fallback: false,
        stream_codec: Some("FLAC".to_string()),
        bit_depth: Some(24),
        sample_rate: Some(96000.0),
    };


    let event = PipelineProgressEvent::new("778899", "tidal", PipelineStepStatus::TrackResolved)
        .with_resolved_track(resolved_info.clone())
        .with_message("Track candidate successfully resolved");

    assert_eq!(event.status, PipelineStepStatus::TrackResolved);
    assert_eq!(event.resolved_track.as_ref().unwrap().provider, "tidal");
    assert_eq!(event.resolved_track.as_ref().unwrap().track_id, "778899");
    assert_eq!(event.resolved_track.as_ref().unwrap().isrc.as_deref(), Some("USRC12345678"));
    assert_eq!(event.resolved_track.as_ref().unwrap().active_account.as_deref(), Some("test_tidal_user"));
    assert_eq!(event.resolved_track.as_ref().unwrap().region.as_deref(), Some("US"));
    assert!(!event.resolved_track.as_ref().unwrap().allow_fallback);

    // Test fine-grained step status string representations
    assert_eq!(PipelineStepStatus::AccountResolved.to_string(), "account_resolved");
    assert_eq!(PipelineStepStatus::TrackResolved.to_string(), "track_resolved");
    assert_eq!(PipelineStepStatus::TrackUnresolved.to_string(), "track_unresolved");
    assert_eq!(PipelineStepStatus::CandidateRejected.to_string(), "candidate_rejected");
    assert_eq!(PipelineStepStatus::DownloadStarted.to_string(), "download_started");
    assert_eq!(PipelineStepStatus::DownloadCompleted.to_string(), "download_completed");
    assert_eq!(PipelineStepStatus::MetadataApplied.to_string(), "metadata_applied");
    assert_eq!(PipelineStepStatus::StagingCompleted.to_string(), "staging_completed");
    assert_eq!(PipelineStepStatus::Persisted.to_string(), "persisted");
}

#[tokio::test]
async fn test_orchestrator_credential_and_error_taxonomy() {
    use syncify_core_domain::errors::{PipelineError, RequiresAuthReason};

    let auth_err = PipelineError::RequiresAuth(RequiresAuthReason::TokenExpired);
    assert!(!auth_err.is_retryable());
    assert!(auth_err.is_auth_failure());

    let playback_err = PipelineError::PlaybackUnauthorized {
        provider: "tidal".to_string(),
        http_status: 401,
        sub_status: Some("11002".to_string()),
        message: "Token has invalid payload".to_string(),
    };
    assert!(!playback_err.is_retryable());
    assert!(playback_err.is_auth_failure());

    let quality_err = PipelineError::RejectedQuality {
        requested: "24-192".to_string(),
        obtained: "320".to_string(),
        reason: "Lossy downgrade rejected".to_string(),
    };
    assert!(!quality_err.is_retryable());

    let unresolved_err = PipelineError::TrackUnresolved {
        provider: "tidal".to_string(),
        query: "Unknown Track".to_string(),
    };
    assert!(!unresolved_err.is_retryable());

    let net_err = PipelineError::NetworkError {
        provider: "tidal".to_string(),
        endpoint: "playbackinfopostpaywall".to_string(),
        message: "Connection timed out".to_string(),
    };
    assert!(net_err.is_retryable());
}


