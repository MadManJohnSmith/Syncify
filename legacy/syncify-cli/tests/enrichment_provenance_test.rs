use syncify_cli::metadata::tag_writer::{apply_and_verify_flac_tags, apply_flac_tags, verify_flac_tags, FlacMetadata};
use syncify_cli::services::enrichment::{ConflictInfo, EnrichedMetadata, EnrichmentEngine, FieldResolution};
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Helper function to construct a valid minimal FLAC file structure with STREAMINFO and audio frames.
fn create_valid_flac_file(path: &Path, audio_payload_len: usize) -> std::io::Result<()> {
    let mut file = File::create(path)?;

    // 1. fLaC marker (4 bytes)
    file.write_all(b"fLaC")?;

    // 2. STREAMINFO block header (4 bytes): type=0 (STREAMINFO), is_last=1, length=34
    let streaminfo_header: [u8; 4] = [0x80, 0x00, 0x00, 0x22];
    file.write_all(&streaminfo_header)?;

    // 3. STREAMINFO block payload (34 bytes)
    let mut streaminfo_payload = [0u8; 34];
    streaminfo_payload[0..2].copy_from_slice(&4608u16.to_be_bytes()); // min_block
    streaminfo_payload[2..4].copy_from_slice(&4608u16.to_be_bytes()); // max_block
    streaminfo_payload[10] = 0x0A;
    streaminfo_payload[11] = 0xC4;
    streaminfo_payload[12] = 0x42; // 44.1kHz, 2ch, 16bit
    streaminfo_payload[13] = 0xF0;
    file.write_all(&streaminfo_payload)?;

    // 4. Audio frames starting with FLAC sync word 0xFFF8
    let mut audio_data = vec![0u8; audio_payload_len.max(16)];
    audio_data[0] = 0xFF;
    audio_data[1] = 0xF8;
    audio_data[2] = 0x18;
    audio_data[3] = 0x00;
    file.write_all(&audio_data)?;

    Ok(())
}

#[test]
fn test_explicit_enrichment_states() {
    let resolved = FieldResolution::Resolved {
        value: "Art Rock".to_string(),
        source: "discogs".to_string(),
        confidence: 0.85,
        resolved_at: "1700000000".to_string(),
        conflict: None,
    };
    assert!(resolved.is_resolved());
    assert_eq!(resolved.value(), Some("Art Rock"));
    assert_eq!(resolved.source(), Some("discogs"));
    assert_eq!(resolved.confidence(), 0.85);

    let not_found = FieldResolution::NotFound {
        source: "discogs".to_string(),
        checked_at: "1700000000".to_string(),
    };
    assert!(!not_found.is_resolved());
    assert_eq!(not_found.source(), Some("discogs"));

    let not_supported = FieldResolution::NotSupported {
        reason: "Field not indexed".to_string(),
    };
    assert!(!not_supported.is_resolved());

    let unavailable = FieldResolution::SourceUnavailable {
        source: "lastfm".to_string(),
        error: "HTTP 503".to_string(),
    };
    assert!(!unavailable.is_resolved());

    let failed = FieldResolution::Failed {
        source: "essentia".to_string(),
        error: "Script crashed".to_string(),
        failed_at: "1700000000".to_string(),
    };
    assert!(!failed.is_resolved());

    let not_requested = FieldResolution::NotRequested;
    assert!(!not_requested.is_resolved());
}

#[test]
fn test_manual_override_preservation() {
    let mut genre_res = FieldResolution::Resolved {
        value: "My Manual Genre".to_string(),
        source: "manual".to_string(),
        confidence: 1.0,
        resolved_at: "1700000000".to_string(),
        conflict: None,
    };

    // Automated candidate from MusicBrainz must be ignored
    genre_res.merge_candidate(Some("Automated Genre".to_string()), "musicbrainz", 0.95, "1700000100");

    assert_eq!(genre_res.value(), Some("My Manual Genre"));
    assert_eq!(genre_res.source(), Some("manual"));
    assert_eq!(genre_res.confidence(), 1.0);
}

