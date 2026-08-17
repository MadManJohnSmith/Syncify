use syncify_tauri_lib::services::animated_cover::{redact_stream_url, AnimatedCoverStatus};

#[test]
fn test_log_redaction_hls_signed_url() {
    let signed_url = "https://cv-mr-itunes.apple.com/us/r1000/000/Purple116/v4/37/eb/8c/37eb8c6f-6821-3ce1-2d7f-2ec550a2491a/video.m3u8?token=secret_jwt_payload.signature_abc123&Expires=1755500000&Key-Pair-Id=APKAEXAMPLEKEY&Signature=secret_sig_here";

    let redacted = redact_stream_url(signed_url);

    // 1. Assert host and high-level resource type are preserved for diagnostics
    assert!(redacted.contains("cv-mr-itunes.apple.com"), "Host must be preserved in redacted output");
    assert!(redacted.contains("HLS playlist (.m3u8)"), "Resource type descriptor must be preserved");
    assert!(redacted.contains("[id_hash:"), "Truncated hash identifier must be present");

    // 2. Assert sensitive query strings, tokens, signatures, and cookies are NOT present
    assert!(!redacted.contains("secret_jwt_payload"), "Tokens must be stripped");
    assert!(!redacted.contains("signature_abc123"), "Signatures must be stripped");
    assert!(!redacted.contains("Expires=1755500000"), "Expiry query params must be stripped");
    assert!(!redacted.contains("Key-Pair-Id"), "Key pair credentials must be stripped");
    assert!(!redacted.contains("secret_sig_here"), "Signature credentials must be stripped");
    assert!(!redacted.contains('?'), "Query parameter separator '?' must not be present in output");
}

#[test]
fn test_log_redaction_js_bundle_and_other_streams() {
    let js_url = "https://music.apple.com/assets/index-legacy-abc12345.js?v=9999&auth=private_key";
    let redacted_js = redact_stream_url(js_url);
    assert!(redacted_js.contains("music.apple.com"));
    assert!(redacted_js.contains("JavaScript bundle (.js)"));
    assert!(!redacted_js.contains("private_key"));

    let dash_url = "https://streaming.service.com/manifest.mpd?session=xyz987654";
    let redacted_dash = redact_stream_url(dash_url);
    assert!(redacted_dash.contains("streaming.service.com"));
    assert!(redacted_dash.contains("DASH manifest (.mpd)"));
    assert!(!redacted_dash.contains("xyz987654"));

    let invalid_url = "not a valid url here";
    let redacted_invalid = redact_stream_url(invalid_url);
    assert_eq!(redacted_invalid, "[REDACTED_STREAM_URL]");
}

#[test]
fn test_animated_cover_status_classification_preserved() {
    let not_found = AnimatedCoverStatus::NotFound;
    assert_eq!(not_found, AnimatedCoverStatus::NotFound);

    let auth_error = AnimatedCoverStatus::SourceUnavailable("Could not extract Apple Music developer token from web player".to_string());
    match auth_error {
        AnimatedCoverStatus::SourceUnavailable(msg) => assert!(msg.contains("developer token")),
        _ => panic!("Expected SourceUnavailable"),
    }

    let failed = AnimatedCoverStatus::Failed("ffmpeg exit error: corrupt file".to_string());
    match failed {
        AnimatedCoverStatus::Failed(msg) => assert!(msg.contains("corrupt file")),
        _ => panic!("Expected Failed"),
    }
}
