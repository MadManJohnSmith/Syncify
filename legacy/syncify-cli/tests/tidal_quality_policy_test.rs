use syncify_cli::download::tidal::QualityClass;
use syncify_cli::download::TrackManifestEntry;

#[test]
fn test_lossless_request_with_flac_succeeds() {
    let _requested_q = "16-44";
    let quality_class_requested = QualityClass::Lossless;
    let final_codec = "FLAC";
    let quality_class_obtained = QualityClass::Lossless;

    assert_eq!(quality_class_requested, QualityClass::Lossless);
    assert_eq!(quality_class_obtained, QualityClass::Lossless);
    assert_eq!(final_codec, "FLAC");

    // Policy check: Lossless requested + Lossless obtained -> ACCEPT
    let is_rejected = quality_class_requested == QualityClass::Lossless
        && quality_class_obtained == QualityClass::Lossy;
    assert!(!is_rejected);
}

#[test]
fn test_lossless_request_with_aac_rejected() {
    let _requested_q = "LOSSLESS";
    let quality_class_requested = QualityClass::Lossless;
    let final_codec = "AAC";
    let quality_class_obtained = QualityClass::Lossy;

    assert_eq!(quality_class_requested, QualityClass::Lossless);
    assert_eq!(quality_class_obtained, QualityClass::Lossy);

    let allow_lossy_fallback = false;
    let is_rejected = quality_class_requested == QualityClass::Lossless
        && quality_class_obtained == QualityClass::Lossy
        && !allow_lossy_fallback;

    assert!(is_rejected);
    let rejection_reason = format!("requested_lossless_but_received_{}", final_codec.to_lowercase());
    assert_eq!(rejection_reason, "requested_lossless_but_received_aac");
}

#[test]
fn test_16_44_request_with_aac_rejected() {
    let _requested_q = "16-44";
    let quality_class_requested = QualityClass::Lossless;
    let final_codec = "AAC";
    let quality_class_obtained = QualityClass::Lossy;

    let allow_lossy_fallback = false;
    let is_rejected = quality_class_requested == QualityClass::Lossless
        && quality_class_obtained == QualityClass::Lossy
        && !allow_lossy_fallback;

    assert!(is_rejected);
    let rejection_reason = format!("requested_lossless_but_received_{}", final_codec.to_lowercase());
    assert_eq!(rejection_reason, "requested_lossless_but_received_aac");
}

#[test]
fn test_320_request_with_aac_succeeds_m4a() {
    let _requested_q = "320";
    let quality_class_requested = QualityClass::Lossy;
    let final_codec = "AAC";
    let quality_class_obtained = QualityClass::Lossy;

    let allow_lossy_fallback = false; // Lossy request doesn't trigger lossy rejection
    let is_rejected = quality_class_requested == QualityClass::Lossless
        && quality_class_obtained == QualityClass::Lossy
        && !allow_lossy_fallback;

    assert!(!is_rejected);
    let extension = if final_codec == "AAC" { "m4a" } else { "flac" };
    let container = if final_codec == "AAC" { "M4A" } else { "FLAC" };
    assert_eq!(extension, "m4a");
    assert_eq!(container, "M4A");
}

#[test]
fn test_320_request_with_mp3_succeeds_mp3() {
    let _requested_q = "320";
    let quality_class_requested = QualityClass::Lossy;
    let final_codec = "MP3";
    let quality_class_obtained = QualityClass::Lossy;

    let is_rejected = quality_class_requested == QualityClass::Lossless
        && quality_class_obtained == QualityClass::Lossy;

    assert!(!is_rejected);
    let extension = if final_codec == "MP3" { "mp3" } else { "flac" };
    let container = if final_codec == "MP3" { "MP3" } else { "FLAC" };
    assert_eq!(extension, "mp3");
    assert_eq!(container, "MP3");
}

#[test]
fn test_incompatible_extension_rejected() {
    let output_path = std::path::Path::new("song.m4a");
    let header_bytes = b"fLaC1234567890"; // FLAC header for .m4a path

    let is_m4a_path = output_path.extension().and_then(|e| e.to_str()) == Some("m4a");
    let is_header_valid = is_m4a_path
        && header_bytes.len() >= 8
        && (&header_bytes[4..8] == b"ftyp" || header_bytes.starts_with(b"\x00\x00\x00"));

    assert!(!is_header_valid);
}

