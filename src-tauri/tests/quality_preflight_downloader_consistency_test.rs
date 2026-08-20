//! Preflight & Downloader Consistency Test (100% Offline)
//!
//! Asserts that Preflight Queue Evaluation and Downloader Stream Resolution
//! adhere to the exact same Quality Decision engine and yield consistent decisions.

use syncify_core_domain::quality::{QualityDecisionKind, QualityPolicy};

#[test]
fn test_strict_quality_rejection_consistency_between_preflight_and_downloader() {
    let requested_quality = "lossless";
    let candidate_quality = "high"; // lossy / AAC
    let candidate_codec = "AAC";
    let origin_service = "tidal";
    let target_service = "tidal";
    let strict_quality = true;
    let allow_lossy_fallback = false;

    // 1. Preflight Evaluation
    let preflight = QualityPolicy::evaluate_preflight(
        requested_quality,
        Some(candidate_quality),
        Some(candidate_codec),
        None,
        origin_service,
        target_service,
        strict_quality,
        allow_lossy_fallback,
    );

    // 2. Downloader / Stream Resolution Evaluation
    let downloader = QualityPolicy::evaluate_stream_resolution(
        requested_quality,
        candidate_quality,
        candidate_codec,
        16,
        44100.0,
        origin_service,
        target_service,
        strict_quality,
        allow_lossy_fallback,
    );

    // Both must be RejectedQuality
    assert_eq!(preflight.decision, QualityDecisionKind::RejectedQuality);
    assert_eq!(downloader.decision, QualityDecisionKind::RejectedQuality);

    // Both must agree on fallback flags
    assert_eq!(preflight.quality_fallback_used, downloader.quality_fallback_used);
    assert_eq!(preflight.provider_fallback_used, downloader.provider_fallback_used);
    assert_eq!(preflight.strict_quality, downloader.strict_quality);
    assert_eq!(preflight.allow_lossy_fallback, downloader.allow_lossy_fallback);

    // Both must agree that this is terminal (non-retryable)
    assert!(!preflight.retryable);
    assert!(!downloader.retryable);
}

#[test]
fn test_opt_in_fallback_consistency_between_preflight_and_downloader() {
    let requested_quality = "lossless";
    let candidate_quality = "high"; // lossy / AAC
    let candidate_codec = "AAC";
    let origin_service = "tidal";
    let target_service = "tidal";
    let strict_quality = false;
    let allow_lossy_fallback = true;

    // 1. Preflight Evaluation
    let preflight = QualityPolicy::evaluate_preflight(
        requested_quality,
        Some(candidate_quality),
        Some(candidate_codec),
        None,
        origin_service,
        target_service,
        strict_quality,
        allow_lossy_fallback,
    );

    // 2. Downloader / Stream Resolution Evaluation
    let downloader = QualityPolicy::evaluate_stream_resolution(
        requested_quality,
        candidate_quality,
        candidate_codec,
        16,
        44100.0,
        origin_service,
        target_service,
        strict_quality,
        allow_lossy_fallback,
    );

    // Preflight is ReadyQualityFallback, Downloader is CompletedWithQualityFallback
    assert_eq!(preflight.decision, QualityDecisionKind::ReadyQualityFallback);
    assert_eq!(downloader.decision, QualityDecisionKind::CompletedWithQualityFallback);

    // Both must have quality_fallback_used = true
    assert!(preflight.quality_fallback_used);
    assert!(downloader.quality_fallback_used);

    // Both effective formats must be "aac" (never flac)
    assert_eq!(preflight.effective_format, "aac");
    assert_eq!(downloader.effective_format, "aac");
}

#[test]
fn test_exact_hires_consistency_between_preflight_and_downloader() {
    let requested_quality = "hires";
    let candidate_quality = "hires";
    let candidate_codec = "FLAC";
    let origin_service = "qobuz";
    let target_service = "qobuz";
    let strict_quality = true;
    let allow_lossy_fallback = false;

    // 1. Preflight Evaluation
    let preflight = QualityPolicy::evaluate_preflight(
        requested_quality,
        Some(candidate_quality),
        Some(candidate_codec),
        Some(24),
        origin_service,
        target_service,
        strict_quality,
        allow_lossy_fallback,
    );

    // 2. Downloader / Stream Resolution Evaluation
    let downloader = QualityPolicy::evaluate_stream_resolution(
        requested_quality,
        candidate_quality,
        candidate_codec,
        24,
        96000.0,
        origin_service,
        target_service,
        strict_quality,
        allow_lossy_fallback,
    );

    assert_eq!(preflight.decision, QualityDecisionKind::ReadyExactQuality);
    assert_eq!(downloader.decision, QualityDecisionKind::CompletedExactQuality);
    assert_eq!(preflight.effective_format, "flac");
    assert_eq!(downloader.effective_format, "flac");
    assert!(!preflight.quality_fallback_used);
    assert!(!downloader.quality_fallback_used);
}
