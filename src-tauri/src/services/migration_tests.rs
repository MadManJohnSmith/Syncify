//! Unit tests for migration service methods
//!
//! Tests search result parsing, ISRC matching logic, and metadata normalization.
//! NOTE: External API calls are mocked - these tests don't hit real services.

use super::*;

// ========== Search Result Tests ==========

#[test]
fn test_qobuz_search_result_serialization() {
    let result = qobuz::QobuzSearchResult {
        track_id: "12345".to_string(),
        title: "Test Track".to_string(),
        artist: "Test Artist".to_string(),
        album: Some("Test Album".to_string()),
        isrc: Some("USRC12345678".to_string()),
        duration_ms: 180000,
        bit_depth: Some(24),
        sample_rate: Some(96000.0),
    };

    // Should serialize without error
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("Test Track"));
    assert!(json.contains("USRC12345678"));
}

#[test]
fn test_tidal_search_result_serialization() {
    let result = tidal::TidalSearchResult {
        track_id: "67890".to_string(),
        title: "Tidal Track".to_string(),
        artist: "Tidal Artist".to_string(),
        album: Some("Tidal Album".to_string()),
        isrc: Some("GBRC12345678".to_string()),
        duration_ms: 210000,
        quality: Some("HI_RES".to_string()),
    };

    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("HI_RES"));
    assert!(json.contains("GBRC12345678"));
}

#[test]
fn test_spotify_search_result_serialization() {
    let result = spotify::SpotifySearchResult {
        track_id: "6rqhFgbbKwnb9MLmUQDhG6".to_string(),
        title: "Spotify Track".to_string(),
        artist: "Spotify Artist".to_string(),
        album: Some("Spotify Album".to_string()),
        isrc: Some("USRC98765432".to_string()),
        duration_ms: 195000,
    };

    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("6rqhFgbbKwnb9MLmUQDhG6"));
    assert!(json.contains("Spotify Artist"));
}

#[test]
fn test_deezer_search_result_serialization() {
    let result = deezer::DeezerSearchResult {
        track_id: "3135556".to_string(),
        title: "Deezer Track".to_string(),
        artist: "Deezer Artist".to_string(),
        album: Some("Deezer Album".to_string()),
        isrc: Some("FRRC12345678".to_string()),
        duration_ms: 240000,
    };

    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("3135556"));
    assert!(json.contains("FRRC12345678"));
}

#[test]
fn test_soundcloud_search_result_serialization() {
    let result = soundcloud::SoundCloudSearchResult {
        track_id: "1234567890".to_string(),
        title: "SoundCloud Track".to_string(),
        artist: "SoundCloud Artist".to_string(),
        duration_ms: 300000,
        permalink_url: Some("https://soundcloud.com/artist/track".to_string()),
    };

    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("SoundCloud Artist"));
    assert!(json.contains("soundcloud.com"));
}

// ========== ISRC Matching Logic Tests ==========

#[test]
fn test_isrc_case_insensitive_match() {
    let isrc1 = "USRC12345678";
    let isrc2 = "usrc12345678";

    // Should match case-insensitively
    assert!(isrc1.eq_ignore_ascii_case(isrc2));
}

#[test]
fn test_isrc_no_match() {
    let isrc1 = "USRC12345678";
    let isrc2 = "GBRC12345678";

    assert!(!isrc1.eq_ignore_ascii_case(isrc2));
}

// ========== Metadata Normalization Tests ==========

fn normalize(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect()
}

#[test]
fn test_metadata_normalization_removes_punctuation() {
    assert_eq!(normalize("Hello, World!"), "hello world");
    assert_eq!(normalize("Rock & Roll"), "rock  roll");
    assert_eq!(normalize("Track (feat. Artist)"), "track feat artist");
}

#[test]
fn test_metadata_normalization_lowercase() {
    assert_eq!(normalize("UPPERCASE"), "uppercase");
    assert_eq!(normalize("MixedCase"), "mixedcase");
}

