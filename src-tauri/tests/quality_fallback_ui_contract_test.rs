//! UI Contract Test for Quality Policy & Quality Decision IPC Serialization (100% Offline)
//!
//! Validates that QualityDecision, QualityDecisionKind, TrackPreflightResult,
//! and related DTOs serialize faithfully for the Tauri frontend IPC boundary.

use syncify_core_domain::quality::{QualityDecisionKind, QualityPolicy};

#[test]
fn test_quality_decision_json_serialization_all_variants() {
    let variants = vec![
        QualityDecisionKind::ReadyExactQuality,
        QualityDecisionKind::ReadyProviderFallbackExactQuality,
        QualityDecisionKind::ReadyQualityFallback,
        QualityDecisionKind::CompletedExactQuality,
        QualityDecisionKind::CompletedWithProviderFallback,
        QualityDecisionKind::CompletedWithQualityFallback,
        QualityDecisionKind::CompletedWithQualityShortfall,
        QualityDecisionKind::RejectedQuality,
        QualityDecisionKind::NoDownloadProvider,
        QualityDecisionKind::UnavailableFromProvider,
        QualityDecisionKind::EntitlementDenied,
        QualityDecisionKind::AuthInvalid,
        QualityDecisionKind::RateLimited,
        QualityDecisionKind::TemporaryFailure,
    ];

    for variant in variants {
        let json_val = serde_json::to_value(&variant).expect("Failed to serialize QualityDecisionKind");
        let serialized_str = json_val.as_str().expect("Variant should serialize to string");
        assert_eq!(serialized_str, variant.to_string());

        let deserialized: QualityDecisionKind =
            serde_json::from_value(json_val).expect("Failed to deserialize QualityDecisionKind");
        assert_eq!(deserialized, variant);
    }
}

#[test]
fn test_quality_decision_payload_structure_matches_ui_expectations() {
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

    let json_str = serde_json::to_string(&decision).expect("QualityDecision must serialize cleanly");
    let v: serde_json::Value = serde_json::from_str(&json_str).expect("Valid JSON");

    // Verify all mandatory contract fields exist in serialized representation
    assert_eq!(v["requested_quality"], "lossless");
    assert_eq!(v["effective_quality"], "high");
    assert_eq!(v["requested_format"], "flac");
    assert_eq!(v["effective_format"], "aac");
    assert_eq!(v["strict_quality"], false);
    assert_eq!(v["allow_lossy_fallback"], true);
    assert_eq!(v["provider_fallback_used"], true);
    assert_eq!(v["quality_fallback_used"], true);
    assert_eq!(v["decision"], "CompletedWithQualityFallback");
    assert_eq!(v["retryable"], false);
    assert!(v["user_message"].is_string());
}

#[test]
fn test_rejected_quality_payload_structure() {
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

    let json_str = serde_json::to_string(&decision).expect("QualityDecision must serialize cleanly");
    let v: serde_json::Value = serde_json::from_str(&json_str).expect("Valid JSON");

    assert_eq!(v["decision"], "RejectedQuality");
    assert_eq!(v["reason"], "Provider returned AAC; lossy fallback is disabled");
    assert_eq!(v["retryable"], false);
}