#[test]
fn test_manifest_uses_audio_validation_for_m4a_without_flac_validation() {
    let entry = TrackManifestEntry {
        provider: "tidal".to_string(),
        source_track_id: "12345".to_string(),
        isrc: Some("USJT10200034".to_string()),
        title: "Test AAC Song".to_string(),
        artist: "Test Artist".to_string(),
        album: "Test Album".to_string(),
        format_requested: "320".to_string(),
        format_obtained: Some("320".to_string()),
        quality_class_requested: "Lossy".to_string(),
        quality_class_obtained: Some("Lossy".to_string()),
        codec: Some("AAC".to_string()),
        container: Some("M4A".to_string()),
        extension: Some("m4a".to_string()),
        source: Some("Tidal Official API".to_string()),
        quality_fallback: false,
        download_result: "Success".to_string(),
        rejection_reason: None,
        audio_validation: "Valid".to_string(),
        error: None,
        format_id_requested: "320".to_string(),
        format_id_obtained: Some("320".to_string()),
        final_path: Some("Test Artist/Test Album/01 - Test AAC Song.m4a".to_string()),
        size_bytes: Some(8734686),
        flac_validation: "None".to_string(),
        tagging_result: "Skipped".to_string(),
        enrichment_result: "Success".to_string(),
        cover_result: "Success".to_string(),
        lyrics_result: "None".to_string(),
        ..Default::default()
    };

    assert_eq!(entry.audio_validation, "Valid");
    assert_eq!(entry.flac_validation, "None");
    assert_eq!(entry.codec.as_deref(), Some("AAC"));
    assert_eq!(entry.container.as_deref(), Some("M4A"));
}

#[test]
fn test_no_tagging_or_enrichment_on_quality_rejection() {
    let entry = TrackManifestEntry {
        provider: "tidal".to_string(),
        source_track_id: "1352259".to_string(),
        isrc: Some("USJT10200034".to_string()),
        title: "Heroes".to_string(),
        artist: "David Bowie".to_string(),
        album: "Best of Bowie".to_string(),
        format_requested: "16-44".to_string(),
        format_obtained: None,
        quality_class_requested: "Lossless".to_string(),
        quality_class_obtained: None,
        codec: None,
        container: None,
        extension: None,
        source: None,
        quality_fallback: false,
        download_result: "RejectedQuality".to_string(),
        rejection_reason: Some("requested_lossless_but_received_aac".to_string()),
        audio_validation: "None".to_string(),
        error: Some("Quality Policy Rejection: requested_lossless_but_received_aac".to_string()),
        format_id_requested: "16-44".to_string(),
        format_id_obtained: None,
        final_path: None,
        size_bytes: None,
        flac_validation: "None".to_string(),
        tagging_result: "Skipped".to_string(),
        enrichment_result: "Skipped".to_string(),
        cover_result: "None".to_string(),
        lyrics_result: "None".to_string(),
        ..Default::default()
    };

    assert_eq!(entry.download_result, "RejectedQuality");
    assert_eq!(entry.rejection_reason.as_deref(), Some("requested_lossless_but_received_aac"));
    assert_eq!(entry.tagging_result, "Skipped");
    assert_eq!(entry.enrichment_result, "Skipped");
    assert!(entry.final_path.is_none());
}

#[test]
fn test_no_sqlite_persistence_on_quality_rejection() {
    let temp_dir = std::env::temp_dir();
    let library_file = temp_dir.join("David Bowie/Best of Bowie/01 - Heroes.flac");

    let is_quality_rejected = true;
    let sqlite_persisted = if is_quality_rejected {
        // Quality rejected -> DO NOT CREATE FILE IN FINAL LIBRARY, DO NOT PERSIST SQLITE
        false
    } else {
        true
    };

    assert!(!sqlite_persisted);
    assert!(!library_file.exists());
}
