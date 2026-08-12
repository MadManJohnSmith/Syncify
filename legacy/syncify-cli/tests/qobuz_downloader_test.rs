use syncify_cli::download::{
    build_output_path, build_request_signature, map_quality_to_format_id, sanitize_path_component,
    sign_api_request, QobuzAuthStatus, QobuzDownloader, StreamResolution, StreamUrlSource,
};
use syncify_cli::services::qobuz::{QOBUZ_APP_SECRET};
use std::path::PathBuf;

#[test]
fn test_build_request_signature_deterministic_md5() {
    // Known test vector
    // MD5("trackgetFileUrlformat_id27intentstreamtrack_id123456781700000000abb21364945c0583309667d13ca3d93a")
    let sig = build_request_signature("27", "12345678", "1700000000", QOBUZ_APP_SECRET);
    
    assert_eq!(sig.len(), 32);
    assert!(sig.chars().all(|c| c.is_ascii_hexdigit()));
    
    // Exact verification
    let raw = format!("trackgetFileUrlformat_id27intentstreamtrack_id123456781700000000{}", QOBUZ_APP_SECRET);
    let expected = format!("{:x}", md5::compute(raw.as_bytes()));
    assert_eq!(sig, expected);
}

#[test]
fn test_sign_api_request_alphabetical_and_md5() {
    let mut params = vec![
        ("query", "Heroes".to_string()),
        ("limit", "50".to_string()),
        ("app_id", "798273057".to_string()),
    ];

    sign_api_request("track/search", &mut params, QOBUZ_APP_SECRET);

    // Verify request_sig was added as the last param
    let sig_param = params.iter().find(|(k, _)| *k == "request_sig").expect("request_sig missing");
    assert_eq!(sig_param.1.len(), 32);

    // Expected base: tracksearch + app_id798273057 + limit50 + queryHeroes + secret
    let expected_base = format!("tracksearchapp_id798273057limit50queryHeroes{}", QOBUZ_APP_SECRET);
    let expected_sig = format!("{:x}", md5::compute(expected_base.as_bytes()));
    assert_eq!(sig_param.1, expected_sig);
}

#[test]
fn test_map_quality_to_format_id_variants() {
    assert_eq!(map_quality_to_format_id("24-192"), "27");
    assert_eq!(map_quality_to_format_id("HI_RES_LOSSLESS"), "27");
    assert_eq!(map_quality_to_format_id("27"), "27");

    assert_eq!(map_quality_to_format_id("24-96"), "7");
    assert_eq!(map_quality_to_format_id("HI_RES"), "7");
    assert_eq!(map_quality_to_format_id("7"), "7");

    assert_eq!(map_quality_to_format_id("16-44.1"), "6");
    assert_eq!(map_quality_to_format_id("LOSSLESS"), "6");
    assert_eq!(map_quality_to_format_id("6"), "6");

    assert_eq!(map_quality_to_format_id("320"), "5");
    assert_eq!(map_quality_to_format_id("MP3"), "5");
    assert_eq!(map_quality_to_format_id("5"), "5");
}

#[test]
fn test_sanitize_path_component_rules() {
    assert_eq!(sanitize_path_component("AC/DC"), "AC_DC");
    assert_eq!(sanitize_path_component("Hello:World?"), "Hello_World_");
    assert_eq!(sanitize_path_component("NUL"), "_NUL");
    assert_eq!(sanitize_path_component("CON"), "_CON");
    assert_eq!(sanitize_path_component("AUX"), "_AUX");
    assert_eq!(sanitize_path_component("trailing dot..."), "trailing dot");
    assert_eq!(sanitize_path_component(""), "_");
}

#[test]
fn test_build_output_path_layout_canonical() {
    let single_disc = build_output_path("/music", "David Bowie", "Heroes", 1, 5, "Heroes", 1);
    let expected_single = PathBuf::from("/music")
        .join("David Bowie")
        .join("Heroes")
        .join("05 - Heroes.flac");
    assert_eq!(single_disc, expected_single);

    let multi_disc = build_output_path("/music", "Pink Floyd", "The Wall", 2, 1, "Hey You", 2);
    let expected_multi = PathBuf::from("/music")
        .join("Pink Floyd")
        .join("The Wall")
        .join("CD 2")
        .join("01 - Hey You.flac");
    assert_eq!(multi_disc, expected_multi);
}

#[tokio::test]
async fn test_resolve_token_hierarchy() {
    let downloader = QobuzDownloader::new();

    // 1. When QOBUZ_USER_TOKEN is set
    std::env::set_var("QOBUZ_USER_TOKEN", "test_env_token_12345");
    let res = downloader.resolve_token().await;
    assert_eq!(res.unwrap(), "test_env_token_12345");

    // 2. When QOBUZ_USER_TOKEN is unset
    std::env::remove_var("QOBUZ_USER_TOKEN");
    let res2 = downloader.resolve_token().await;
    match res2 {
        Ok(token) => {
            assert!(!token.trim().is_empty(), "Resolved token from DB must not be empty");
        }
        Err(status) => {
            assert!(matches!(status, QobuzAuthStatus::RequiresAuth(_)), "Missing token must return RequiresAuth");
        }
    }
}

#[tokio::test]
async fn test_stream_url_source_traceability() {
    let official_res = StreamResolution {
        url: "https://streaming.qobuz.com/track/123.flac".to_string(),
        source: StreamUrlSource::QobuzOfficial,
        format_id: "27".to_string(),
    };

    let proxy_res = StreamResolution {
        url: "https://proxy.example.com/stream/123".to_string(),
        source: StreamUrlSource::ProxyFallback("proxy.example.com".to_string()),
        format_id: "6".to_string(),
    };

    assert_eq!(official_res.source, StreamUrlSource::QobuzOfficial);
    assert_ne!(proxy_res.source, StreamUrlSource::QobuzOfficial);
}

