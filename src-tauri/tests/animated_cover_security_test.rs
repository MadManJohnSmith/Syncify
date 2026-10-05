//! Security Tests for Motion / Animated Cover Stream Ingestion [SEC-015 / TASK-99]
//!
//! Validates:
//! 1. Insecure URL schemes (`file://`, `http://`, `concat:`, `gopher://`, `ftp://`, etc.) are strictly rejected.
//! 2. Unauthorized domains and spoofed hostnames (`evil.com`, `attacker.apple.com.evil.com`, `evilapple.com`) are rejected.
//! 3. Legitimate Apple Music stream URLs (`*.apple.com`, `*.mzstatic.com`) are accepted regardless of case.
//! 4. Embedded credentials (userinfo) in stream URLs are rejected.
//! 5. Loopback URLs are rejected by default and only accepted when explicitly permitted for tests.
//! 6. FFmpeg argument builder enforces `-protocol_whitelist` `"https,tls,tcp"` positioned before `-i`.
//! 7. Tidal fallback (`*.tidal.com`) is accepted and its `videoCover` UUID
//!    parser only emits HTTPS `resources.tidal.com` URLs.

use syncify_tauri_lib::services::animated_cover::{
    build_ffmpeg_animated_cover_args, extract_tidal_video_candidates, tidal_video_cover_url,
    validate_hls_stream_url, validate_hls_stream_url_for_test, validate_hls_stream_url_opts,
    FFMPEG_HLS_PROTOCOL_WHITELIST,
};

#[test]
fn test_rejects_insecure_url_schemes() {
    let insecure_urls = [
        "file:///etc/passwd",
        "file:///C:/Windows/System32/drivers/etc/hosts",
        "http://video-ssl.itunes.apple.com/master.m3u8",
        "http://a1.mzstatic.com/video/master.m3u8",
        "concat:file1|file2",
        "gopher://video-ssl.itunes.apple.com/",
        "ftp://video-ssl.itunes.apple.com/master.m3u8",
        "data:text/plain;base64,SGVsbG8sIFdvcmxkIQ==",
        "tcp://127.0.0.1:8080",
    ];

    for raw_url in insecure_urls {
        let result = validate_hls_stream_url(raw_url);
        assert!(
            result.is_err(),
            "Expected insecure scheme to be rejected: '{}', got {:?}",
            raw_url,
            result
        );
        let err = result.unwrap_err();
        assert!(
            err.contains("Insecure URL scheme") || err.contains("Invalid stream URL format"),
            "Error message should clearly report scheme or parse failure: '{}' for '{}'",
            err,
            raw_url
        );
    }
}

#[test]
fn test_rejects_unauthorized_domains_and_spoofs() {
    let unauthorized_urls = [
        "https://evil.com/master.m3u8",
        "https://attacker.apple.com.evil.com/video/master.m3u8",
        "https://apple.com.attacker.com/video.m3u8",
        "https://evilapple.com/video.m3u8",
        "https://not-apple.com/video.m3u8",
        "https://mzstatic.com.attacker.com/master.m3u8",
        "https://fake-mzstatic.com/master.m3u8",
        "https://192.168.1.100/master.m3u8",
        "https://10.0.0.1/video.m3u8",
        "https://169.254.169.254/latest/meta-data/",
        "https://8.8.8.8/video.m3u8",
    ];

    for raw_url in unauthorized_urls {
        let result = validate_hls_stream_url(raw_url);
        assert!(
            result.is_err(),
            "Expected unauthorized domain to be rejected: '{}', got {:?}",
            raw_url,
            result
        );
        let err = result.unwrap_err();
        assert!(
            err.contains("Unauthorized stream host"),
            "Error message should mention unauthorized stream host: '{}' for '{}'",
            err,
            raw_url
        );
    }
}

#[test]
fn test_accepts_legitimate_apple_music_and_mzstatic_urls() {
    let legitimate_urls = [
        "https://video-ssl.itunes.apple.com/apple-assets-us-std-000001/video/master.m3u8",
        "https://itunes.apple.com/lookup?id=123456",
        "https://music.apple.com/us/album/example-album/123456789",
        "https://amp-api.music.apple.com/v1/catalog/us/albums/123456",
        "https://apple.com/master.m3u8",
        "https://a1.mzstatic.com/us/r1000/063/video/master.m3u8",
        "https://cv-ssl.mzstatic.com/motion/master.m3u8",
        "https://mzstatic.com/master.m3u8",
        // Case-insensitivity tests
        "https://VIDEO-SSL.ITUNES.APPLE.COM/apple-assets/master.m3u8",
        "https://A1.MzStatic.COM/motion/master.m3u8",
        "https://Music.Apple.Com/video.m3u8",
    ];

    for raw_url in legitimate_urls {
        let result = validate_hls_stream_url(raw_url);
        assert!(
            result.is_ok(),
            "Expected legitimate URL to be accepted: '{}', got error: {:?}",
            raw_url,
            result.err()
        );
        let parsed = result.unwrap();
        assert_eq!(parsed.scheme(), "https");
    }
}