#[test]
fn test_conflict_tracking_between_valid_sources() {
    let mut style_res = FieldResolution::Resolved {
        value: "Glam Rock".to_string(),
        source: "musicbrainz".to_string(),
        confidence: 0.80,
        resolved_at: "1700000000".to_string(),
        conflict: None,
    };

    // Candidate from Discogs with higher confidence
    style_res.merge_candidate(Some("Art Rock".to_string()), "discogs", 0.90, "1700000100");

    assert_eq!(style_res.value(), Some("Art Rock"));
    assert_eq!(style_res.source(), Some("discogs"));
    assert_eq!(style_res.confidence(), 0.90);

    if let FieldResolution::Resolved { conflict: Some(ref conflict), .. } = style_res {
        assert_eq!(conflict.alternate_source, "musicbrainz");
        assert_eq!(conflict.alternate_value, "Glam Rock");
        assert_eq!(conflict.alternate_confidence, 0.80);
    } else {
        panic!("Conflict expected when higher confidence candidate replaces existing value");
    }
}

#[test]
fn test_protection_against_empty_and_placeholder_values() {
    let mut res = FieldResolution::Resolved {
        value: "Valid Genre".to_string(),
        source: "discogs".to_string(),
        confidence: 0.85,
        resolved_at: "1700000000".to_string(),
        conflict: None,
    };

    res.merge_candidate(Some("".to_string()), "secondary", 0.99, "1700000100");
    assert_eq!(res.value(), Some("Valid Genre"));

    res.merge_candidate(Some("   ".to_string()), "secondary", 0.99, "1700000100");
    assert_eq!(res.value(), Some("Valid Genre"));

    res.merge_candidate(Some("Unknown".to_string()), "secondary", 0.99, "1700000100");
    assert_eq!(res.value(), Some("Valid Genre"));

    res.merge_candidate(None, "secondary", 0.99, "1700000100");
    assert_eq!(res.value(), Some("Valid Genre"));
}

