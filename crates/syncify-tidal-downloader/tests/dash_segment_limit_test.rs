use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use syncify_tidal_downloader::{
    parse_tidal_playback_manifest, TidalDownloader, MAX_DASH_SEGMENTS,
};
use tempfile::tempdir;

fn make_dash_json(segment_timeline_xml: &str) -> String {
    let dash_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<MPD xmlns="urn:mpeg:dash:schema:mpd:2011" minBufferTime="PT2.0S" type="static">
  <Period>
    <AdaptationSet mimeType="audio/mp4" codecs="flac" lang="en">
      <SegmentTemplate timescale="96000" initialization="https://sp-pr-cf.audio.tidal.com/init.mp4" media="https://sp-pr-cf.audio.tidal.com/seg_$Number$.mp4">
        <SegmentTimeline>
          {}
        </SegmentTimeline>
      </SegmentTemplate>
      <Representation id="1" bandwidth="2800000" audioSamplingRate="96000" />
    </AdaptationSet>
  </Period>
</MPD>"#,
        segment_timeline_xml
    );
    let b64_manifest = BASE64.encode(dash_xml);
    format!(
        r#"{{"trackId":80654035,"audioQuality":"HI_RES_LOSSLESS","manifestMimeType":"application/dash+xml","manifest":"{}"}}"#,
        b64_manifest
    )
}

#[test]
fn test_max_dash_segments_constant_value() {
    assert_eq!(
        MAX_DASH_SEGMENTS, 500,
        "MAX_DASH_SEGMENTS must be exactly 500 as per SEC-023 invariant"
    );
}

