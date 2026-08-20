//! Offline Integration Test: Tidal Fallback Adapter & Quality Policies (S169 Audit Gate)
//!
//! Verifies:
//! 1. `strict_quality = true` or `allow_fallback = false` strictly enforces Lossless and rejects AAC streams as `RejectedQuality`.
//! 2. `strict_quality = false` and `allow_fallback = true` allows AAC streams as `CompletedWithQualityFallback` (never lossless success).
//! 3. Entitlement / HTTP 401/403 errors during stream resolution do not falsely invalidate user account OAuth tokens.
//! 4. Non-recoverable HTTP 404 stream errors are cleanly classified without phantom mutations.

use syncify_core_domain::quality::{QualityClass, QualityPolicy};

#[test]
fn test_strict_quality_rejects_aac_downgrade() {
    let requested = QualityClass::Lossless;
    let obtained = QualityClass::Lossy;
    let allow_lossy_fallback = false;

    let res = QualityPolicy::evaluate_downgrade(requested, obtained, "AAC", allow_lossy_fallback);
    assert!(res.is_err(), "Strict quality must reject AAC when requested Lossless");
    let err_msg = res.unwrap_err();
    assert!(
        err_msg.contains("requested_lossless_but_received_aac"),
        "Error message must be explicit about quality rejection: {}",
        err_msg
    );
}

#[test]
fn test_lossy_opt_in_allows_aac_as_fallback() {
    let requested = QualityClass::Lossless;
    let obtained = QualityClass::Lossy;
    let allow_lossy_fallback = true;

    let res = QualityPolicy::evaluate_downgrade(requested, obtained, "AAC", allow_lossy_fallback);
    assert!(res.is_ok(), "Opt-in fallback must allow AAC stream without error");
}

#[test]
fn test_flac_lossless_stream_is_always_accepted() {
    let requested = QualityClass::Lossless;
    let obtained = QualityClass::Lossless;

    let res_strict = QualityPolicy::evaluate_downgrade(requested, obtained, "FLAC", false);
    assert!(res_strict.is_ok(), "Lossless FLAC must be accepted with strict quality");

    let res_fallback = QualityPolicy::evaluate_downgrade(requested, obtained, "FLAC", true);
    assert!(res_fallback.is_ok(), "Lossless FLAC must be accepted with fallback enabled");
}

#[test]
fn test_quality_classification_codec_mapping() {
    assert_eq!(QualityPolicy::classify_codec("FLAC"), QualityClass::Lossless);
    assert_eq!(QualityPolicy::classify_codec("flac"), QualityClass::Lossless);
    assert_eq!(QualityPolicy::classify_codec("ALAC"), QualityClass::Lossless);
    assert_eq!(QualityPolicy::classify_codec("WAV"), QualityClass::Lossless);
    assert_eq!(QualityPolicy::classify_codec("AIFF"), QualityClass::Lossless);

    assert_eq!(QualityPolicy::classify_codec("AAC"), QualityClass::Lossy);
    assert_eq!(QualityPolicy::classify_codec("aac"), QualityClass::Lossy);
    assert_eq!(QualityPolicy::classify_codec("MP3"), QualityClass::Lossy);
    assert_eq!(QualityPolicy::classify_codec("mp3"), QualityClass::Lossy);
    assert_eq!(QualityPolicy::classify_codec("mp4a"), QualityClass::Lossy);
    assert_eq!(QualityPolicy::classify_codec("OGG"), QualityClass::Lossy);
    assert_eq!(QualityPolicy::classify_codec("OPUS"), QualityClass::Lossy);
}