#[test]
fn test_rejects_embedded_userinfo_credentials() {
    let credential_urls = [
        "https://attacker:secret@video-ssl.itunes.apple.com/master.m3u8",
        "https://admin@a1.mzstatic.com/video.m3u8",
        "https://user:password@apple.com/master.m3u8",
    ];

    for raw_url in credential_urls {
        let result = validate_hls_stream_url(raw_url);
        assert!(
            result.is_err(),
            "Expected URL with embedded credentials to be rejected: '{}', got {:?}",
            raw_url,
            result
        );
        let err = result.unwrap_err();
        assert!(
            err.contains("user credentials"),
            "Error message should mention user credentials: '{}'",
            err
        );
    }
}

#[test]
fn test_loopback_handling() {
    let loopback_urls = [
        "https://127.0.0.1:8443/master.m3u8",
        "https://localhost:8443/master.m3u8",
        "http://127.0.0.1:8080/master.m3u8",
        "http://localhost:8080/master.m3u8",
    ];

    // 1. By default, production validator MUST reject loopback / localhost
    for raw_url in loopback_urls {
        let prod_result = validate_hls_stream_url(raw_url);
        assert!(
            prod_result.is_err(),
            "Production validation must reject loopback: '{}', got {:?}",
            raw_url,
            prod_result
        );
    }

    // 2. Under test flag / helper, loopback URLs are safely accepted for local mocks
    for raw_url in loopback_urls {
        let test_result = validate_hls_stream_url_for_test(raw_url);
        assert!(
            test_result.is_ok(),
            "Test helper must accept loopback when allowed: '{}', got error: {:?}",
            raw_url,
            test_result.err()
        );

        let opts_result = validate_hls_stream_url_opts(raw_url, true);
        assert!(
            opts_result.is_ok(),
            "validate_hls_stream_url_opts(..., true) must accept loopback: '{}', got error: {:?}",
            raw_url,
            opts_result.err()
        );
    }

    // 3. Non-loopback invalid domains still rejected even if allow_loopback is true
    assert!(validate_hls_stream_url_opts("https://evil.com/master.m3u8", true).is_err());
    assert!(validate_hls_stream_url_opts("file:///etc/passwd", true).is_err());
}

#[test]
fn test_rejects_empty_and_malformed_urls() {
    assert!(validate_hls_stream_url("").is_err());
    assert!(validate_hls_stream_url("   ").is_err());
    assert!(validate_hls_stream_url("not a url at all").is_err());
    assert!(validate_hls_stream_url("://missing-scheme").is_err());
}

#[test]
fn test_ffmpeg_arguments_include_protocol_whitelist_before_input() {
    let test_url = "https://video-ssl.itunes.apple.com/apple-assets/master.m3u8";
    let test_output = "/path/to/target/cover.webp";

    let args = build_ffmpeg_animated_cover_args(test_url, test_output);

    // 1. Check protocol whitelist constant
    assert_eq!(FFMPEG_HLS_PROTOCOL_WHITELIST, "https,tls,tcp");

    // 2. Verify -protocol_whitelist argument and its value are present
    let pw_idx = args.iter().position(|&arg| arg == "-protocol_whitelist");
    assert!(
        pw_idx.is_some(),
        "Arguments must contain '-protocol_whitelist'"
    );
    let pw_idx = pw_idx.unwrap();

    assert_eq!(
        args.get(pw_idx + 1),
        Some(&"https,tls,tcp"),
        "The argument immediately after '-protocol_whitelist' must be 'https,tls,tcp'"
    );

    // 3. Verify -i is present and occurs AFTER -protocol_whitelist
    let input_idx = args.iter().position(|&arg| arg == "-i");
    assert!(input_idx.is_some(), "Arguments must contain '-i'");
    let input_idx = input_idx.unwrap();

    assert!(
        pw_idx < input_idx,
        "'-protocol_whitelist' (index {}) must appear before '-i' (index {}) in FFmpeg args",
        pw_idx,
        input_idx
    );

    assert_eq!(
        args.get(input_idx + 1),
        Some(&test_url),
        "The argument immediately after '-i' must be the stream URL"
    );

    // 4. Verify output path is the final argument
    assert_eq!(args.last(), Some(&test_output));
}

// ═══════════════════════════════════════════════════════
// TIDAL FALLBACK (fuente secundaria)
// ═══════════════════════════════════════════════════════

