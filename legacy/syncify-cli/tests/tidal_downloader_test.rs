//! Offline unit and integration tests for Tidal Downloader master restoration

use syncify_cli::download::{StreamSourceType, TidalAuthStatus, TidalDownloader, TidalStreamResolution, TidalTrack};
use syncify_cli::services::tidal::{
    artist_matches, clean_title, score_tidal_candidate, score_tidal_release, title_matches,
    TidalAlbum, TidalArtist,
};

#[test]
fn test_stream_source_type_classification() {
    let official = StreamSourceType::TidalOfficial;
    let proxy = StreamSourceType::TidalProxy("tidal-api.binimum.org".to_string());

    assert_eq!(official.to_string(), "Tidal Official API");
    assert_eq!(proxy.to_string(), "Tidal Proxy (tidal-api.binimum.org)");
    assert_ne!(official, proxy);
}

use std::path::Path;

#[tokio::test]
async fn test_tidal_auth_status_hierarchy() {
    let downloader = TidalDownloader::new();

    // 1. Explicit user token takes precedence
    let status_explicit = downloader.check_auth_status(Some("explicit_user_token_123")).await;
    assert_eq!(status_explicit, TidalAuthStatus::UserToken("explicit_user_token_123".to_string()));

    // 2. Set user_token on downloader instance
    let downloader_with_token = TidalDownloader::new().with_user_token(Some("stored_token_abc".to_string()));
    let status_stored = downloader_with_token.check_auth_status(None).await;
    assert_eq!(status_stored, TidalAuthStatus::UserToken("stored_token_abc".to_string()));
}

#[test]
fn test_title_and_artist_matching_rules() {
    assert!(title_matches("Heroes", "Heroes (2017 Remaster)"));
    assert!(title_matches("Bohemian Rhapsody", "Bohemian Rhapsody - Remastered 2011"));
    assert!(!title_matches("Heroes", "Starman"));

    assert!(artist_matches("David Bowie", "David Bowie"));
    assert!(artist_matches("Queen", "Queen & David Bowie"));
    assert!(artist_matches("Queen / David Bowie", "Queen"));
}

#[test]
fn test_clean_title_strips_unwanted_suffixes() {
    assert_eq!(clean_title("Heroes (Remastered 2017)"), "heroes");
    assert_eq!(clean_title("No Surprises - Live at Wembley"), "no surprises");
    assert_eq!(clean_title("Numb (Deluxe Version)"), "numb");
}

#[test]
fn test_studio_candidate_scoring() {
    let score_studio = score_tidal_candidate(
        "Heroes", "David Bowie", "David Bowie", "Heroes", "", "David Bowie", true
    );
    let score_live = score_tidal_candidate(
        "Heroes Live in Berlin", "David Bowie", "David Bowie", "Heroes (Live)", "live", "David Bowie", false
    );
    let score_remix = score_tidal_candidate(
        "Heroes Remixes", "David Bowie", "David Bowie", "Heroes (Club Remix)", "remix", "David Bowie", false
    );

    assert!(score_studio > score_live, "Studio candidate must score higher than live release");
    assert!(score_studio > score_remix, "Studio candidate must score higher than remix release");
}

#[test]
fn test_proxy_api_cascade_list_decoding() {
    let apis = TidalDownloader::get_proxy_apis();
    assert!(!apis.is_empty(), "Proxy API list must decode valid URLs");
    for api in &apis {
        assert!(api.starts_with("https://"), "Decoded proxy API URL must start with https://");
    }
}

#[tokio::test]
async fn test_zero_byte_download_payload_rejection() {
    let downloader = TidalDownloader::new();
    let temp_dir = std::env::temp_dir().join(format!("tidal_test_0byte_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let output_file = temp_dir.join("test_empty.flac");

    // Attempting to download from an invalid URL should error and not leave a zero-byte file
    let res = downloader.download_audio_payload("https://httpbin.org/status/404", &output_file).await;
    assert!(res.is_err());
    assert!(!output_file.exists(), "Zero-byte file must be deleted upon failure");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
#[ignore]
async fn test_real_single_track_tidal_download_manual_ignored() {
    let downloader = TidalDownloader::new();
    let track_res = downloader.search_by_metadata("Heroes", "David Bowie", 210).await;
    assert!(track_res.is_ok(), "Real Tidal search for 'David Bowie - Heroes' should succeed");
    
    let track = track_res.unwrap();
    assert_eq!(track.title.to_lowercase(), "heroes");

    let stream_res = downloader.get_stream_resolution(track.id, Some("16-44"), None, true).await;
    assert!(stream_res.is_ok(), "Real Tidal stream resolution should return valid URL");
}
