use syncify_metadata_domain::*;
use syncify_tauri_lib::services::enrichment::EnrichmentEngine;
use syncify_tauri_lib::services::tag_writer::{apply_flac_tags, verify_flac_tags, FlacMetadata};

#[test]
fn test_metadata_domain_parity_and_precedence_invariants() {
    let mut meta = EnrichedMetadata::default();
    let now = chrono_now_iso();

    // 1. Manual source is immutable against any higher-confidence candidate
    meta.title.merge_candidate(Some("Manual Title Override".to_string()), "manual", 1.0, &now);
    meta.title.merge_candidate(Some("Streaming Title".to_string()), "qobuz", 0.95, &now);
    meta.title.merge_candidate(Some("MB Title".to_string()), "musicbrainz", 0.99, &now);
    assert_eq!(meta.title.value(), Some("Manual Title Override"));
    assert_eq!(meta.title.source(), Some("manual"));

    // 2. Streaming priority beats MusicBrainz
    meta.album.merge_candidate(Some("MusicBrainz Album".to_string()), "musicbrainz", 0.95, &now);
    meta.album.merge_candidate(Some("Official Qobuz Album".to_string()), "qobuz", 0.90, &now);
    assert_eq!(meta.album.value(), Some("Official Qobuz Album"));
    assert_eq!(meta.album.source(), Some("qobuz"));

    // 3. Rejection of invalid placeholders
    assert!(!FieldValidator::is_valid_year("0000"));
    assert!(!FieldValidator::is_valid_year("0"));
    assert!(FieldValidator::is_valid_year("1977"));
    assert!(!FieldValidator::is_valid_identifier(""));
    assert!(!FieldValidator::is_valid_identifier("0"));
    assert!(!FieldValidator::is_valid_identifier("null"));
    assert!(FieldValidator::is_valid_identifier("GBAYE7700021"));
    assert!(FieldValidator::is_valid_artist("Various Artists"));
    assert!(FieldValidator::is_valid_artist("Various"));
    assert!(!FieldValidator::is_valid_artist("???"));
}

