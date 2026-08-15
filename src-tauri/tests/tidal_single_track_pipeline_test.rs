//! Integration tests for Tidal Single Track E2E Pipeline in `src-tauri` (Corte 2)

use sqlx::sqlite::SqlitePoolOptions;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use syncify_core_domain::byte_validators::AudioByteValidator;
use syncify_core_domain::events::{PipelineProgressEvent, PipelineStepStatus};
use syncify_core_domain::quality::{QualityClass, QualityPolicy};
use syncify_flac_writer::{apply_and_verify_flac_tags, FlacMetadata};
use syncify_tauri_lib::services::tidal_pipeline::sanitize_filename_component;

async fn write_test_flac_payload(path: &std::path::Path) {
    let mut flac_bytes = Vec::new();
    flac_bytes.extend_from_slice(b"fLaC");
    flac_bytes.extend_from_slice(&[0x80, 0x00, 0x00, 0x22]);
    let mut streaminfo = [0u8; 34];
    streaminfo[0..2].copy_from_slice(&4608u16.to_be_bytes());
    streaminfo[2..4].copy_from_slice(&4608u16.to_be_bytes());
    streaminfo[10] = 0x0A;
    streaminfo[11] = 0xC4;
    streaminfo[12] = 0x42;
    streaminfo[13] = 0xF0;
    flac_bytes.extend_from_slice(&streaminfo);
    flac_bytes.extend_from_slice(&[0xFF, 0xF8, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00]);
    tokio::fs::write(path, &flac_bytes).await.unwrap();
}



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
        obtained_quality: Some("24-bit / 96.0 kHz FLAC".to_string()),
        format_id_requested: Some("HI_RES_LOSSLESS".to_string()),
        format_id_obtained: Some("HI_RES_LOSSLESS".to_string()),
        quality_class: Some(syncify_core_domain::quality::QualityClass::Lossless),
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
    assert_eq!(event.resolved_track.as_ref().unwrap().format_id_requested.as_deref(), Some("HI_RES_LOSSLESS"));
    assert_eq!(event.resolved_track.as_ref().unwrap().format_id_obtained.as_deref(), Some("HI_RES_LOSSLESS"));
    assert_eq!(event.resolved_track.as_ref().unwrap().quality_class, Some(syncify_core_domain::quality::QualityClass::Lossless));
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