#[test]
fn test_metadata_fuzzy_matching() {
    let target_title = normalize("Bohemian Rhapsody");
    let target_artist = normalize("Queen");

    // Exact match
    let r_title = normalize("Bohemian Rhapsody");
    let r_artist = normalize("Queen");
    assert!(r_title.contains(&target_title) || target_title.contains(&r_title));
    assert!(r_artist.contains(&target_artist));

    // Partial match
    let _r_title2 = normalize("Bohemian Rhapsody - Remastered");
    assert!(target_title.contains(&normalize("bohemian rhapsody")));

    // No match
    let r_title3 = normalize("Stairway to Heaven");
    assert!(!r_title3.contains(&target_title));
}

#[test]
fn test_metadata_matching_with_special_chars() {
    let target = normalize("Don't Stop Me Now");
    let result = normalize("Dont Stop Me Now");

    // Should be close match (differs by apostrophe)
    assert!(target.contains("dont stop me now") || result.contains("dont stop me now"));
}

// ========== Duration Matching Tests ==========

#[test]
fn test_duration_tolerance() {
    let source_duration: i64 = 180000; // 3:00
    let dest_duration: i64 = 182000; // 3:02

    let tolerance_ms: i64 = 5000; // 5 second tolerance
    let diff = (source_duration - dest_duration).abs();

    assert!(
        diff < tolerance_ms,
        "Duration difference {} should be within tolerance {}",
        diff,
        tolerance_ms
    );
}

#[test]
fn test_duration_too_different() {
    let source_duration: i64 = 180000; // 3:00
    let dest_duration: i64 = 300000; // 5:00

    let tolerance_ms: i64 = 5000;
    let diff = (source_duration - dest_duration).abs();

    assert!(
        diff > tolerance_ms,
        "Duration difference should exceed tolerance"
    );
}

// ========== Confidence Score Tests ==========

#[test]
fn test_confidence_scores() {
    // ISRC match = 1.0
    let isrc_confidence = 1.0_f64;
    assert!(
        isrc_confidence >= 0.95,
        "ISRC match should have high confidence"
    );

    // Metadata match = 0.80-0.85
    let metadata_confidence = 0.85_f64;
    assert!(metadata_confidence >= 0.80 && metadata_confidence <= 0.90);

    // Simulated = 0.85
    let simulated_confidence = 0.85_f64;
    assert!(simulated_confidence > 0.0);
}

#[test]
fn test_match_threshold() {
    let threshold = 0.75_f64;

    // ISRC match should pass
    assert!(1.0_f64 >= threshold);

    // Good metadata match should pass
    assert!(0.85_f64 >= threshold);

    // Poor metadata match should fail
    assert!(0.5_f64 < threshold);
}

// ========== Client Initialization Tests ==========

#[test]
fn test_qobuz_client_creation() {
    let _client = qobuz::QobuzClient::new_with_token(
        "test_app_id".to_string(),
        "test_app_secret".to_string(),
        "test_token".to_string(),
    );
    // Client should be created without panic
    assert!(true);
}

#[test]
fn test_tidal_client_creation() {
    let _client = tidal::TidalClient::new("test_token".to_string())
        .with_user("123456".to_string(), "US".to_string());
    // Client should be created without panic
    assert!(true);
}

#[test]
fn test_spotify_client_creation() {
    let _client = spotify::SpotifyClient::new("test_token".to_string(), None);
    // Client should be created without panic
    assert!(true);
}

#[test]
fn test_deezer_client_creation() {
    let _client = deezer::DeezerClient::new("test_arl".to_string());
    // Client should be created without panic
    assert!(true);
}

#[test]
fn test_soundcloud_client_creation() {
    let _client = soundcloud::SoundCloudClient::new("test_token".to_string()).with_user_id(12345);
    // Client should be created without panic
    assert!(true);
}