#[test]
fn test_dash_manifest_large_repeat_count_fails_immediately() {
    // Malicious manifest with r="999999" (1,000,000 segments)
    let json_payload = make_dash_json(r#"<S d="96000" r="999999" />"#);
    let result = parse_tidal_playback_manifest(&json_payload, "HI_RES_LOSSLESS");

    assert!(result.is_err(), "Manifest with r=999999 must fail");
    let err_msg = result.err().unwrap().to_string();
    assert!(
        err_msg.contains("ManifestSegmentLimitExceeded"),
        "Error message must contain 'ManifestSegmentLimitExceeded', got: {}",
        err_msg
    );
    assert!(
        err_msg.contains("safety limit of 500"),
        "Error message must cite the safety limit of 500, got: {}",
        err_msg
    );
}

#[test]
fn test_dash_manifest_overflow_u32_saturating_fails_safely() {
    // Arithmetic overflow attempt with r="4294967295" (u32::MAX)
    let json_payload = make_dash_json(r#"<S d="96000" r="4294967295" />"#);
    let result = parse_tidal_playback_manifest(&json_payload, "HI_RES_LOSSLESS");

    assert!(result.is_err(), "Manifest with r=u32::MAX must fail");
    let err_msg = result.err().unwrap().to_string();
    assert!(
        err_msg.contains("ManifestSegmentLimitExceeded"),
        "Error must contain ManifestSegmentLimitExceeded without arithmetic panic, got: {}",
        err_msg
    );
}

#[test]
fn test_dash_manifest_multi_element_cumulative_overflow_fails() {
    // Cumulative overflow across multiple <S> tags: 251 + 251 = 502 segments (> 500)
    let json_payload = make_dash_json(r#"
        <S d="96000" r="250" />
        <S d="96000" r="250" />
    "#);
    let result = parse_tidal_playback_manifest(&json_payload, "HI_RES_LOSSLESS");

    assert!(result.is_err(), "Manifest with 502 total segments must fail");
    let err_msg = result.err().unwrap().to_string();
    assert!(
        err_msg.contains("ManifestSegmentLimitExceeded"),
        "Cumulative segments exceeding limit must fail with ManifestSegmentLimitExceeded, got: {}",
        err_msg
    );
}

#[test]
fn test_dash_manifest_boundary_501_fails() {
    // r="500" produces 501 segments -> exceeds 500
    let json_payload = make_dash_json(r#"<S d="96000" r="500" />"#);
    let result = parse_tidal_playback_manifest(&json_payload, "HI_RES_LOSSLESS");

    assert!(result.is_err(), "Manifest with 501 segments must be rejected");
    let err_msg = result.err().unwrap().to_string();
    assert!(
        err_msg.contains("ManifestSegmentLimitExceeded"),
        "Boundary + 1 must fail with ManifestSegmentLimitExceeded, got: {}",
        err_msg
    );
}

#[test]
fn test_dash_manifest_legitimate_within_limit_succeeds() {
    // Standard track with 11 segments (r="10")
    let json_payload = make_dash_json(r#"<S d="96000" r="10" />"#);
    let result = parse_tidal_playback_manifest(&json_payload, "HI_RES_LOSSLESS");

    assert!(result.is_ok(), "Manifest with 11 segments must succeed");
    let parsed = result.unwrap();
    assert!(parsed.is_dash);
    assert!(
        parsed.stream_url.ends_with("|11"),
        "stream_url must end with |11, got: {}",
        parsed.stream_url
    );
}

#[test]
fn test_dash_manifest_exact_boundary_500_succeeds() {
    // Exactly 500 segments: r="499" -> 499 + 1 = 500
    let json_payload = make_dash_json(r#"<S d="96000" r="499" />"#);
    let result = parse_tidal_playback_manifest(&json_payload, "HI_RES_LOSSLESS");

    assert!(result.is_ok(), "Manifest with exactly 500 segments must succeed");
    let parsed = result.unwrap();
    assert!(parsed.is_dash);
    assert!(
        parsed.stream_url.ends_with("|500"),
        "stream_url must end with |500, got: {}",
        parsed.stream_url
    );
}

#[tokio::test]
async fn test_download_audio_payload_rejects_excessive_segments_without_io_or_network() {
    let downloader = TidalDownloader::new();
    let temp_dir = tempdir().expect("Create temp directory");
    let output_path = temp_dir.path().join("subagent_test_track.flac");
    let temp_file_path = output_path.with_extension("stream.tmp");

    // Attack payload declaring 10000 segments
    let attack_url = "DASH_MANIFEST|http://127.0.0.1:9/init.mp4|http://127.0.0.1:9/seg_$Number$.mp4|10000";
    let res = downloader
        .download_audio_payload_with_progress(attack_url, &output_path, |_, _, _| {})
        .await;

    assert!(res.is_err(), "Download with 10000 segments must be rejected");
    let err_msg = res.err().unwrap().to_string();
    assert!(
        err_msg.contains("ManifestSegmentLimitExceeded"),
        "Expected ManifestSegmentLimitExceeded, got: {}",
        err_msg
    );

    // Defense in depth: Verify no disk writes occurred
    assert!(
        !output_path.exists(),
        "Output file must NOT exist on disk when segment limit is exceeded"
    );
    assert!(
        !temp_file_path.exists(),
        "Temp stream file must NOT exist on disk when segment limit is exceeded"
    );

    // Also test boundary 501 segments
    let boundary_attack_url = "DASH_MANIFEST|http://127.0.0.1:9/init.mp4|http://127.0.0.1:9/seg_$Number$.mp4|501";
    let res_boundary = downloader
        .download_audio_payload_with_progress(boundary_attack_url, &output_path, |_, _, _| {})
        .await;

    assert!(res_boundary.is_err(), "Download with 501 segments must be rejected");
    let boundary_err = res_boundary.err().unwrap().to_string();
    assert!(
        boundary_err.contains("ManifestSegmentLimitExceeded"),
        "Expected ManifestSegmentLimitExceeded for 501 segments, got: {}",
        boundary_err
    );
    assert!(
        !output_path.exists(),
        "Output file must still NOT exist after boundary attack attempt"
    );
    assert!(
        !temp_file_path.exists(),
        "Temp file must still NOT exist after boundary attack attempt"
    );
}