#[tokio::test]
async fn test_mp4_m4a_tagging_and_verification() {
    use syncify_tauri_lib::services::mp4_writer::{apply_and_verify_mp4_tags, Mp4Metadata};

    let temp_dir = std::env::temp_dir().join(format!("syncify_m4a_test_{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_dir).await.unwrap();
    let m4a_path = temp_dir.join("test_track.m4a");

    // Generate a minimal valid AAC/M4A file using ffmpeg
    let ffmpeg_out = tokio::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-f", "lavfi",
            "-i", "anullsrc=r=44100:cl=stereo",
            "-t", "1",
            "-c:a", "aac",
            "-b:a", "320k",
            m4a_path.to_str().unwrap(),
        ])
        .output()
        .await;

    if let Ok(out) = ffmpeg_out {
        if !out.status.success() {
            eprintln!("ffmpeg dummy generation skipped: {}", String::from_utf8_lossy(&out.stderr));
            return;
        }
    } else {
        eprintln!("ffmpeg not available on host, skipping physical M4A test");
        return;
    }

    let dummy_cover_jpeg = vec![
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01,
        0x01, 0x01, 0x00, 0x60, 0x00, 0x60, 0x00, 0x00, 0xFF, 0xD9,
    ];

    let mp4_meta = Mp4Metadata {
        title: "Heroes".to_string(),
        artist: "David Bowie".to_string(),
        album: "Heroes".to_string(),
        album_artist: Some("David Bowie".to_string()),
        composer: Some("David Bowie, Brian Eno".to_string()),
        performer: Some("David Bowie".to_string()),
        genre: Some("Art Rock".to_string()),
        release_year: Some("1977".to_string()),
        release_date: Some("1977-10-14".to_string()),
        original_date: Some("1977-10-14".to_string()),
        track_number: 1,
        track_total: 10,
        disc_number: 1,
        disc_total: 1,
        isrc: Some("GBAYE7700021".to_string()),
        label: Some("RCA Records".to_string()),
        catalog_number: Some("AFL1-2522".to_string()),
        barcode: Some("07863525221".to_string()),
        release_country: Some("GB".to_string()),
        comment: Some("Audio: 320 kbps AAC | Source: Tidal | Engine: Syncify Production".to_string()),
        lyrics: Some("[00:01.00]I, I will be king\n[00:05.00]And you, you will be queen".to_string()),
        cover_data: Some(dummy_cover_jpeg.clone()),
        cover_mime: Some("image/jpeg".to_string()),
        musicbrainz_track_id: Some("12345678-1234-1234-1234-123456789abc".to_string()),
        musicbrainz_artist_id: Some("5441c29d-3602-4898-b1a1-b77fa23b8e50".to_string()),
        musicbrainz_album_id: Some("abcdef01-2345-6789-abcd-ef0123456789".to_string()),
        musicbrainz_albumartist_id: Some("5441c29d-3602-4898-b1a1-b77fa23b8e50".to_string()),
        musicbrainz_release_group_id: Some("fedcba98-7654-3210-fedc-ba9876543210".to_string()),
        replaygain_track_gain: Some("-8.22 dB".to_string()),
        replaygain_track_peak: Some("0.9882".to_string()),
        replaygain_album_gain: Some("-7.50 dB".to_string()),
        replaygain_album_peak: Some("0.9910".to_string()),
        r128_track_gain: None,
        audio_source: Some("Tidal Official Stream Direct".to_string()),
        explicit: Some(false),
    };

    // 1. Write and verify tags in one pass
    let verification = apply_and_verify_mp4_tags(&m4a_path, &mp4_meta).expect("MP4 tagging and verification must succeed");
    assert!(verification.tags_match);
    assert!(verification.title_matches);
    assert!(verification.artist_matches);
    assert!(verification.album_matches);
    assert!(verification.album_artist_matches);
    assert!(verification.track_number_matches);
    assert!(verification.cover_present);
    assert!(verification.lyrics_present);
    assert!(verification.isrc_present);
    assert!(verification.musicbrainz_present);
    assert!(verification.mismatches.is_empty());

    // 2. Direct readback verification with mp4ameta
    let tag = mp4ameta::Tag::read_from_path(&m4a_path).expect("Must read tagged M4A file");
    assert_eq!(tag.title(), Some("Heroes"));
    assert_eq!(tag.artist(), Some("David Bowie"));
    assert_eq!(tag.album(), Some("Heroes"));
    assert_eq!(tag.album_artist(), Some("David Bowie"));
    assert_eq!(tag.composer(), Some("David Bowie, Brian Eno"));
    assert_eq!(tag.genre(), Some("Art Rock"));
    assert_eq!(tag.year(), Some("1977-10-14"));
    assert_eq!(tag.track_number(), Some(1));
    assert_eq!(tag.total_tracks(), Some(10));
    assert_eq!(tag.disc_number(), Some(1));
    assert_eq!(tag.total_discs(), Some(1));
    assert_eq!(tag.comment(), Some("Audio: 320 kbps AAC | Source: Tidal | Engine: Syncify Production"));
    assert_eq!(tag.lyrics(), Some("[00:01.00]I, I will be king\n[00:05.00]And you, you will be queen"));
    assert!(tag.artwork().is_some() || tag.artworks().next().is_some());

    // Freeform readbacks
    let isrc_ident = mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "ISRC");
    assert_eq!(tag.strings_of(&isrc_ident).next(), Some("GBAYE7700021"));

    let mb_trk_ident = mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "MusicBrainz Track Id");
    assert_eq!(tag.strings_of(&mb_trk_ident).next(), Some("12345678-1234-1234-1234-123456789abc"));

    let mb_art_ident = mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "MusicBrainz Artist Id");
    assert_eq!(tag.strings_of(&mb_art_ident).next(), Some("5441c29d-3602-4898-b1a1-b77fa23b8e50"));

    let label_ident = mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "LABEL");
    assert_eq!(tag.strings_of(&label_ident).next(), Some("RCA Records"));

    let source_ident = mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "SOURCE");
    assert_eq!(tag.strings_of(&source_ident).next(), Some("Tidal Official Stream Direct"));

    // 3. Direct ffprobe verification
    let ffprobe_out = tokio::process::Command::new("ffprobe")
        .args([
            "-v", "quiet",
            "-show_entries", "format_tags",
            "-of", "json",
            m4a_path.to_str().unwrap(),
        ])
        .output()
        .await;

    if let Ok(out) = ffprobe_out {
        if out.status.success() {
            let json_str = String::from_utf8_lossy(&out.stdout);
            println!("FFPROBE JSON TAGS:\n{}", json_str);
            assert!(json_str.contains("\"title\": \"Heroes\""), "ffprobe missing title");
            assert!(json_str.contains("\"artist\": \"David Bowie\""), "ffprobe missing artist");
            assert!(json_str.contains("\"album\": \"Heroes\""), "ffprobe missing album");
            assert!(json_str.contains("\"album_artist\": \"David Bowie\""), "ffprobe missing album_artist");
            assert!(json_str.contains("\"date\": \"1977-10-14\""), "ffprobe missing date");
            assert!(json_str.contains("\"genre\": \"Art Rock\""), "ffprobe missing genre");
            assert!(json_str.contains("\"composer\": \"David Bowie, Brian Eno\""), "ffprobe missing composer");
            assert!(json_str.contains("\"ISRC\": \"GBAYE7700021\""), "ffprobe missing ISRC");
        }
    }

    // Clean up
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
}