#[tokio::test]
async fn test_flac_tagging_and_conditional_sqlite_persistence_roundtrip() {
    let candidate_paths = [
        "downloads/05 - I Will Survive.flac",
        "tests/fixtures/05 - I Will Survive.flac",
        "adjacent_tools/streamrip/tests/silence.flac",
    ];

    let mut real_flac = None;
    for c in &candidate_paths {
        let p = std::path::Path::new("c:/Users/tardis/Documents/Syncify").join(c);
        if p.exists() {
            real_flac = Some(p);
            break;
        }
    }

    let src_path = real_flac.expect("Real FLAC candidate track must exist in workspace");
    let temp_dir = std::env::temp_dir().join(format!("syncify_parity_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let flac_path = temp_dir.join("test_track.flac");
    std::fs::copy(&src_path, &flac_path).unwrap();

    // 1. Write FLAC tags with metaflac
    let flac_meta = FlacMetadata {
        title: "Heroes".to_string(),
        artist: "David Bowie".to_string(),
        album: "Heroes".to_string(),
        album_artist: Some("David Bowie".to_string()),
        performers: Some("David Bowie".to_string()),
        label: Some("RCA Victor".to_string()),
        barcode: Some("0035629007421".to_string()),
        catalog_number: Some("PL 12522".to_string()),
        original_date: Some("1977-10-14".to_string()),
        track_number: 1,
        track_total: 10,
        disc_number: 1,
        disc_total: 1,
        isrc: Some("GBAYE7700021".to_string()),
        release_year: Some("1977".to_string()),
        musicbrainz_track_id: Some("b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d".to_string()),
        musicbrainz_artist_id: Some("5441c29d-3602-48f7-b1a9-30704df52227".to_string()),
        musicbrainz_album_id: Some("673752e3-2e06-4447-aa72-a080ef8a1768".to_string()),
        musicbrainz_albumartist_id: Some("5441c29d-3602-48f7-b1a9-30704df52227".to_string()),
        musicbrainz_release_group_id: Some("c0e9b90c-d9c0-3ec6-b33a-bcbbd011f061".to_string()),
        ..Default::default()
    };

    apply_flac_tags(&flac_path, &flac_meta).unwrap();

    // 2. Verify re-read
    let verification = verify_flac_tags(&flac_path, &flac_meta).unwrap();
    assert!(verification.tags_match);
    assert!(verification.flac_valid);

    // 3. Conditional SQLite persistence test
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query("CREATE TABLE artists (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, musicbrainz_id TEXT);")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE tracks (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT, album_id INTEGER, track_number INTEGER, disc_number INTEGER, isrc TEXT, release_year INTEGER, record_label TEXT, musicbrainz_id TEXT, enrichment_status TEXT, enriched_at TEXT);")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE track_artists (track_id INTEGER, artist_id INTEGER, role TEXT, PRIMARY KEY(track_id, artist_id));")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE albums (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT, release_date TEXT, upc TEXT, total_tracks INTEGER, label TEXT, musicbrainz_id TEXT);")
        .execute(&pool).await.unwrap();

    sqlx::query("INSERT INTO artists (name) VALUES ('David Bowie');").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO albums (title) VALUES ('Heroes');").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO tracks (title, album_id, enrichment_status) VALUES ('Heroes', 1, 'pending');").execute(&pool).await.unwrap();

    let mut enriched = EnrichedMetadata::default();
    let now = chrono_now_iso();
    enriched.title.merge_candidate(Some("Heroes".to_string()), "stream", 1.0, &now);
    enriched.artist.merge_candidate(Some("David Bowie".to_string()), "stream", 1.0, &now);
    enriched.album.merge_candidate(Some("Heroes".to_string()), "stream", 1.0, &now);
    enriched.track_number.merge_candidate(Some("1".to_string()), "stream", 1.0, &now);
    enriched.disc_number.merge_candidate(Some("1".to_string()), "stream", 1.0, &now);
    enriched.track_total.merge_candidate(Some("10".to_string()), "stream", 0.95, &now);
    enriched.disc_total.merge_candidate(Some("1".to_string()), "stream", 0.95, &now);
    enriched.isrc.merge_candidate(Some("GBAYE7700021".to_string()), "stream", 0.95, &now);
    enriched.barcode.merge_candidate(Some("0035629007421".to_string()), "stream", 0.95, &now);
    enriched.release_year.merge_candidate(Some("1977".to_string()), "musicbrainz", 0.90, &now);
    enriched.original_date.merge_candidate(Some("1977-10-14".to_string()), "musicbrainz", 0.90, &now);
    enriched.label.merge_candidate(Some("RCA Victor".to_string()), "musicbrainz", 0.85, &now);
    enriched.catalog_number.merge_candidate(Some("PL 12522".to_string()), "musicbrainz", 0.85, &now);
    enriched.musicbrainz_recording_id.merge_candidate(Some("b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d".to_string()), "musicbrainz", 0.95, &now);
    enriched.musicbrainz_artist_id.merge_candidate(Some("5441c29d-3602-48f7-b1a9-30704df52227".to_string()), "musicbrainz", 0.95, &now);
    enriched.musicbrainz_release_id.merge_candidate(Some("673752e3-2e06-4447-aa72-a080ef8a1768".to_string()), "musicbrainz", 0.95, &now);
    enriched.musicbrainz_release_group_id.merge_candidate(Some("c0e9b90c-d9c0-3ec6-b33a-bcbbd011f061".to_string()), "musicbrainz", 0.95, &now);

    let engine = EnrichmentEngine::new();
    let persist_res: Result<(), String> = engine.apply_to_database(&pool, 1, &enriched, Some(&flac_path)).await;
    assert!(persist_res.is_ok());

    // Assert database state after successful re-read verification
    let (t_title, t_isrc, t_mbid, t_status, t_year, t_label): (String, Option<String>, Option<String>, Option<String>, Option<i64>, Option<String>) =
        sqlx::query_as("SELECT title, isrc, musicbrainz_id, enrichment_status, release_year, record_label FROM tracks WHERE id = 1")
            .fetch_one(&pool).await.unwrap();

    assert_eq!(t_title, "Heroes");
    assert_eq!(t_isrc.as_deref(), Some("GBAYE7700021"));
    assert_eq!(t_mbid.as_deref(), Some("b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d"));
    assert_eq!(t_status.as_deref(), Some("complete"));
    assert_eq!(t_year, Some(1977));
    assert_eq!(t_label.as_deref(), Some("RCA Victor"));

    let (alb_title, alb_date, alb_upc, alb_tracks, alb_mbid): (String, Option<String>, Option<String>, Option<i64>, Option<String>) =
        sqlx::query_as("SELECT title, release_date, upc, total_tracks, musicbrainz_id FROM albums WHERE id = 1")
            .fetch_one(&pool).await.unwrap();

    assert_eq!(alb_title, "Heroes");
    assert_eq!(alb_date.as_deref(), Some("1977-10-14"));
    assert_eq!(alb_upc.as_deref(), Some("0035629007421"));
    assert_eq!(alb_tracks, Some(10));
    assert_eq!(alb_mbid.as_deref(), Some("673752e3-2e06-4447-aa72-a080ef8a1768"));

    // Cleanup temp files
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_tauri_consumes_syncify_core_domain_pure_contracts() {
    use syncify_core_domain::quality::{QualityClass, QualityPolicy};
    use syncify_core_domain::errors::{PipelineError, RequiresAuthReason};
    use syncify_core_domain::manifest::{TrackManifestEntry, FavoritesBatchSummary};
    use syncify_core_domain::events::{PipelineStepStatus, PipelineProgressEvent};
    use syncify_core_domain::cover_rules::{CoverType, CoverPreservationPolicy, CoverUpdateDecision};
    use syncify_core_domain::byte_validators::{AudioByteValidator, WebpByteValidator};
    use syncify_core_domain::metadata::{TidalTrack, score_tidal_candidate, clean_title};

    // 1. Quality contract & downgrade evaluation
    assert_eq!(QualityClass::Lossless.to_string(), "Lossless");
    assert!(QualityPolicy::evaluate_downgrade(QualityClass::Lossless, QualityClass::Lossy, "AAC", false).is_err());
    assert!(QualityPolicy::evaluate_downgrade(QualityClass::Lossless, QualityClass::Lossy, "AAC", true).is_ok());

    // 2. Error classification
    let err = PipelineError::RequiresAuth(RequiresAuthReason::NoCredentialsStored);
    assert_eq!(err.to_string(), "Authentication required: No active credentials stored");


    // 3. Manifest contract
    let entry = TrackManifestEntry {
        queue_id: Some(1),
        track_id: Some(101),
        provider: "tidal".to_string(),
        source_track_id: "12345".to_string(),
        isrc: Some("USRC12345678".to_string()),
        title: "Test Track".to_string(),
        artist: "Test Artist".to_string(),
        album: "Test Album".to_string(),
        format_requested: "lossless".to_string(),
        format_obtained: Some("lossless".to_string()),
        quality_class_requested: "Lossless".to_string(),
        quality_class_obtained: Some("Lossless".to_string()),
        codec: Some("FLAC".to_string()),
        container: Some("FLAC".to_string()),
        extension: Some("flac".to_string()),
        source: Some("TidalOfficial".to_string()),
        quality_fallback: false,
        download_result: "Success".to_string(),
        rejection_reason: None,
        audio_validation: "Valid".to_string(),
        error: None,
        format_id_requested: "LOSSLESS".to_string(),
        format_id_obtained: Some("LOSSLESS".to_string()),
        final_path: Some("C:/music/track.flac".to_string()),
        size_bytes: Some(25_000_000),
        flac_validation: "Valid".to_string(),
        tagging_result: "Success".to_string(),
        enrichment_result: "Success".to_string(),
        cover_result: "StaticAndAnimated".to_string(),
        lyrics_result: "WordSynced".to_string(),
        created_artifacts: vec!["C:/music/track.flac".to_string()],
        bit_depth: Some(16),
        sample_rate: Some(44100),
        created_at: None,
        completed_at: None,
    };
    assert_eq!(entry.is_success(), true);

    let summary = FavoritesBatchSummary {
        requested: 1,
        succeeded: 1,
        manifest: vec![entry],
        ..Default::default()
    };
    assert_eq!(summary.all_succeeded(), true);

    // 4. Progress event
    let event = PipelineProgressEvent::new("track-1", "tidal", PipelineStepStatus::Tagging);
    assert_eq!(event.status, PipelineStepStatus::Tagging);

    // 5. Cover preservation invariant
    assert_eq!(
        CoverPreservationPolicy::evaluate(CoverType::AnimatedWebp, CoverType::StaticJpeg),
        CoverUpdateDecision::PreserveExisting
    );

    // 6. Byte validators
    assert!(AudioByteValidator::is_flac_magic(b"fLaC\x00\x00\x00\x22"));
    assert!(!AudioByteValidator::is_flac_magic(b"RIFF\x00\x00\x00\x00"));
    assert_eq!(WebpByteValidator::detect_cover_type(b""), CoverType::None);
    assert_eq!(WebpByteValidator::detect_cover_type(b"\xFF\xD8\xFF\xE0"), CoverType::StaticJpeg);

    // 7. Metadata models & scoring
    let track = TidalTrack {
        id: 100,
        title: "Test Track (Live Remaster 2024)".to_string(),
        duration: 210,
        track_number: Some(1),
        volume_number: Some(1),
        isrc: Some("USRC12345678".to_string()),
        audio_quality: Some("HI_RES_LOSSLESS".to_string()),
        version: None,
        artist: None,
        artists: None,
        album: None,
        media_metadata: None,
    };
    assert_eq!(clean_title(&track.title), "test track");
    let score = score_tidal_candidate("Heroes", "David Bowie", "David Bowie", "Heroes", "", "David Bowie", true);
    assert!(score >= 50);
}

#[test]
fn test_tauri_consumes_syncify_flac_writer_shared_module() {
    use syncify_tauri_lib::services::tag_writer::{
        apply_flac_tags, verify_flac_tags, audit_flac_stage, FlacMetadata,
    };

    let temp_dir = std::env::temp_dir().join(format!("tauri_flac_writer_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let flac_path = temp_dir.join("test_tauri.flac");

    // Initialize mock FLAC
    let mut initial_tag = metaflac::Tag::new();
    initial_tag.vorbis_comments_mut().set_title(vec!["Initial Title".to_string()]);
    initial_tag.write_to_path(&flac_path).unwrap();

    let meta = FlacMetadata {
        title: "Tauri Track".to_string(),
        artist: "Tauri Artist".to_string(),
        album: "Tauri Album".to_string(),
        track_number: 1,
        track_total: 10,
        genre: Some("Electronic".to_string()),
        isrc: Some("US1234567890".to_string()),
        release_year: Some("2026".to_string()),
        lyrics_lrc: Some("[00:05.00] Tauri Line 1\n[00:10.00] Tauri Line 2".to_string()),
        ..Default::default()
    };

    apply_flac_tags(&flac_path, &meta).expect("Failed to apply FLAC tags via Tauri tag_writer");

    let verification = verify_flac_tags(&flac_path, &meta).expect("Failed to verify FLAC tags");
    assert!(verification.file_exists);
    assert!(verification.flac_valid);
    assert!(verification.tags_match);
    assert!(verification.lyrics_present);
    assert!(verification.synced_lyrics_present);
    assert!(verification.unsynced_lyrics_present);
    assert!(verification.mismatches.is_empty());

    let audit = audit_flac_stage("TauriStage", &flac_path).expect("Failed to audit FLAC stage");
    assert_eq!(audit.stage, "TauriStage");
    assert_eq!(audit.picture_count, 0);

    let _ = std::fs::remove_dir_all(&temp_dir);
}


