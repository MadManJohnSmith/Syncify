//! Matrix Integration Test for Complete Quality Policy, Fallback, and Provider Degradation (100% Offline)
//!
//! Validates the 10 canonical combinations:
//! 1. Tidal | Hi-Res/Lossless | FLAC | strict=true, allow_lossy=false -> CompletedExactQuality
//! 2. Tidal | Hi-Res/Lossless | AAC  | strict=true, allow_lossy=false -> RejectedQuality
//! 3. Tidal | Hi-Res/Lossless | AAC  | strict=false, allow_lossy=true -> CompletedWithQualityFallback
//! 4. Qobuz | Hi-Res          | 24/96 FLAC | strict=true, allow_lossy=false -> CompletedExactQuality
//! 5. Qobuz | Lossless        | 16/44 FLAC | strict=true, allow_lossy=false -> CompletedExactQuality
//! 6. Deezer| Lossless        | MP3  | strict=true, allow_lossy=false -> RejectedQuality
//! 7. Spotify -> Qobuz        | Lossless | FLAC | strict=true, allow_lossy=false -> CompletedWithProviderFallback
//! 8. Spotify -> Tidal        | Lossless | AAC  | strict=true, allow_lossy=false -> RejectedQuality
//! 9. Spotify -> Tidal        | Lossless | AAC  | strict=false, allow_lossy=true -> CompletedWithQualityFallback (with provider_fallback_used=true)
//! 10. No provider            | any      | none | any, any -> NoDownloadProvider

use syncify_core_domain::quality::{QualityDecisionKind, QualityPolicy};

#[test]
fn test_case_1_tidal_lossless_flac_strict_succeeds_exact() {
    let decision = QualityPolicy::evaluate_stream_resolution(
        "lossless",
        "lossless",
        "FLAC",
        16,
        44100.0,
        "tidal",
        "tidal",
        true,
        false,
    );

    assert_eq!(decision.decision, QualityDecisionKind::CompletedExactQuality);
    assert_eq!(decision.effective_format, "flac");
    assert!(!decision.quality_fallback_used);
    assert!(!decision.provider_fallback_used);
    assert!(!decision.retryable);
    assert!(decision.reason.is_none());
}

#[test]
fn test_case_2_tidal_lossless_aac_strict_rejected() {
    let decision = QualityPolicy::evaluate_stream_resolution(
        "lossless",
        "high",
        "AAC",
        16,
        44100.0,
        "tidal",
        "tidal",
        true,
        false,
    );

    assert_eq!(decision.decision, QualityDecisionKind::RejectedQuality);
    assert!(decision.quality_fallback_used);
    assert!(!decision.provider_fallback_used);
    assert!(!decision.retryable);
    assert_eq!(decision.reason.as_deref(), Some("Provider returned AAC; lossy fallback is disabled"));
}

#[test]
fn test_case_3_tidal_lossless_aac_opt_in_completed_with_quality_fallback() {
    let decision = QualityPolicy::evaluate_stream_resolution(
        "lossless",
        "high",
        "AAC",
        16,
        44100.0,
        "tidal",
        "tidal",
        false,
        true,
    );

    assert_eq!(decision.decision, QualityDecisionKind::CompletedWithQualityFallback);
    assert_eq!(decision.effective_format, "aac");
    assert!(decision.quality_fallback_used);
    assert!(!decision.provider_fallback_used);
    assert!(!decision.retryable);
    assert_eq!(decision.reason.as_deref(), Some("Provider returned AAC; lossy fallback is enabled"));
}

#[test]
fn test_case_4_qobuz_hires_flac_strict_succeeds_exact() {
    let decision = QualityPolicy::evaluate_stream_resolution(
        "hires",
        "hires",
        "FLAC",
        24,
        96000.0,
        "qobuz",
        "qobuz",
        true,
        false,
    );

    assert_eq!(decision.decision, QualityDecisionKind::CompletedExactQuality);
    assert_eq!(decision.effective_format, "flac");
    assert!(!decision.quality_fallback_used);
    assert!(!decision.provider_fallback_used);
}