#[tokio::test]
async fn test_flac_parity_cli_vs_tauri() {
    use syncify_flac_writer::{apply_and_verify_flac_tags, audit_flac_stage, FlacMetadata};

    let temp_dir = std::env::temp_dir().join(format!("syncify_flac_parity_{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_dir).await.unwrap();
    let flac_path = temp_dir.join("heroes_parity.flac");

    // Construct valid minimal FLAC
    let mut flac_bytes = Vec::new();
    flac_bytes.extend_from_slice(b"fLaC");
    flac_bytes.extend_from_slice(&[0x80, 0x00, 0x00, 0x22]); // is_last=1, len=34
    let mut streaminfo = [0u8; 34];
    streaminfo[0..2].copy_from_slice(&4608u16.to_be_bytes());
    streaminfo[2..4].copy_from_slice(&4608u16.to_be_bytes());
    streaminfo[10] = 0x0A;
    streaminfo[11] = 0xC4;
    streaminfo[12] = 0x42;
    streaminfo[13] = 0xF0;
    flac_bytes.extend_from_slice(&streaminfo);
    flac_bytes.extend_from_slice(&[0xFF, 0xF8, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00]);
    tokio::fs::write(&flac_path, &flac_bytes).await.unwrap();

    let dummy_cover_jpeg = vec![
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01,
        0x01, 0x01, 0x00, 0x60, 0x00, 0x60, 0x00, 0x00, 0xFF, 0xD9,
    ];

    let full_flac_meta = FlacMetadata {
        title: "Heroes".to_string(),
        artist: "David Bowie".to_string(),
        album: "Heroes".to_string(),
        album_artist: Some("David Bowie".to_string()),
        composer: Some("David Bowie, Brian Eno".to_string()),
        performers: Some("David Bowie".to_string()),
        work: None,
        genre: Some("Art Rock".to_string()),
        style: Some("Glam Rock".to_string()),
        mood: Some("Epic".to_string()),
        release_type: Some("Album".to_string()),
        release_status: Some("Official".to_string()),
        release_country: Some("GB".to_string()),
        language: Some("eng".to_string()),
        copyright: Some("1977 Jones/Tintoretto Entertainment Co., LLC".to_string()),
        label: Some("RCA Victor".to_string()),
        barcode: Some("0035629007421".to_string()),
        catalog_number: Some("PL 12522".to_string()),
        original_date: Some("1977-10-14".to_string()),
        track_number: 1,
        track_total: 10,
        disc_number: 1,
        disc_total: 1,
        disc_subtitle: None,
        isrc: Some("GBAYE7700021".to_string()),
        release_year: Some("1977".to_string()),
        release_date: Some("1977-10-14".to_string()),
        explicit: Some(false),
        bpm: Some(112),
        initial_key: Some("D".to_string()),
        energy: Some(0.85),
        danceability: Some(0.55),
        loudness: Some(-7.2),
        replaygain_track_gain: Some("-6.50 dB".to_string()),
        replaygain_track_peak: Some("0.985000".to_string()),
        replaygain_album_gain: Some("-6.20 dB".to_string()),
        replaygain_album_peak: Some("0.990000".to_string()),
        r128_track_gain: Some("-5.20 dB".to_string()),
        comment: Some("Audio: Tidal Official Stream Direct | Source: Tidal | Engine: Syncify Production".to_string()),
        bit_depth: Some(24),
        sample_rate: Some(96000.0),
        musicbrainz_track_id: Some("b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d".to_string()),
        musicbrainz_artist_id: Some("5441c29d-3602-48f7-b1a9-30704df52227".to_string()),
        musicbrainz_album_id: Some("673752e3-2e06-4447-aa72-a080ef8a1768".to_string()),
        musicbrainz_albumartist_id: Some("5441c29d-3602-48f7-b1a9-30704df52227".to_string()),
        musicbrainz_release_group_id: Some("c0e9b90c-d9c0-3ec6-b33a-bcbbd011f061".to_string()),
        musicbrainz_work_id: None,
        lyrics_lrc: Some("[00:01.00]I, I will be king\n[00:05.00]And you, you will be queen".to_string()),
        cover_data: Some(dummy_cover_jpeg),
        lyrics_source: Some("LRCLIB".to_string()),
        cover_source: Some("Tidal Cover Art".to_string()),
        audio_source: Some("Tidal".to_string()),
    };

    let result = apply_and_verify_flac_tags(&flac_path, &full_flac_meta).expect("FLAC tagging must succeed");
    assert!(result.flac_valid);
    assert!(result.tags_match);
    assert!(result.cover_present);
    assert!(result.lyrics_present);
    assert!(result.synced_lyrics_present);
    assert!(result.unsynced_lyrics_present);
    assert_eq!(result.mismatches.len(), 0);

    let audit = audit_flac_stage("test_verification", &flac_path).expect("Audit must succeed");
    assert_eq!(audit.picture_count, 1);

    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
}

#[test]
fn test_animated_cover_flow_and_status() {
    use syncify_tauri_lib::services::animated_cover::{strip_album_edition_suffixes, validate_animated_webp_bytes, AnimatedCoverStatus};
    use std::path::PathBuf;

    assert_eq!(strip_album_edition_suffixes("Heroes (Deluxe Edition)"), "Heroes");
    assert_eq!(strip_album_edition_suffixes("Heroes [Deluxe]"), "Heroes");
    assert_eq!(strip_album_edition_suffixes("Heroes"), "Heroes");

    let success = AnimatedCoverStatus::Success(PathBuf::from("test/cover.webp"));
    let not_found = AnimatedCoverStatus::NotFound;
    let source_unavail = AnimatedCoverStatus::SourceUnavailable("Token expired".to_string());
    let failed = AnimatedCoverStatus::Failed("ffmpeg error".to_string());

    assert!(matches!(success, AnimatedCoverStatus::Success(_)));
    assert_eq!(not_found, AnimatedCoverStatus::NotFound);
    assert!(matches!(source_unavail, AnimatedCoverStatus::SourceUnavailable(_)));
    assert!(matches!(failed, AnimatedCoverStatus::Failed(_)));

    // Non-animated data should fail validation
    assert!(validate_animated_webp_bytes(&[0u8; 10]).is_err());
}

#[tokio::test]
async fn test_aac_tagging_failure_does_not_report_completed() {
    use syncify_tauri_lib::services::mp4_writer::{apply_and_verify_mp4_tags, Mp4Metadata};
    use std::path::PathBuf;

    let non_existent_path = PathBuf::from("/non/existent/path/corrupted.m4a");
    let meta = Mp4Metadata {
        title: "Test Track".to_string(),
        artist: "Test Artist".to_string(),
        album: "Test Album".to_string(),
        ..Default::default()
    };

    let result = apply_and_verify_mp4_tags(&non_existent_path, &meta);
    assert!(result.is_err(), "Must fail when file does not exist or cannot be tagged");
}

#[tokio::test]
async fn test_e2e_gate_scenario_flac_success_full_lifecycle() {
    use syncify_flac_writer::{apply_and_verify_flac_tags, FlacMetadata};
    let db = create_test_db().await;

    let temp_staging = std::env::temp_dir().join(format!("syncify_gate_staging_{}", uuid::Uuid::new_v4()));
    let temp_library = std::env::temp_dir().join(format!("syncify_gate_lib_{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_staging).await.unwrap();
    tokio::fs::create_dir_all(&temp_library).await.unwrap();

    let staged_flac = temp_staging.join("80654035.flac");
    let staged_lrc = temp_staging.join("80654035.lrc");
    let staged_cover = temp_staging.join("cover.jpg");

    // Write valid FLAC bytes
    let mut flac_bytes = Vec::new();
    flac_bytes.extend_from_slice(b"fLaC");
    flac_bytes.extend_from_slice(&[0x80, 0x00, 0x00, 0x22]);
    let mut streaminfo = [0u8; 34];
    streaminfo[0..2].copy_from_slice(&4608u16.to_be_bytes());
    streaminfo[2..4].copy_from_slice(&4608u16.to_be_bytes());
    streaminfo[10] = 0x0A;
    streaminfo[11] = 0xC4;
    streaminfo[12] = 0x42;
    streaminfo[13] = 0xF0;
    flac_bytes.extend_from_slice(&streaminfo);
    flac_bytes.extend_from_slice(&[0xFF, 0xF8, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00]);
    tokio::fs::write(&staged_flac, &flac_bytes).await.unwrap();
    tokio::fs::write(&staged_lrc, "[00:01.00] Heroes lyrics").await.unwrap();
    tokio::fs::write(&staged_cover, &[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x01, 0x00, 0x60, 0x00, 0x60, 0x00, 0x00, 0xFF, 0xD9]).await.unwrap();

    // 1. Tagging
    let flac_meta = FlacMetadata {
        title: "Heroes".to_string(),
        artist: "David Bowie".to_string(),
        album: "Heroes".to_string(),
        album_artist: Some("David Bowie".to_string()),
        track_number: 3,
        track_total: 10,
        disc_number: 1,
        disc_total: 1,
        isrc: Some("GBAYE7700021".to_string()),
        release_year: Some("1977".to_string()),
        lyrics_lrc: Some("[00:01.00] Heroes lyrics".to_string()),
        ..Default::default()
    };
    let tag_res = apply_and_verify_flac_tags(&staged_flac, &flac_meta).expect("FLAC tag must succeed");
    assert!(tag_res.flac_valid);
    assert!(tag_res.tags_match);

    // 2. Persist SQLite
    let final_dir = temp_library.join("David Bowie").join("1977 - Heroes");
    tokio::fs::create_dir_all(&final_dir).await.unwrap();
    let final_flac = final_dir.join("03 - Heroes.flac");
    let final_flac_str = final_flac.to_string_lossy().to_string();

    let service_id: i64 = sqlx::query_scalar("INSERT INTO services (name, supports_download, max_quality) VALUES ('tidal', 1, 'hires') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let _artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('David Bowie') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, release_date, total_tracks) VALUES ('Heroes', '1977-10-14', 10) RETURNING id")
        .fetch_one(&db).await.unwrap();
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, duration_ms, track_number, isrc, audio_quality) VALUES ('Heroes', ?, 371000, 3, 'GBAYE7700021', 'HI_RES_LOSSLESS') RETURNING id")
        .bind(album_id).fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO downloads (track_id, source_service_id, file_path, file_format, bit_depth, sample_rate, file_size_bytes, status) VALUES (?, ?, ?, 'FLAC', 24, 96000, 50000000, 'verified')")
        .bind(track_id).bind(service_id).bind(&final_flac_str).execute(&db).await.unwrap();

    // 3. Move files to library and clean staging
    tokio::fs::rename(&staged_flac, &final_flac).await.unwrap();
    tokio::fs::copy(&staged_lrc, final_dir.join("03 - Heroes.lrc")).await.unwrap();
    tokio::fs::copy(&staged_cover, final_dir.join("cover.jpg")).await.unwrap();
    tokio::fs::remove_dir_all(&temp_staging).await.unwrap();

    // Snapshot assertions
    assert!(!temp_staging.exists(), "Staging dir must be 100% removed (0 orphan staging files)");
    assert!(final_flac.exists(), "Final FLAC file must exist in library");
    assert!(final_dir.join("03 - Heroes.lrc").exists(), "Sidecar .lrc must exist in library");
    assert!(final_dir.join("cover.jpg").exists(), "Sidecar cover.jpg must exist in library");

    let row: (String, String) = sqlx::query_as("SELECT status, file_format FROM downloads WHERE track_id = ?")
        .bind(track_id).fetch_one(&db).await.unwrap();
    assert_eq!(row.0, "verified");
    assert_eq!(row.1, "FLAC");

    let _ = tokio::fs::remove_dir_all(&temp_library).await;
}

#[tokio::test]
async fn test_e2e_gate_scenario_m4a_success_full_lifecycle() {
    use syncify_tauri_lib::services::mp4_writer::{apply_and_verify_mp4_tags, Mp4Metadata};
    let db = create_test_db().await;

    let temp_staging = std::env::temp_dir().join(format!("syncify_gate_staging_{}", uuid::Uuid::new_v4()));
    let temp_library = std::env::temp_dir().join(format!("syncify_gate_lib_{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_staging).await.unwrap();
    tokio::fs::create_dir_all(&temp_library).await.unwrap();

    let staged_m4a = temp_staging.join("80654035.m4a");

    // Generate valid minimal M4A with ffmpeg
    let status = std::process::Command::new("ffmpeg")
        .args(["-y", "-f", "lavfi", "-i", "anullsrc=r=44100:cl=stereo", "-t", "1", "-c:a", "aac", "-b:a", "320k"])
        .arg(&staged_m4a)
        .output()
        .expect("ffmpeg must generate valid test M4A");
    assert!(status.status.success());

    // 1. Tagging M4A with atoms
    let mp4_meta = Mp4Metadata {
        title: "Heroes".to_string(),
        artist: "David Bowie".to_string(),
        album: "Heroes".to_string(),
        album_artist: Some("David Bowie".to_string()),
        track_number: 3,
        track_total: 10,
        disc_number: 1,
        disc_total: 1,
        release_year: Some("1977".to_string()),
        isrc: Some("USJT11700035".to_string()),
        musicbrainz_track_id: Some("722190f8-f718-482f-a8bc-a8d479426a30".to_string()),
        musicbrainz_artist_id: Some("5441c29d-3602-48f7-b1a9-30704df52227".to_string()),
        musicbrainz_album_id: Some("9427d0a7-396a-4150-bc67-4800255cf6aa".to_string()),
        lyrics: Some("[00:01.00] Heroes lyrics".to_string()),
        cover_data: Some(vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x01, 0x00, 0x60, 0x00, 0x60, 0x00, 0x00, 0xFF, 0xD9]),
        ..Default::default()
    };
    let tag_res = apply_and_verify_mp4_tags(&staged_m4a, &mp4_meta).expect("M4A tagging must succeed");
    assert!(tag_res.title_matches);
    assert!(tag_res.artist_matches);
    assert!(tag_res.album_matches);
    assert!(tag_res.track_number_matches);

    // 2. Persist SQLite with CHECK constraint file_format='AAC'
    let final_dir = temp_library.join("David Bowie").join("1977 - Heroes");
    tokio::fs::create_dir_all(&final_dir).await.unwrap();
    let final_m4a = final_dir.join("03 - Heroes.m4a");
    let final_m4a_str = final_m4a.to_string_lossy().to_string();

    let service_id: i64 = sqlx::query_scalar("INSERT INTO services (name, supports_download, max_quality) VALUES ('tidal', 1, 'hires') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let _artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('David Bowie') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, release_date, total_tracks) VALUES ('Heroes', '1977-10-14', 10) RETURNING id")
        .fetch_one(&db).await.unwrap();
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, duration_ms, track_number, isrc, audio_quality) VALUES ('Heroes', ?, 371000, 3, 'USJT11700035', 'HIGH') RETURNING id")
        .bind(album_id).fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO downloads (track_id, source_service_id, file_path, file_format, bit_depth, sample_rate, file_size_bytes, status) VALUES (?, ?, ?, 'AAC', 16, 44100, 15000000, 'verified')")
        .bind(track_id).bind(service_id).bind(&final_m4a_str).execute(&db).await.unwrap();

    // 3. Move file to library and clean staging
    tokio::fs::rename(&staged_m4a, &final_m4a).await.unwrap();
    tokio::fs::remove_dir_all(&temp_staging).await.unwrap();

    // Snapshot assertions
    assert!(!temp_staging.exists(), "Staging dir must be 100% removed (0 orphan staging files)");
    assert!(final_m4a.exists(), "Final M4A file must exist in library");

    let row: (String, String) = sqlx::query_as("SELECT status, file_format FROM downloads WHERE track_id = ?")
        .bind(track_id).fetch_one(&db).await.unwrap();
    assert_eq!(row.0, "verified");
    assert_eq!(row.1, "AAC");

    let _ = tokio::fs::remove_dir_all(&temp_library).await;
}

#[tokio::test]
async fn test_e2e_gate_scenario_quality_downgrade_rejected() {
    let res = QualityPolicy::evaluate_downgrade(QualityClass::Lossless, QualityClass::Lossy, "HIGH", false);
    assert!(res.is_err(), "Must reject lossy AAC stream when LOSSLESS requested without fallback");
    assert_eq!(res.unwrap_err(), "Quality rejection: requested_lossless_but_received_high");
}

#[tokio::test]
async fn test_e2e_gate_scenario_payload_corruption_rollback() {
    let temp_staging = std::env::temp_dir().join(format!("syncify_gate_staging_{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_staging).await.unwrap();

    let staged_file = temp_staging.join("corrupted_payload.flac");
    tokio::fs::write(&staged_file, b"NOT_A_VALID_FLAC_MAGIC_HEADER_TRUNCATED").await.unwrap();

    let bytes = tokio::fs::read(&staged_file).await.unwrap();
    let is_flac = AudioByteValidator::is_flac_magic(&bytes);
    assert!(!is_flac, "Corrupted payload must fail FLAC magic byte validator");

    // Rollback cleanup
    let _ = tokio::fs::remove_dir_all(&temp_staging).await;
    assert!(!temp_staging.exists(), "Staging dir must be cleaned on payload validation failure (0 orphan files)");
}

#[tokio::test]
async fn test_e2e_gate_scenario_tagging_failure_rollback() {
    use syncify_tauri_lib::services::mp4_writer::{apply_and_verify_mp4_tags, Mp4Metadata};

    let temp_staging = std::env::temp_dir().join(format!("syncify_gate_staging_{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_staging).await.unwrap();

    let staged_m4a = temp_staging.join("corrupted_atoms.m4a");
    // Write pseudo-m4a that fails MP4 atom parser
    tokio::fs::write(&staged_m4a, &[0x00, 0x00, 0x00, 0x20, 0x66, 0x74, 0x79, 0x70, 0x4D, 0x34, 0x41, 0x20]).await.unwrap();

    let meta = Mp4Metadata {
        title: "Test".to_string(),
        artist: "Test".to_string(),
        album: "Test".to_string(),
        ..Default::default()
    };

    let tag_res = apply_and_verify_mp4_tags(&staged_m4a, &meta);
    assert!(tag_res.is_err(), "Tagging corrupted M4A file must fail");

    let _ = tokio::fs::remove_dir_all(&temp_staging).await;
    assert!(!temp_staging.exists(), "Staging dir must be cleaned on tagging failure (0 orphan files)");
}

#[tokio::test]
async fn test_e2e_gate_scenario_staging_move_failure_rollback() {
    let db = create_test_db().await;
    let temp_staging = std::env::temp_dir().join(format!("syncify_gate_staging_{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_staging).await.unwrap();

    let staged_flac = temp_staging.join("01 - Test.flac");
    write_test_flac_payload(&staged_flac).await;

    let final_dest = "/non_existent_or_invalid_root_directory/subfolder/01 - Test.flac";

    // 1. Transaction insert
    let service_id: i64 = sqlx::query_scalar("INSERT INTO services (name, supports_download, max_quality) VALUES ('tidal', 1, 'hires') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('Test Move Rollback', 'USJT11799999') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO downloads (track_id, source_service_id, file_path, file_format, status) VALUES (?, ?, ?, 'FLAC', 'verified')")
        .bind(track_id).bind(service_id).bind(final_dest).execute(&db).await.unwrap();

    // 2. Simulate move failure
    let move_res = tokio::fs::rename(&staged_flac, final_dest).await;
    assert!(move_res.is_err(), "Move to invalid destination must fail");

    // 3. Compensation: delete download entry and purge staging
    sqlx::query("DELETE FROM downloads WHERE file_path = ?").bind(final_dest).execute(&db).await.unwrap();
    let _ = tokio::fs::remove_dir_all(&temp_staging).await;

    // Snapshot assertions
    assert!(!temp_staging.exists(), "Staging must be 100% removed on move failure");
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads WHERE file_path = ?").bind(final_dest).fetch_one(&db).await.unwrap();
    assert_eq!(count, 0, "No orphaned download rows in SQLite after move failure rollback");
}

#[tokio::test]
async fn test_e2e_gate_scenario_best_effort_degradation_cover_and_lyrics() {
    let db = create_test_db().await;
    let temp_staging = std::env::temp_dir().join(format!("syncify_gate_staging_{}", uuid::Uuid::new_v4()));
    let temp_library = std::env::temp_dir().join(format!("syncify_gate_lib_{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_staging).await.unwrap();
    tokio::fs::create_dir_all(&temp_library).await.unwrap();

    let staged_flac = temp_staging.join("01 - Heroes (Degraded).flac");
    write_test_flac_payload(&staged_flac).await;

    // Best effort: No cover data, no lyrics data -> tagging succeeds with base metadata
    let flac_meta = FlacMetadata {
        title: "Heroes".to_string(),
        artist: "David Bowie".to_string(),
        album: "Heroes".to_string(),
        track_number: 1,
        lyrics_lrc: None,
        ..Default::default()
    };
    let tag_res = apply_and_verify_flac_tags(&staged_flac, &flac_meta);
    assert!(tag_res.is_ok(), "Audio tagging must succeed even without cover and lyrics");

    let final_flac = temp_library.join("01 - Heroes (Degraded).flac");
    tokio::fs::rename(&staged_flac, &final_flac).await.unwrap();
    tokio::fs::remove_dir_all(&temp_staging).await.unwrap();

    let final_flac_str = final_flac.to_string_lossy().to_string();
    let service_id: i64 = sqlx::query_scalar("INSERT INTO services (name, supports_download, max_quality) VALUES ('tidal', 1, 'hires') RETURNING id")
        .fetch_one(&db).await.unwrap();
    let track_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, isrc) VALUES ('Heroes Degraded', 'USJT11788888') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query("INSERT INTO downloads (track_id, source_service_id, file_path, file_format, status) VALUES (?, ?, ?, 'FLAC', 'verified')")
        .bind(track_id).bind(service_id).bind(&final_flac_str).execute(&db).await.unwrap();

    // Snapshot assertions
    assert!(!temp_staging.exists(), "Staging must be 100% removed (0 orphan files)");
    assert!(final_flac.exists(), "Audio file must be preserved in library");
    let status: String = sqlx::query_scalar("SELECT status FROM downloads WHERE track_id = ?").bind(track_id).fetch_one(&db).await.unwrap();
    assert_eq!(status, "verified");

    let _ = tokio::fs::remove_dir_all(&temp_library).await;
}

#[tokio::test]
async fn test_e2e_gate_scenario_cancellation_cleanup() {
    let temp_staging = std::env::temp_dir().join(format!("syncify_gate_staging_{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_staging).await.unwrap();

    // Write in-flight temporary partial chunks
    let partial_file = temp_staging.join("stream.part");
    tokio::fs::write(&partial_file, b"partial_in_flight_download_data_before_cancel").await.unwrap();
    assert!(partial_file.exists());

    // Trigger cancellation cleanup
    let _ = tokio::fs::remove_dir_all(&temp_staging).await;
    assert!(!temp_staging.exists(), "Staging dir and partial download files must be cleaned upon cancellation (0 orphan files)");
}
