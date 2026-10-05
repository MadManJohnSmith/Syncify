//! R1 — a lossless request must not settle for the first lossy answer.
//!
//! The official endpoints do not degrade together: `playbackinfopostpaywall`
//! answers 2xx with whatever the calling client context may serve (an AAC
//! manifest is a normal success for it), while `streamUrl` / `url` can still
//! hand out FLAC for the same track. Settling on the first response is what made
//! a FLAC request land as `.m4a`.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use syncify_core_domain::quality::{QualityClass, StreamSourceType};
use syncify_tidal_downloader::{
    parse_tidal_playback_manifest, prefer_best_resolution, resolve_tidal_quality_request,
    tidal_quality_param_for_endpoint, TidalStreamResolution,
};

fn resolution(codec: &str, class: QualityClass, url: &str) -> TidalStreamResolution {
    let lossless = class == QualityClass::Lossless;
    TidalStreamResolution {
        url: url.to_string(),
        source: StreamSourceType::TidalOfficial,
        source_name: "Tidal Official API".to_string(),
        requested_quality: "24-192".to_string(),
        obtained_quality: if lossless { "24-192" } else { "320" }.to_string(),
        format_id_requested: "HI_RES_LOSSLESS".to_string(),
        format_id_obtained: if lossless { "HI_RES_LOSSLESS" } else { "HIGH" }.to_string(),
        quality_class_requested: QualityClass::Lossless,
        quality_class_obtained: class,
        codec: codec.to_string(),
        container: if lossless { "FLAC" } else { "M4A" }.to_string(),
        extension: if lossless { "flac" } else { "m4a" }.to_string(),
        bit_depth: if lossless { 24 } else { 16 },
        sample_rate: if lossless { 96000.0 } else { 44100.0 },
        is_fallback: !lossless,
    }
}

/// The lossy answer that arrives first must not win over the lossless one behind it.
#[test]
fn lossless_candidate_replaces_a_lossy_incumbent() {
    let lossy = resolution("AAC", QualityClass::Lossy, "https://cdn/a.m4a");
    let lossless = resolution("FLAC", QualityClass::Lossless, "https://cdn/a.flac");

    let winner = prefer_best_resolution(Some(lossy), lossless);

    assert_eq!(winner.extension, "flac");
    assert_eq!(winner.codec, "FLAC");
    assert_eq!(winner.quality_class_obtained, QualityClass::Lossless);
}

/// Same class on both sides: the earlier answer keeps the tie, so the endpoint
/// order still decides and the resolution stays deterministic.
#[test]
fn incumbent_keeps_the_tie_within_the_same_quality_class() {
    let first = resolution("FLAC", QualityClass::Lossless, "https://cdn/first.flac");
    let second = resolution("FLAC", QualityClass::Lossless, "https://cdn/second.flac");

    let winner = prefer_best_resolution(Some(first), second);

    assert_eq!(winner.url, "https://cdn/first.flac");
}

/// A lossless incumbent is never downgraded by a later lossy proxy answer.
#[test]
fn lossy_candidate_never_replaces_a_lossless_incumbent() {
    let lossless = resolution("FLAC", QualityClass::Lossless, "https://cdn/a.flac");
    let lossy = resolution("AAC", QualityClass::Lossy, "https://cdn/a.m4a");

    let winner = prefer_best_resolution(Some(lossless), lossy);

    assert_eq!(winner.extension, "flac");
}

/// Nothing to compare against yet: the only answer on offer wins.
#[test]
fn first_answer_wins_when_there_is_no_incumbent() {
    let lossless = resolution("FLAC", QualityClass::Lossless, "https://cdn/a.flac");

    let winner = prefer_best_resolution(None, lossless);

    assert_eq!(winner.extension, "flac");
}

/// The extension follows what the manifest actually declares — the parser is not
/// allowed to upgrade an AAC manifest to FLAC just because lossless was requested.
#[test]
fn extension_follows_the_manifest_and_not_the_request() {
    let flac_manifest = serde_json::json!({
        "audioQuality": "LOSSLESS",
        "manifest": BASE64.encode(
            serde_json::json!({
                "mimeType": "audio/flac",
                "codecs": "flac",
                "urls": ["https://cdn/track.flac"]
            })
            .to_string()
        )
    })
    .to_string();

    let aac_manifest = serde_json::json!({
        "audioQuality": "HIGH",
        "manifest": BASE64.encode(
            serde_json::json!({
                "mimeType": "audio/mp4",
                "codecs": "mp4a.40.2",
                "urls": ["https://cdn/track.m4a"]
            })
            .to_string()
        )
    })
    .to_string();

    let flac = parse_tidal_playback_manifest(&flac_manifest, "HI_RES_LOSSLESS").unwrap();
    assert_eq!(flac.extension, "flac");
    assert_eq!(flac.quality_class, QualityClass::Lossless);

    let aac = parse_tidal_playback_manifest(&aac_manifest, "HI_RES_LOSSLESS").unwrap();
    assert_eq!(aac.extension, "m4a");
    assert_eq!(aac.quality_class, QualityClass::Lossy);
}

/// The labels the download queue persists must never resolve to a lossy tier:
/// that is how a FLAC request silently becomes an AAC one before it leaves here.
#[test]
fn queue_quality_labels_never_resolve_to_a_lossy_tier() {
    for label in [
        "hires", "HIRES", "any", "lossless", "LOSSLESS", "flac", "FLAC", "16-44", "24-192", "",
    ] {
        let (_, param, class) = resolve_tidal_quality_request(Some(label));
        assert_eq!(
            class,
            QualityClass::Lossless,
            "label {:?} lost its lossless intent (param {})",
            label,
            param
        );
    }

    let (_, param, class) = resolve_tidal_quality_request(Some("high"));
    assert_eq!(class, QualityClass::Lossy);
    assert_eq!(param, "HIGH");

    let (_, param, class) = resolve_tidal_quality_request(Some("320"));
    assert_eq!(class, QualityClass::Lossy);
    assert_eq!(param, "HIGH");
}

/// The legacy endpoints only know the classic enum; sending HI_RES_LOSSLESS there
/// makes Tidal answer with its default (AAC) tier instead of erroring.
#[test]
fn legacy_endpoints_receive_the_classic_hi_res_enum() {
    assert_eq!(
        tidal_quality_param_for_endpoint("streamUrl", "HI_RES_LOSSLESS"),
        "HI_RES"
    );
    assert_eq!(
        tidal_quality_param_for_endpoint("url", "HI_RES_LOSSLESS"),
        "HI_RES"
    );
    assert_eq!(
        tidal_quality_param_for_endpoint("playbackinfopostpaywall", "HI_RES_LOSSLESS"),
        "HI_RES_LOSSLESS"
    );
    // A capped request must stay capped on every endpoint.
    assert_eq!(
        tidal_quality_param_for_endpoint("streamUrl", "LOSSLESS"),
        "LOSSLESS"
    );
}