#[test]
fn test_flac_roundtrip_and_tag_preservation() {
    let temp_flac = std::env::temp_dir().join(format!("test_roundtrip_{}.flac", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    create_valid_flac_file(&temp_flac, 1024).expect("Failed to create FLAC file");

    // Write a pre-existing custom tag into FLAC
    {
        let mut tag = metaflac::Tag::read_from_path(&temp_flac).unwrap();
        tag.vorbis_comments_mut().set("PRE_EXISTING_CUSTOM_TAG", vec!["DoNotDelete".to_string()]);
        tag.write_to_path(&temp_flac).unwrap();
    }

    let meta = FlacMetadata {
        title: "Heroes".to_string(),
        artist: "David Bowie".to_string(),
        album: "Heroes".to_string(),
        genre: Some("Rock".to_string()),
        style: Some("Art Rock".to_string()),
        mood: Some("epic".to_string()),
        release_type: Some("Album".to_string()),
        release_status: Some("Official".to_string()),
        release_country: Some("United States".to_string()),
        language: Some("English".to_string()),
        label: Some("Parlophone".to_string()),
        barcode: Some("078635388022".to_string()),
        catalog_number: Some("AFL1-2522".to_string()),
        original_date: Some("1977-10-14".to_string()),
        isrc: Some("GBUM71029604".to_string()),
        bpm: Some(112),
        initial_key: Some("G major".to_string()),
        lyrics_lrc: Some("[00:05.00] I, I wish you could swim\n[00:10.00] Like dolphins, like dolphins can swim".to_string()),
        ..Default::default()
    };

    let verification = apply_and_verify_flac_tags(&temp_flac, &meta).expect("apply_and_verify_flac_tags failed");

    assert!(verification.file_exists);
    assert!(verification.flac_valid);
    assert!(verification.tags_match);
    assert!(verification.bpm_present);
    assert!(verification.lyrics_present);
    assert!(verification.synced_lyrics_present);
    assert!(verification.unsynced_lyrics_present);
    assert!(verification.mismatches.is_empty());

    // Confirm pre-existing unrelated tag was preserved
    let tag_after = metaflac::Tag::read_from_path(&temp_flac).unwrap();
    let comments = tag_after.vorbis_comments().unwrap();
    let pre_existing = comments.get("PRE_EXISTING_CUSTOM_TAG").and_then(|v| v.first().map(|s| s.as_str()));
    assert_eq!(pre_existing, Some("DoNotDelete"), "Pre-existing unrelated VorbisComment tag must be preserved");

    let _ = std::fs::remove_file(&temp_flac);
}

#[test]
fn test_cover_art_jpeg_and_webp_verification() {
    let temp_flac = std::env::temp_dir().join(format!("test_cover_{}.flac", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    create_valid_flac_file(&temp_flac, 1024).expect("Failed to create FLAC file");

    let mock_jpeg: Vec<u8> = vec![
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01,
        0x01, 0x01, 0x00, 0x48, 0x00, 0x48, 0x00, 0x00, 0xFF, 0xD9
    ];

    let meta = FlacMetadata {
        title: "Test Track".to_string(),
        artist: "Test Artist".to_string(),
        album: "Test Album".to_string(),
        cover_data: Some(mock_jpeg.clone()),
        ..Default::default()
    };

    let ver = apply_and_verify_flac_tags(&temp_flac, &meta).expect("Tag writing & verification failed");
    assert!(ver.cover_present);
    assert_eq!(ver.cover_size_bytes, Some(mock_jpeg.len()));
    assert_eq!(ver.cover_mime, Some("image/jpeg".to_string()));

    // Verify picture block non-duplication when writing new picture
    let mock_webp: Vec<u8> = vec![
        b'R', b'I', b'F', b'F', 0x10, 0x00, 0x00, 0x00, b'W', b'E', b'B', b'P',
        b'V', b'P', b'8', b' '
    ];
    let meta_webp = FlacMetadata {
        title: "Test Track".to_string(),
        artist: "Test Artist".to_string(),
        album: "Test Album".to_string(),
        cover_data: Some(mock_webp.clone()),
        ..Default::default()
    };

    let ver_webp = apply_and_verify_flac_tags(&temp_flac, &meta_webp).expect("WebP tag writing failed");
    assert!(ver_webp.cover_present);
    assert_eq!(ver_webp.cover_size_bytes, Some(mock_webp.len()));
    assert_eq!(ver_webp.cover_mime, Some("image/webp".to_string()));

    // Check metaflac picture count is exactly 1 (no duplicate cover blocks)
    let tag = metaflac::Tag::read_from_path(&temp_flac).unwrap();
    assert_eq!(tag.pictures().count(), 1, "Must contain exactly 1 picture block without duplication");

    let _ = std::fs::remove_file(&temp_flac);
}

#[tokio::test]
async fn test_sqlite_persistence_only_after_validation() {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to SQLite in-memory DB");

    sqlx::query(
        "CREATE TABLE tracks (
            id INTEGER PRIMARY KEY,
            title TEXT,
            artist TEXT,
            album TEXT,
            genre TEXT,
            style TEXT,
            mood TEXT,
            bpm REAL,
            initial_key TEXT,
            genre_source_type TEXT DEFAULT 'enrichment',
            style_source_type TEXT DEFAULT 'enrichment',
            mood_source_type TEXT DEFAULT 'enrichment',
            bpm_source_type TEXT DEFAULT 'enrichment',
            key_source_type TEXT DEFAULT 'enrichment',
            label_source_type TEXT DEFAULT 'enrichment'
        )",
    )
    .execute(&pool)
    .await
    .expect("Failed to create tracks table");

    sqlx::query("INSERT INTO tracks (id, title, artist, album) VALUES (1, 'Heroes', 'David Bowie', 'Heroes')")
        .execute(&pool)
        .await
        .unwrap();

    let temp_flac = std::env::temp_dir().join(format!("test_sqlite_val_{}.flac", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    create_valid_flac_file(&temp_flac, 1024).expect("Failed to create FLAC file");

    let meta = FlacMetadata {
        title: "Heroes".to_string(),
        artist: "David Bowie".to_string(),
        album: "Heroes".to_string(),
        genre: Some("Art Rock".to_string()),
        bpm: Some(112),
        ..Default::default()
    };

    // 1. Validate FLAC write first
    let ver = apply_and_verify_flac_tags(&temp_flac, &meta).expect("FLAC tag verification failed");
    assert!(ver.tags_match, "Tags must match before persisting to SQLite");

    // 2. Only persist to SQLite after validation passes
    let engine = EnrichmentEngine::new();
    let enriched = EnrichedMetadata {
        genre: Some("Art Rock".to_string()),
        bpm: Some(112.0),
        ..Default::default()
    };

    engine.apply_to_track(&pool, 1, &enriched).await.expect("DB persistence failed");

    // 3. Query back from SQLite
    let row: (String, f64) = sqlx::query_as("SELECT genre, bpm FROM tracks WHERE id = 1")
        .fetch_one(&pool)
        .await
        .expect("Failed to fetch track from SQLite");

    assert_eq!(row.0, "Art Rock");
    assert_eq!(row.1, 112.0);

    let _ = std::fs::remove_file(&temp_flac);
}
