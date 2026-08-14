//! Quality Classes, Stream Resolution models, and Quality Policy rules.

use serde::{Deserialize, Serialize};

/// High-level audio quality classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QualityClass {
    Lossless,
    Lossy,
}

impl std::fmt::Display for QualityClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QualityClass::Lossless => write!(f, "Lossless"),
            QualityClass::Lossy => write!(f, "Lossy"),
        }
    }
}

/// Source type of resolved audio stream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StreamSourceType {
    TidalOfficial,
    TidalProxy(String),
    QobuzOfficial,
    RequiresAuth,
    SourceUnavailable(String),
    Failed(String),
}

impl std::fmt::Display for StreamSourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StreamSourceType::TidalOfficial => write!(f, "Tidal Official API"),
            StreamSourceType::TidalProxy(domain) => write!(f, "Tidal Proxy ({})", domain),
            StreamSourceType::QobuzOfficial => write!(f, "Qobuz Official API"),
            StreamSourceType::RequiresAuth => write!(f, "Requires Authentication"),
            StreamSourceType::SourceUnavailable(reason) => write!(f, "Source Unavailable ({})", reason),
            StreamSourceType::Failed(reason) => write!(f, "Failed ({})", reason),
        }
    }
}

/// Detailed stream resolution metrics for downloads.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamResolution {
    pub url: String,
    pub source: StreamSourceType,
    pub source_name: String,
    pub requested_quality: String,
    pub obtained_quality: String,
    pub format_id_requested: String,
    pub format_id_obtained: String,
    pub quality_class_requested: QualityClass,
    pub quality_class_obtained: QualityClass,
    pub codec: String,
    pub container: String,
    pub extension: String,
    pub bit_depth: i32,
    pub sample_rate: f64,
    pub is_fallback: bool,
}


/// Quality policy engine enforcing strict quality guarantees without I/O.
pub struct QualityPolicy;

impl QualityPolicy {
    /// Evaluate whether an obtained quality class should be rejected given requested quality class.
    ///
    /// Returns `Err(rejection_reason)` if requested was Lossless but obtained was Lossy and `allow_lossy_fallback` is false.
    pub fn evaluate_downgrade(
        requested: QualityClass,
        obtained: QualityClass,
        codec: &str,
        allow_lossy_fallback: bool,
    ) -> Result<(), String> {
        if requested == QualityClass::Lossless && obtained == QualityClass::Lossy && !allow_lossy_fallback {
            Err(format!(
                "Quality rejection: requested_lossless_but_received_{}",
                codec.to_lowercase()
            ))
        } else {
            Ok(())
        }
    }

    /// Map a codec string to its corresponding QualityClass.
    pub fn classify_codec(codec: &str) -> QualityClass {
        match codec.to_uppercase().as_str() {
            "FLAC" | "ALAC" | "WAV" | "AIFF" => QualityClass::Lossless,
            _ => QualityClass::Lossy,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quality_policy_rejection() {
        // Lossless requested, Lossy obtained -> Reject
        let res = QualityPolicy::evaluate_downgrade(QualityClass::Lossless, QualityClass::Lossy, "AAC", false);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err(), "Quality rejection: requested_lossless_but_received_aac");

        // Lossless requested, Lossy obtained, but allow_lossy_fallback=true -> Accept
        let res_fallback = QualityPolicy::evaluate_downgrade(QualityClass::Lossless, QualityClass::Lossy, "AAC", true);
        assert!(res_fallback.is_ok());

        // Lossless requested, Lossless obtained -> Accept
        let res_lossless = QualityPolicy::evaluate_downgrade(QualityClass::Lossless, QualityClass::Lossless, "FLAC", false);
        assert!(res_lossless.is_ok());

        // Lossy requested, Lossy obtained -> Accept
        let res_lossy = QualityPolicy::evaluate_downgrade(QualityClass::Lossy, QualityClass::Lossy, "MP3", false);
        assert!(res_lossy.is_ok());
    }

    #[test]
    fn test_classify_codec() {
        assert_eq!(QualityPolicy::classify_codec("FLAC"), QualityClass::Lossless);
        assert_eq!(QualityPolicy::classify_codec("flac"), QualityClass::Lossless);
        assert_eq!(QualityPolicy::classify_codec("AAC"), QualityClass::Lossy);
        assert_eq!(QualityPolicy::classify_codec("mp3"), QualityClass::Lossy);
        assert_eq!(QualityPolicy::classify_codec("mp4a"), QualityClass::Lossy);
    }
}