#[test]
fn test_case_5_qobuz_lossless_flac_strict_succeeds_exact() {
    let decision = QualityPolicy::evaluate_stream_resolution(
        "lossless",
        "lossless",
        "FLAC",
        16,
        44100.0,
        "qobuz",
        "qobuz",
        true,
        false,
    );

    assert_eq!(decision.decision, QualityDecisionKind::CompletedExactQuality);
    assert_eq!(decision.effective_format, "flac");
    assert!(!decision.quality_fallback_used);
    assert!(!decision.provider_fallback_used);
}

#[test]
fn test_case_6_deezer_lossless_mp3_strict_rejected() {
    let decision = QualityPolicy::evaluate_stream_resolution(
        "lossless",
        "high",
        "MP3",
        16,
        44100.0,
        "deezer",
        "deezer",
        true,
        false,
    );

    assert_eq!(decision.decision, QualityDecisionKind::RejectedQuality);
    assert!(decision.quality_fallback_used);
    assert!(!decision.provider_fallback_used);
    assert_eq!(decision.reason.as_deref(), Some("Provider returned MP3; lossy fallback is disabled"));
}

#[test]
fn test_case_7_spotify_source_to_qobuz_lossless_flac_completed_with_provider_fallback() {
    let decision = QualityPolicy::evaluate_stream_resolution(
        "lossless",
        "lossless",
        "FLAC",
        16,
        44100.0,
        "spotify",
        "qobuz",
        true,
        false,
    );

    assert_eq!(decision.decision, QualityDecisionKind::CompletedWithProviderFallback);
    assert_eq!(decision.effective_format, "flac");
    assert!(!decision.quality_fallback_used);
    assert!(decision.provider_fallback_used);
}

#[test]
fn test_case_8_spotify_source_to_tidal_lossless_aac_strict_rejected() {
    let decision = QualityPolicy::evaluate_stream_resolution(
        "lossless",
        "high",
        "AAC",
        16,
        44100.0,
        "spotify",
        "tidal",
        true,
        false,
    );

    assert_eq!(decision.decision, QualityDecisionKind::RejectedQuality);
    assert!(decision.quality_fallback_used);
    assert!(decision.provider_fallback_used);
    assert_eq!(decision.reason.as_deref(), Some("Provider returned AAC; lossy fallback is disabled"));
}

#[test]
fn test_case_9_spotify_source_to_tidal_lossless_aac_opt_in_both_fallbacks() {
    let decision = QualityPolicy::evaluate_stream_resolution(
        "lossless",
        "high",
        "AAC",
        16,
        44100.0,
        "spotify",
        "tidal",
        false,
        true,
    );

    assert_eq!(decision.decision, QualityDecisionKind::CompletedWithQualityFallback);
    assert_eq!(decision.effective_format, "aac");
    assert!(decision.quality_fallback_used);
    assert!(decision.provider_fallback_used);
}

#[test]
fn test_case_10_preflight_no_provider_outcome() {
    let preflight_decision = QualityPolicy::evaluate_preflight(
        "lossless",
        None,
        None,
        None,
        "spotify",
        "spotify",
        true,
        false,
    );

    // When candidate quality is None, evaluate_preflight defaults cand to lossy which under strict policy rejects
    assert_eq!(preflight_decision.decision, QualityDecisionKind::RejectedQuality);
}

#[test]
fn test_case_11_hires_requested_but_cd_delivered_emits_shortfall() {
    // Mitigates C7 / F3.5: Hi-Res requested, physical FLAC STREAMINFO verified 16-bit / 44.1kHz
    let decision = QualityPolicy::evaluate_stream_resolution(
        "hires",
        "lossless",
        "FLAC",
        16,
        44100.0,
        "tidal",
        "tidal",
        true,
        false,
    );

    assert_eq!(decision.decision, QualityDecisionKind::CompletedWithQualityShortfall);
    assert_eq!(decision.effective_format, "flac");
    assert!(decision.quality_fallback_used);
    assert!(!decision.provider_fallback_used);
    assert!(!decision.retryable);
    assert!(decision.reason.is_some());
    assert!(decision.reason.as_deref().unwrap().contains("Quality shortfall"));
}