#[test]
fn test_accepts_legitimate_tidal_motion_cover_urls() {
    let legitimate_urls = [
        "https://resources.tidal.com/videos/d5/e4/8f/de/b33c400aa632b1e667731cd4/640x640.mp4",
        "https://resources.tidal.com/videos/d5/e4/8f/de/b33c400aa632b1e667731cd4/1280x1280.mp4",
        "https://resources.tidal.com/videos/d5/e4/8f/de/b33c400aa632b1e667731cd4/origin.mp4",
        // Case-insensitivity
        "https://RESOURCES.TIDAL.COM/videos/d5/e4/8f/de/b33c400aa632b1e667731cd4/640x640.mp4",
    ];

    for raw_url in legitimate_urls {
        let result = validate_hls_stream_url(raw_url);
        assert!(
            result.is_ok(),
            "Expected legitimate Tidal URL to be accepted: '{}', got error: {:?}",
            raw_url,
            result.err()
        );
    }
}

#[test]
fn test_rejects_tidal_host_spoofs() {
    let unauthorized_urls = [
        "https://resources.tidal.com.attacker.com/videos/x/640x640.mp4",
        "https://resources.tidal.com.evil.com/videos/x/640x640.mp4",
        "https://tideal.com/videos/x/640x640.mp4",
        "http://resources.tidal.com/videos/x/640x640.mp4",
    ];

    for raw_url in unauthorized_urls {
        let result = validate_hls_stream_url(raw_url);
        assert!(
            result.is_err(),
            "Expected spoofed/invalid Tidal host to be rejected: '{}', got {:?}",
            raw_url,
            result
        );
    }
}

#[test]
fn test_tidal_video_cover_url_builds_verified_pattern() {
    // UUID real observado en la API (Certified Lover Boy, id 196486028). El
    // patrón verificado (sone, vivi-music y Levyra coinciden): cada guion del
    // UUID se convierte en barra de ruta.
    let uuid = "d5e48fde-b33c-400a-a632-b1e667731cd4";
    assert_eq!(
        tidal_video_cover_url(uuid, "640x640.mp4"),
        Some(
            "https://resources.tidal.com/videos/d5e48fde/b33c/400a/a632/b1e667731cd4/640x640.mp4"
                .to_string()
        )
    );

    // Un valor que no sea UUID válido no construye URL alguna.
    assert_eq!(tidal_video_cover_url("not-a-uuid", "640x640.mp4"), None);
    assert_eq!(tidal_video_cover_url("", "640x640.mp4"), None);
    assert_eq!(
        tidal_video_cover_url("zzzzzzzz-1111-2222-3333-444444444444", "640x640.mp4"),
        None,
        "hex no válido debe rechazarse"
    );
}

#[test]
fn test_extract_tidal_video_candidates_prefers_videocover_uuid() {
    let album = serde_json::json!({
        "id": 196486028,
        "title": "Certified Lover Boy",
        "videoCover": "d5e48fde-b33c-400a-a632-b1e667731cd4"
    });

    let candidates = extract_tidal_video_candidates(&album);
    assert_eq!(
        candidates.len(),
        3,
        "un UUID expande a los 3 ficheros: {candidates:?}"
    );
    assert!(
        candidates[0].ends_with("/640x640.mp4"),
        "orden de preferencia: {candidates:?}"
    );
    assert!(candidates[1].ends_with("/1280x1280.mp4"));
    assert!(candidates[2].ends_with("/origin.mp4"));
}

#[test]
fn test_extract_tidal_video_candidates_handles_videothumbnail_shapes() {
    // Objeto con claves por tamaño: se aceptan URLs https de tidal.com,
    // se descartan esquemas inseguros y hosts ajenos.
    let album = serde_json::json!({
        "videoThumbnail": {
            "large": "https://resources.tidal.com/videos/aa/bb/cc/dd/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee/origin.mp4",
            "small": "http://evil.com/steal.mp4",
            "medium": "https://attacker.com/steal.mp4"
        }
    });
    let candidates = extract_tidal_video_candidates(&album);
    assert_eq!(
        candidates,
        vec!["https://resources.tidal.com/videos/aa/bb/cc/dd/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee/origin.mp4".to_string()],
        "solo la URL https de tidal.com debe salir: {candidates:?}"
    );

    // String plano con URL de tidal.com también se acepta.
    let album_string = serde_json::json!({
        "videoThumbnail": "https://resources.tidal.com/videos/aa/bb/cc/dd/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee/640x640.mp4"
    });
    assert_eq!(extract_tidal_video_candidates(&album_string).len(), 1);

    // Un álbum sin campos de video no produce candidatos.
    let plain = serde_json::json!({ "title": "Renaissance", "videoCover": null });
    assert!(extract_tidal_video_candidates(&plain).is_empty());
}
