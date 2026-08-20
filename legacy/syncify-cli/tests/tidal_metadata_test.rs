use syncify_cli::download::TrackManifestEntry;
use syncify_cli::services::tidal::TidalTrack;

#[test]
fn test_track_and_album_different_names() {
    let raw_json = r#"{
        "id": 562214,
        "title": "So What",
        "duration": 562,
        "trackNumber": 1,
        "volumeNumber": 1,
        "artist": { "id": 123, "name": "Miles Davis" },
        "album": { "id": 999, "title": "Kind of Blue" }
    }"#;

    let track: TidalTrack = serde_json::from_str(raw_json).expect("Failed to parse TidalTrack JSON");

    assert_eq!(track.clean_title(), "So What");
    assert_eq!(track.artist_name().as_deref(), Some("Miles Davis"));
    assert_eq!(track.album_title().as_deref(), Some("Kind of Blue"));
    assert_ne!(track.clean_title(), track.album_title().unwrap());
}

#[test]
fn test_remaster_suffix_separation() {
    let raw_json = r#"{
        "id": 80654035,
        "title": "\"Heroes\" (2017 Remaster)",
        "duration": 371,
        "artist": { "id": 4768, "name": "David Bowie" },
        "album": { "id": 80654032, "title": "\"Heroes\"" }
    }"#;

    let track: TidalTrack = serde_json::from_str(raw_json).expect("Failed to parse TidalTrack JSON");

    assert_eq!(track.title, "\"Heroes\" (2017 Remaster)");
    assert_eq!(track.album_title().as_deref(), Some("\"Heroes\""));
    assert_eq!(track.release_id(), Some(80654032));
}

#[test]
fn test_absent_album_returns_none_without_fallback() {
    let raw_json = r#"{
        "id": 99999,
        "title": "Orphan Track",
        "duration": 200,
        "artist": { "id": 555, "name": "Solo Artist" },
        "album": null
    }"#;

    let track: TidalTrack = serde_json::from_str(raw_json).expect("Failed to parse TidalTrack JSON");

    assert_eq!(track.clean_title(), "Orphan Track");
    assert_eq!(track.album_title(), None);
    assert_ne!(track.album_title().as_deref(), Some(track.title.as_str()));
    assert_ne!(track.album_title().as_deref(), Some("Unknown Album"));
}

#[test]
fn test_compilation_album_artist_distinct_from_track_artist() {
    let raw_json = r#"{
        "id": 77777,
        "title": "Track on Compilation",
        "duration": 240,
        "artist": { "id": 101, "name": "Featured Performer" },
        "album": {
            "id": 888,
            "title": "Best of 80s",
            "artist": { "id": 999, "name": "Various Artists" }
        }
    }"#;

    let track: TidalTrack = serde_json::from_str(raw_json).expect("Failed to parse TidalTrack JSON");

    assert_eq!(track.artist_name().as_deref(), Some("Featured Performer"));
    assert_eq!(track.album_artist_name().as_deref(), Some("Various Artists"));
    assert_ne!(track.artist_name(), track.album_artist_name());
}

#[test]
fn test_tidal_manifest_schema_has_no_qobuz_fields() {
    let entry = TrackManifestEntry {
        provider: "tidal".to_string(),
        source_track_id: "80654035".to_string(),
        isrc: Some("USJT11700035".to_string()),
        title: "\"Heroes\" (2017 Remaster)".to_string(),
        artist: "David Bowie".to_string(),
        album: "\"Heroes\" (2017 Remaster)".to_string(),
        format_requested: "HI_RES_LOSSLESS".to_string(),
        format_obtained: Some("HI_RES_LOSSLESS".to_string()),
        quality_class_requested: "Lossless".to_string(),
        quality_class_obtained: Some("Lossless".to_string()),
        codec: Some("FLAC".to_string()),
        container: Some("FLAC".to_string()),
        extension: Some("flac".to_string()),
        source: Some("Tidal Official Stream Direct".to_string()),
        quality_fallback: false,
        download_result: "Success".to_string(),
        rejection_reason: None,
        audio_validation: "Valid".to_string(),
        error: None,
        format_id_requested: "HI_RES_LOSSLESS".to_string(),
        format_id_obtained: Some("HI_RES_LOSSLESS".to_string()),
        final_path: Some("downloads_tidal_test/David Bowie/Heroes/01 - Heroes.flac".to_string()),
        size_bytes: Some(139450849),
        flac_validation: "Valid".to_string(),
        tagging_result: "Success".to_string(),
        enrichment_result: "Success".to_string(),
        cover_result: "Success".to_string(),
        lyrics_result: "None".to_string(),
        ..Default::default()
    };

    let serialized = serde_json::to_string(&entry).expect("Failed to serialize TrackManifestEntry");

    assert!(serialized.contains("\"provider\":\"tidal\""));
    assert!(serialized.contains("\"source_track_id\":\"80654035\""));
    assert!(!serialized.contains("qobuz_track_id"));
}