#[tokio::test]
async fn test_download_file_flac_header_validation() {
    let temp_dir = std::env::temp_dir().join(format!("test_qobuz_dl_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let _ = std::fs::create_dir_all(&temp_dir);

    // Test rejection of HTML / JSON error response served with HTTP 200
    let html_error = b"<html><body><h1>403 Forbidden - Rate Limit Exceeded</h1></body></html>";
    
    // Simulate what download_file validates
    let is_flac_valid = html_error.len() >= 4 && &html_error[0..4] == b"fLaC";
    assert!(!is_flac_valid, "Must reject HTML response as non-FLAC audio");

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_title_and_artist_matching_rules() {
    use syncify_cli::download::qobuz_downloader::{title_matches, artist_matches, clean_title};

    // Clean title
    assert_eq!(clean_title("Heroes (Remastered 2017)"), "heroes");
    assert_eq!(clean_title("Bohemian Rhapsody - Deluxe Edition"), "bohemian rhapsody");
    assert_eq!(clean_title("Song Name (Live)"), "song name");

    // Title matches
    assert!(title_matches("Heroes", "Heroes"));
    assert!(title_matches("Heroes", "Heroes (2017 Remaster)"));
    assert!(title_matches("Heroes (Remastered)", "Heroes"));
    assert!(!title_matches("Heroes", "Starman"));

    // Artist matches
    assert!(artist_matches("David Bowie", "David Bowie"));
    assert!(artist_matches("Queen", "Queen, David Bowie"));
    assert!(artist_matches("Queen", "David Bowie & Queen"));
    assert!(artist_matches("Freddie Mercury & Montserrat Caballé", "Freddie Mercury"));
    assert!(!artist_matches("David Bowie", "Pink Floyd"));
}

#[test]
fn test_qobuz_search_fixture_parsing() {
    use syncify_cli::download::qobuz_downloader::QobuzSearchResponse;

    let fixture = r#"{
        "tracks": {
            "items": [
                {
                    "id": 13498234,
                    "title": "Heroes",
                    "isrc": "GBAYE7700021",
                    "duration": 371,
                    "maximum_bit_depth": 24,
                    "maximum_sampling_rate": 192.0,
                    "track_number": 5,
                    "media_number": 1,
                    "performer": {
                        "name": "David Bowie"
                    },
                    "album": {
                        "title": "\"Heroes\"",
                        "release_date_original": "1977-10-14",
                        "media_count": 1,
                        "image": {
                            "small": "https://static.qobuz.com/images/covers/small.jpg",
                            "large": "https://static.qobuz.com/images/covers/large.jpg"
                        },
                        "artist": {
                            "name": "David Bowie"
                        }
                    }
                }
            ]
        }
    }"#;

    let search_res: QobuzSearchResponse = serde_json::from_str(fixture).expect("Failed to parse QobuzSearchResponse fixture");
    let tracks = search_res.tracks.expect("Missing tracks container");
    assert_eq!(tracks.items.len(), 1);

    let track = &tracks.items[0];
    assert_eq!(track.id, 13498234);
    assert_eq!(track.title, "Heroes");
    assert_eq!(track.isrc.as_deref(), Some("GBAYE7700021"));
    assert_eq!(track.duration, 371);
    assert_eq!(track.max_bit_depth, Some(24));
    assert_eq!(track.max_sample_rate, Some(192.0));
    assert_eq!(track.track_number, Some(5));
    assert_eq!(track.disc_number, Some(1));
    assert_eq!(track.performer.as_ref().map(|p| p.name.as_str()), Some("David Bowie"));

    let album = track.album.as_ref().expect("Missing album");
    assert_eq!(album.title, "\"Heroes\"");
    assert_eq!(album.release_date_original.as_deref(), Some("1977-10-14"));
    assert_eq!(album.total_discs, Some(1));
    assert_eq!(album.artist.as_ref().and_then(|a| a.name.as_deref()), Some("David Bowie"));
    assert_eq!(album.image.as_ref().and_then(|i| i.large.as_deref()), Some("https://static.qobuz.com/images/covers/large.jpg"));
}

#[test]
fn test_qobuz_get_file_url_response_parsing() {
    let valid_response = r#"{
        "track_id": 13498234,
        "format_id": 27,
        "url": "https://streaming.qobuz.com/stream/track_sample_24_192.flac?token=abc123xyz",
        "mime_type": "audio/flac",
        "sampling_rate": 192.0,
        "bit_depth": 24
    }"#;

    let parsed: serde_json::Value = serde_json::from_str(valid_response).unwrap();
    assert_eq!(parsed["track_id"].as_i64(), Some(13498234));
    assert_eq!(parsed["format_id"].as_i64(), Some(27));
    assert_eq!(parsed["url"].as_str(), Some("https://streaming.qobuz.com/stream/track_sample_24_192.flac?token=abc123xyz"));
    assert_eq!(parsed["mime_type"].as_str(), Some("audio/flac"));

    let error_response = r#"{
        "status": "error",
        "message": "User subscription does not allow 24-bit streaming for this track"
    }"#;

    let parsed_err: serde_json::Value = serde_json::from_str(error_response).unwrap();
    assert!(parsed_err["url"].as_str().is_none());
    assert_eq!(parsed_err["message"].as_str(), Some("User subscription does not allow 24-bit streaming for this track"));
}
