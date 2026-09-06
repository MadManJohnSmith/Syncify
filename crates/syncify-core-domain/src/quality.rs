//! Quality Classes, Stream Resolution models, and Quality Policy rules.

use serde::{Deserialize, Serialize};

/// High-level audio quality classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QualityClass {
    HiRes,
    Lossless,
    Lossy,
}

impl QualityClass {
    pub fn is_hires(&self) -> bool {
        matches!(self, QualityClass::HiRes)
    }

    pub fn is_lossless(&self) -> bool {
        matches!(self, QualityClass::HiRes | QualityClass::Lossless)
    }

    pub fn is_lossy(&self) -> bool {
        matches!(self, QualityClass::Lossy)
    }
}

impl std::fmt::Display for QualityClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QualityClass::HiRes => write!(f, "HiRes"),
            QualityClass::Lossless => write!(f, "Lossless"),
            QualityClass::Lossy => write!(f, "Lossy"),
        }
    }
}

/// Canonical audio format identifier enum for Hi-Res and Lossless tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormatId {
    Mp3_320,
    LosslessCd,
    HiRes96,
    HiResLossless,
}

impl FormatId {
    pub fn is_hires(&self) -> bool {
        matches!(self, FormatId::HiRes96 | FormatId::HiResLossless)
    }

    pub fn qobuz_id(&self) -> i32 {
        match self {
            FormatId::Mp3_320 => 5,
            FormatId::LosslessCd => 6,
            FormatId::HiRes96 => 7,
            FormatId::HiResLossless => 27,
        }
    }
}

/// Canonical audio tier classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioTier {
    Lossy,
    Lossless,
    HiRes,
}

impl AudioTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            AudioTier::Lossy => "lossy",
            AudioTier::Lossless => "lossless",
            AudioTier::HiRes => "hires",
        }
    }

    pub fn quality_class(&self) -> QualityClass {
        match self {
            AudioTier::Lossy => QualityClass::Lossy,
            AudioTier::Lossless | AudioTier::HiRes => QualityClass::Lossless,
        }
    }

    pub fn is_hires(&self) -> bool {
        matches!(self, AudioTier::HiRes)
    }

    pub fn is_lossless(&self) -> bool {
        matches!(self, AudioTier::Lossless | AudioTier::HiRes)
    }

    pub fn is_lossy(&self) -> bool {
        matches!(self, AudioTier::Lossy)
    }

    /// Canonical baseline score corresponding to this tier (for sorting/scoring).
    pub fn canonical_score(&self) -> i32 {
        match self {
            AudioTier::Lossy => 40,
            AudioTier::Lossless => 80,
            AudioTier::HiRes => 120,
        }
    }
}

impl std::fmt::Display for AudioTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for AudioTier {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "hires" | "hi_res" | "hi-res" | "hi_res_lossless" | "hires_lossless" | "high_resolution" | "max" | "24-192" | "24-96" => Ok(AudioTier::HiRes),
            "lossless" | "flac" | "cd" | "16-44" | "alac" | "wav" | "aiff" | "ape" => Ok(AudioTier::Lossless),
            "lossy" | "high" | "standard" | "low" | "normal" | "320" | "256" | "128" | "96" | "mp3" | "aac" | "ogg" | "opus" | "vorbis" | "m4a" | "wma" => Ok(AudioTier::Lossy),
            other => Err(format!("Unknown audio tier: {}", other)),
        }
    }
}

/// Canonical audio quality normalizer.
/// Maps any casing or legacy audio quality string into a canonical lowercase tier: `"lossless"`, `"hires"`, or `"lossy"`.
pub fn normalize_audio_quality(raw: &str) -> &'static str {
    match raw.trim().to_lowercase().as_str() {
        "hires" | "hi_res" | "hi-res" | "hi_res_lossless" | "hires_lossless" | "high_resolution" | "max" | "24-192" | "24-96" => "hires",
        "lossless" | "flac" | "cd" | "16-44" | "alac" | "wav" | "aiff" | "ape" => "lossless",
        "lossy" | "standard" | "high" | "low" | "normal" | "320" | "256" | "128" | "96" | "mp3" | "aac" | "ogg" | "opus" | "vorbis" | "m4a" | "wma" => "lossy",
        _ => classify_audio_tier(None, None, None, Some(raw)).as_str(),
    }
}

/// Canonical audio tier classifier based on physical audio attributes.
pub fn classify_audio_tier(
    bit_depth: Option<i32>,
    sample_rate: Option<i32>,
    bitrate: Option<i32>,
    codec: Option<&str>,
) -> AudioTier {
    let norm_codec = codec.map(|c| c.trim().to_uppercase());

    if let Some(ref c) = norm_codec {
        match c.as_str() {
            "MP3" | "AAC" | "M4A" | "OGG" | "OPUS" | "VORBIS" | "WMA" | "LOSSY" | "HIGH" | "STANDARD" | "LOW" | "NORMAL" | "320" | "256" | "128" | "96" => {
                return AudioTier::Lossy;
            }
            "HIRES" | "HI_RES" | "HI-RES" | "HI_RES_LOSSLESS" | "HIRES_LOSSLESS" | "HIGH_RESOLUTION" | "24-192" | "24-96" | "MAX" => {
                return AudioTier::HiRes;
            }
            _ => {}
        }
    }

    // Explicit bitrate without lossless indicator implies lossy compression
    if bitrate.is_some() && norm_codec.is_none() && bit_depth.is_none() {
        return AudioTier::Lossy;
    }

    let is_hires = bit_depth.map_or(false, |bd| bd > 16)
        || sample_rate.map_or(false, |sr| sr > 48000 || (sr > 48 && sr <= 384));

    if is_hires {
        return AudioTier::HiRes;
    }

    let is_lossless_codec = norm_codec.as_deref().map_or(false, |c| {
        matches!(c, "FLAC" | "ALAC" | "WAV" | "AIFF" | "APE" | "LOSSLESS" | "16-44" | "CD")
    });

    let is_lossless = is_lossless_codec || bit_depth.map_or(false, |bd| bd >= 16);

    if is_lossless {
        AudioTier::Lossless
    } else {
        AudioTier::Lossy
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


/// Canonical quality decision outcome variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QualityDecisionKind {
    ReadyExactQuality,
    ReadyProviderFallbackExactQuality,
    ReadyQualityFallback,
    CompletedExactQuality,
    CompletedWithProviderFallback,
    CompletedWithQualityFallback,
    CompletedWithQualityShortfall,
    RejectedQuality,
    NoDownloadProvider,
    UnavailableFromProvider,
    EntitlementDenied,
    AuthInvalid,
    RateLimited,
    TemporaryFailure,
}

impl QualityDecisionKind {
    pub fn is_success(&self) -> bool {
        matches!(
            self,
            QualityDecisionKind::ReadyExactQuality
                | QualityDecisionKind::ReadyProviderFallbackExactQuality
                | QualityDecisionKind::ReadyQualityFallback
                | QualityDecisionKind::CompletedExactQuality
                | QualityDecisionKind::CompletedWithProviderFallback
                | QualityDecisionKind::CompletedWithQualityFallback
                | QualityDecisionKind::CompletedWithQualityShortfall
        )
    }

    pub fn is_terminal_failure(&self) -> bool {
        matches!(
            self,
            QualityDecisionKind::RejectedQuality
                | QualityDecisionKind::NoDownloadProvider
                | QualityDecisionKind::UnavailableFromProvider
                | QualityDecisionKind::EntitlementDenied
                | QualityDecisionKind::AuthInvalid
        )
    }

    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            QualityDecisionKind::RateLimited | QualityDecisionKind::TemporaryFailure
        )
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            QualityDecisionKind::ReadyExactQuality => "ReadyExactQuality",
            QualityDecisionKind::ReadyProviderFallbackExactQuality => "ReadyProviderFallbackExactQuality",
            QualityDecisionKind::ReadyQualityFallback => "ReadyQualityFallback",
            QualityDecisionKind::CompletedExactQuality => "CompletedExactQuality",
            QualityDecisionKind::CompletedWithProviderFallback => "CompletedWithProviderFallback",
            QualityDecisionKind::CompletedWithQualityFallback => "CompletedWithQualityFallback",
            QualityDecisionKind::CompletedWithQualityShortfall => "CompletedWithQualityShortfall",
            QualityDecisionKind::RejectedQuality => "RejectedQuality",
            QualityDecisionKind::NoDownloadProvider => "NoDownloadProvider",
            QualityDecisionKind::UnavailableFromProvider => "UnavailableFromProvider",
            QualityDecisionKind::EntitlementDenied => "EntitlementDenied",
            QualityDecisionKind::AuthInvalid => "AuthInvalid",
            QualityDecisionKind::RateLimited => "RateLimited",
            QualityDecisionKind::TemporaryFailure => "TemporaryFailure",
        }
    }

    pub fn as_snake_case(&self) -> &'static str {
        match self {
            QualityDecisionKind::ReadyExactQuality => "ready_exact_quality",
            QualityDecisionKind::ReadyProviderFallbackExactQuality => "ready_provider_fallback_exact_quality",
            QualityDecisionKind::ReadyQualityFallback => "ready_quality_fallback",
            QualityDecisionKind::CompletedExactQuality => "completed_exact_quality",
            QualityDecisionKind::CompletedWithProviderFallback => "completed_with_provider_fallback",
            QualityDecisionKind::CompletedWithQualityFallback => "completed_with_quality_fallback",
            QualityDecisionKind::CompletedWithQualityShortfall => "completed_with_quality_shortfall",
            QualityDecisionKind::RejectedQuality => "rejected_quality",
            QualityDecisionKind::NoDownloadProvider => "no_download_provider",
            QualityDecisionKind::UnavailableFromProvider => "unavailable_from_provider",
            QualityDecisionKind::EntitlementDenied => "entitlement_denied",
            QualityDecisionKind::AuthInvalid => "auth_invalid",
            QualityDecisionKind::RateLimited => "rate_limited",
            QualityDecisionKind::TemporaryFailure => "temporary_failure",
        }
    }
}

impl std::fmt::Display for QualityDecisionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for QualityDecisionKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "ReadyExactQuality" | "ready_exact_quality" => Ok(QualityDecisionKind::ReadyExactQuality),
            "ReadyProviderFallbackExactQuality" | "ready_provider_fallback_exact_quality" => {
                Ok(QualityDecisionKind::ReadyProviderFallbackExactQuality)
            }
            "ReadyQualityFallback" | "ready_quality_fallback" => Ok(QualityDecisionKind::ReadyQualityFallback),
            "CompletedExactQuality" | "completed_exact_quality" => Ok(QualityDecisionKind::CompletedExactQuality),
            "CompletedWithProviderFallback" | "completed_with_provider_fallback" => {
                Ok(QualityDecisionKind::CompletedWithProviderFallback)
            }
            "CompletedWithQualityFallback" | "completed_with_quality_fallback" => {
                Ok(QualityDecisionKind::CompletedWithQualityFallback)
            }
            "CompletedWithQualityShortfall"
            | "completed_with_quality_shortfall"
            | "completed_with_shortfall"
            | "shortfall" => Ok(QualityDecisionKind::CompletedWithQualityShortfall),
            "RejectedQuality" | "rejected_quality" => Ok(QualityDecisionKind::RejectedQuality),
            "NoDownloadProvider" | "no_download_provider" => Ok(QualityDecisionKind::NoDownloadProvider),
            "UnavailableFromProvider" | "unavailable_from_provider" => {
                Ok(QualityDecisionKind::UnavailableFromProvider)
            }
            "EntitlementDenied" | "entitlement_denied" => Ok(QualityDecisionKind::EntitlementDenied),
            "AuthInvalid" | "auth_invalid" => Ok(QualityDecisionKind::AuthInvalid),
            "RateLimited" | "rate_limited" => Ok(QualityDecisionKind::RateLimited),
            "TemporaryFailure" | "temporary_failure" => Ok(QualityDecisionKind::TemporaryFailure),
            other => Err(format!("Unknown QualityDecisionKind: {}", other)),
        }
    }
}

/// Canonical observable decision struct for quality and fallback evaluation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityDecision {
    pub requested_quality: String,
    pub provider_available_quality: Option<String>,
    pub effective_quality: String,
    pub requested_format: String,
    pub effective_format: String,
    pub strict_quality: bool,
    pub allow_lossy_fallback: bool,
    pub provider_fallback_used: bool,
    pub quality_fallback_used: bool,
    pub decision: QualityDecisionKind,
    pub reason: Option<String>,
    pub retryable: bool,
    pub user_message: String,
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
        if requested.is_lossless() && obtained == QualityClass::Lossy && !allow_lossy_fallback {
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

    /// Canonical audio tier classifier delegating to `classify_audio_tier`.
    pub fn classify_audio_tier(
        bit_depth: Option<i32>,
        sample_rate: Option<i32>,
        bitrate: Option<i32>,
        codec: Option<&str>,
    ) -> AudioTier {
        classify_audio_tier(bit_depth, sample_rate, bitrate, codec)
    }

    /// Helper to determine if an available candidate quality is inferior to requested under strict policy.
    pub fn is_quality_inferior(
        requested_quality: Option<&str>,
        candidate_quality: Option<&str>,
        candidate_format: Option<&str>,
        candidate_bit_depth: Option<i64>,
    ) -> bool {
        let req = requested_quality.unwrap_or("lossless").to_lowercase();
        let cand_q = candidate_quality.unwrap_or("lossy").to_lowercase();
        let fmt = candidate_format.unwrap_or("").to_uppercase();
        let bd = candidate_bit_depth.unwrap_or(0);

        let req_rank = match req.as_str() {
            "hires" | "24-192" | "24-96" | "hi_res" | "hi-res" | "max" => 3,
            "lossless" | "flac" | "16-44" | "cd" => 2,
            _ => 1,
        };

        let is_lossy_codec = fmt == "AAC"
            || fmt == "MP3"
            || fmt == "M4A"
            || fmt == "OGG"
            || fmt == "OPUS"
            || cand_q == "lossy"
            || cand_q == "high"
            || cand_q == "320";

        let cand_rank = if is_lossy_codec {
            1
        } else if bd >= 24 || cand_q == "hires" {
            3
        } else if fmt == "FLAC"
            || fmt == "ALAC"
            || fmt == "WAV"
            || fmt == "AIFF"
            || cand_q == "lossless"
            || bd >= 16
        {
            2
        } else {
            1
        };

        cand_rank < req_rank
    }

    /// Evaluate preflight quality and provider compatibility
    pub fn evaluate_preflight(
        requested_quality: &str,
        candidate_quality: Option<&str>,
        candidate_format: Option<&str>,
        candidate_bit_depth: Option<i64>,
        origin_service: &str,
        target_service: &str,
        strict_quality: bool,
        allow_fallback: bool,
    ) -> QualityDecision {
        let provider_fallback_used = !origin_service.eq_ignore_ascii_case(target_service);
        let cand_q_str = candidate_quality.unwrap_or("lossy");
        let cand_fmt_str = candidate_format.unwrap_or("FLAC");
        let is_inferior = Self::is_quality_inferior(
            Some(requested_quality),
            Some(cand_q_str),
            Some(cand_fmt_str),
            candidate_bit_depth,
        );

        let req_norm = requested_quality.to_lowercase();
        let req_format = if req_norm == "mp3" || req_norm == "high" || req_norm == "320" {
            "mp3".to_string()
        } else {
            "flac".to_string()
        };

        if is_inferior && (strict_quality || !allow_fallback) {
            let reason = format!(
                "Quality rejection: requested_{}_but_provider_available_is_{}",
                requested_quality, cand_q_str
            );
            return QualityDecision {
                requested_quality: requested_quality.to_string(),
                provider_available_quality: candidate_quality.map(|s| s.to_string()),
                effective_quality: cand_q_str.to_string(),
                requested_format: req_format,
                effective_format: cand_fmt_str.to_lowercase(),
                strict_quality,
                allow_lossy_fallback: allow_fallback,
                provider_fallback_used,
                quality_fallback_used: true,
                decision: QualityDecisionKind::RejectedQuality,
                reason: Some(reason.clone()),
                retryable: false,
                user_message: format!(
                    "Quality rejected: available format ({}) does not meet strict quality requirement ({})",
                    cand_q_str, requested_quality
                ),
            };
        }

        let quality_fallback_used = is_inferior && allow_fallback && !strict_quality;
        let decision_kind = if quality_fallback_used {
            QualityDecisionKind::ReadyQualityFallback
        } else if provider_fallback_used {
            QualityDecisionKind::ReadyProviderFallbackExactQuality
        } else {
            QualityDecisionKind::ReadyExactQuality
        };

        QualityDecision {
            requested_quality: requested_quality.to_string(),
            provider_available_quality: candidate_quality.map(|s| s.to_string()),
            effective_quality: if is_inferior { cand_q_str.to_string() } else { requested_quality.to_string() },
            requested_format: req_format,
            effective_format: cand_fmt_str.to_lowercase(),
            strict_quality,
            allow_lossy_fallback: allow_fallback,
            provider_fallback_used,
            quality_fallback_used,
            decision: decision_kind,
            reason: None,
            retryable: false,
            user_message: format!(
                "Ready for download via {} (Quality: {})",
                target_service, if is_inferior { cand_q_str } else { requested_quality }
            ),
        }
    }

    /// Check if a requested quality string indicates Hi-Res audio.
    pub fn is_hires_requested(requested_quality: &str) -> bool {
        let req_norm = requested_quality.trim().to_lowercase();
        if matches!(
            req_norm.as_str(),
            "hires" | "hi_res" | "hi-res" | "max" | "24-192" | "24-96" | "24/96" | "24/192"
                | "hires_lossless" | "hireslossless" | "hires lossless" | "hi-res lossless"
        ) || req_norm.contains("hires")
          || req_norm.contains("hi-res")
          || req_norm.contains("hi_res")
        {
            return true;
        }

        if let Ok(fid) = req_norm.parse::<i32>() {
            if fid >= 7 {
                return true;
            }
        }

        false
    }

    /// Evaluate post-stream-resolution quality outcome
    pub fn evaluate_stream_resolution(
        requested_quality: &str,
        stream_quality: &str,
        stream_codec: &str,
        stream_bit_depth: i32,
        stream_sample_rate: f64,
        origin_service: &str,
        target_service: &str,
        strict_quality: bool,
        allow_fallback: bool,
    ) -> QualityDecision {
        let provider_fallback_used = !origin_service.eq_ignore_ascii_case(target_service);
        let req_class = match requested_quality.to_lowercase().as_str() {
            "mp3" | "high" | "320" | "lossy" => QualityClass::Lossy,
            "hires" | "hi_res" | "hi-res" | "max" | "24-192" | "24-96" => QualityClass::HiRes,
            _ => QualityClass::Lossless,
        };
        let obtained_class = Self::classify_codec(stream_codec);
        let quality_downgrade = req_class.is_lossless() && obtained_class == QualityClass::Lossy;

        let req_format = if req_class == QualityClass::Lossy {
            "mp3".to_string()
        } else {
            "flac".to_string()
        };
        let eff_format = stream_codec.to_lowercase();

        if quality_downgrade && (strict_quality || !allow_fallback) {
            let reason = format!(
                "Provider returned {}; lossy fallback is disabled",
                stream_codec.to_uppercase()
            );
            return QualityDecision {
                requested_quality: requested_quality.to_string(),
                provider_available_quality: Some(stream_quality.to_string()),
                effective_quality: stream_quality.to_string(),
                requested_format: req_format,
                effective_format: eff_format,
                strict_quality,
                allow_lossy_fallback: allow_fallback,
                provider_fallback_used,
                quality_fallback_used: true,
                decision: QualityDecisionKind::RejectedQuality,
                reason: Some(reason),
                retryable: false,
                user_message: format!(
                    "Quality rejection: stream format ({}) is lossy, but strict quality was requested",
                    stream_codec
                ),
            };
        }

        // F3.5: Detect Quality Shortfall when Hi-Res was requested but verified physical STREAMINFO is CD standard
        let req_is_hires = Self::is_hires_requested(requested_quality) || req_class == QualityClass::HiRes;
        let physical_tier = classify_audio_tier(
            if stream_bit_depth > 0 { Some(stream_bit_depth) } else { None },
            if stream_sample_rate > 0.0 { Some(stream_sample_rate as i32) } else { None },
            None,
            Some(stream_codec),
        );

        let is_hires_shortfall = req_is_hires && !physical_tier.is_hires() && !quality_downgrade && obtained_class.is_lossless();

        if is_hires_shortfall {
            let bd = if stream_bit_depth > 0 { stream_bit_depth } else { 16 };
            let sr = if stream_sample_rate > 0.0 { stream_sample_rate } else { 44100.0 };
            let reason = format!(
                "Quality shortfall: requested Hi-Res ({}), but STREAMINFO verified CD quality ({}bit/{:.1}kHz)",
                requested_quality, bd, sr / 1000.0
            );
            let effective_q = if stream_bit_depth > 0 && stream_sample_rate > 0.0 {
                format!("FLAC {}bit/{:.1}kHz", stream_bit_depth, sr / 1000.0)
            } else {
                stream_quality.to_string()
            };

            return QualityDecision {
                requested_quality: requested_quality.to_string(),
                provider_available_quality: Some(stream_quality.to_string()),
                effective_quality: effective_q,
                requested_format: req_format,
                effective_format: eff_format,
                strict_quality,
                allow_lossy_fallback: allow_fallback,
                provider_fallback_used,
                quality_fallback_used: true,
                decision: QualityDecisionKind::CompletedWithQualityShortfall,
                reason: Some(reason),
                retryable: false,
                user_message: format!(
                    "Successfully downloaded via {} with quality shortfall (requested Hi-Res, verified CD quality)",
                    target_service
                ),
            };
        }

        let decision_kind = if quality_downgrade {
            QualityDecisionKind::CompletedWithQualityFallback
        } else if provider_fallback_used {
            QualityDecisionKind::CompletedWithProviderFallback
        } else {
            QualityDecisionKind::CompletedExactQuality
        };

        let decision_reason = if quality_downgrade {
            Some(format!(
                "Provider returned {}; lossy fallback is enabled",
                stream_codec.to_uppercase()
            ))
        } else {
            None
        };

        QualityDecision {
            requested_quality: requested_quality.to_string(),
            provider_available_quality: Some(stream_quality.to_string()),
            effective_quality: stream_quality.to_string(),
            requested_format: req_format,
            effective_format: eff_format,
            strict_quality,
            allow_lossy_fallback: allow_fallback,
            provider_fallback_used,
            quality_fallback_used: quality_downgrade,
            decision: decision_kind,
            reason: decision_reason,
            retryable: false,
            user_message: format!(
                "Successfully downloaded via {} with {}",
                target_service, if quality_downgrade { "quality fallback" } else { "exact quality" }
            ),
        }
    }
}

/// Standard loudness normalization targets
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum LoudnessStandard {
    /// EBU R128 (-23.0 LUFS)
    EbuR128,
    /// ReplayGain 2.0 (-18.0 LUFS / 89 dB SPL)
    ReplayGain2,
    /// Streaming standard (Spotify / YouTube -14.0 LUFS)
    Streaming,
}

impl LoudnessStandard {
    pub fn target_lufs(&self) -> f64 {
        match self {
            LoudnessStandard::EbuR128 => -23.0,
            LoudnessStandard::ReplayGain2 => -18.0,
            LoudnessStandard::Streaming => -14.0,
        }
    }
}

/// Extracted loudness metrics for an audio file
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioLoudnessMetrics {
    pub integrated_lufs: f64,
    pub true_peak_dbfs: f64,
    pub loudness_range_lu: f64,
}

impl AudioLoudnessMetrics {
    /// Compute recommended track gain delta in dB for a given standard
    pub fn calculate_gain_delta(&self, standard: LoudnessStandard) -> f64 {
        standard.target_lufs() - self.integrated_lufs
    }

    /// Format gain in standard ReplayGain format ("-X.XX dB")
    pub fn format_replaygain_track_gain(&self) -> String {
        let gain = self.calculate_gain_delta(LoudnessStandard::ReplayGain2);
        format!("{:+.2} dB", gain)
    }

    /// Format gain in EBU R128 format ("-X.XX LU")
    pub fn format_r128_track_gain(&self) -> String {
        let gain = self.calculate_gain_delta(LoudnessStandard::EbuR128);
        format!("{:+.2} LU", gain)
    }

    /// Format true peak as standard ReplayGain ratio string ("0.XXXXXX")
    pub fn format_replaygain_track_peak(&self) -> String {
        let peak_linear = 10.0_f64.powf(self.true_peak_dbfs / 20.0);
        format!("{:.6}", peak_linear.min(1.0).max(0.0))
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

    #[test]
    fn test_loudness_metrics_and_gain_delta() {
        let metrics = AudioLoudnessMetrics {
            integrated_lufs: -11.5,
            true_peak_dbfs: -0.1,
            loudness_range_lu: 6.2,
        };

        // ReplayGain delta (-18.0 - (-11.5) = -6.50 dB)
        let rg_delta = metrics.calculate_gain_delta(LoudnessStandard::ReplayGain2);
        assert!((rg_delta - (-6.50)).abs() < 1e-6);
        assert_eq!(metrics.format_replaygain_track_gain(), "-6.50 dB");

        // EBU R128 delta (-23.0 - (-11.5) = -11.50 LU)
        let r128_delta = metrics.calculate_gain_delta(LoudnessStandard::EbuR128);
        assert!((r128_delta - (-11.50)).abs() < 1e-6);
        assert_eq!(metrics.format_r128_track_gain(), "-11.50 LU");

        // Streaming delta (-14.0 - (-11.5) = -2.50 dB)
        let stream_delta = metrics.calculate_gain_delta(LoudnessStandard::Streaming);
        assert!((stream_delta - (-2.50)).abs() < 1e-6);

        // Peak linear ratio
        let peak_str = metrics.format_replaygain_track_peak();
        assert!(!peak_str.is_empty());
    }

    #[test]
    fn test_quality_policy_evaluate_preflight_matrix() {
        // 1. Exact quality matching
        let d1 = QualityPolicy::evaluate_preflight(
            "lossless", Some("lossless"), Some("FLAC"), Some(16), "qobuz", "qobuz", true, false,
        );
        assert_eq!(d1.decision, QualityDecisionKind::ReadyExactQuality);
        assert!(!d1.provider_fallback_used);
        assert!(!d1.quality_fallback_used);

        // 2. Provider fallback with exact quality
        let d2 = QualityPolicy::evaluate_preflight(
            "lossless", Some("lossless"), Some("FLAC"), Some(16), "spotify", "qobuz", true, false,
        );
        assert_eq!(d2.decision, QualityDecisionKind::ReadyProviderFallbackExactQuality);
        assert!(d2.provider_fallback_used);
        assert!(!d2.quality_fallback_used);

        // 3. Strict quality rejection of inferior candidate
        let d3 = QualityPolicy::evaluate_preflight(
            "lossless", Some("lossy"), Some("AAC"), Some(16), "tidal", "tidal", true, false,
        );
        assert_eq!(d3.decision, QualityDecisionKind::RejectedQuality);
        assert!(!d3.retryable);
        assert!(d3.reason.is_some());

        // 4. Quality fallback opt-in allowed
        let d4 = QualityPolicy::evaluate_preflight(
            "lossless", Some("lossy"), Some("AAC"), Some(16), "tidal", "tidal", false, true,
        );
        assert_eq!(d4.decision, QualityDecisionKind::ReadyQualityFallback);
        assert!(d4.quality_fallback_used);
    }

    #[test]
    fn test_quality_policy_evaluate_stream_resolution_matrix() {
        // 1. Exact lossless FLAC
        let s1 = QualityPolicy::evaluate_stream_resolution(
            "lossless", "lossless", "FLAC", 16, 44100.0, "qobuz", "qobuz", true, false,
        );
        assert_eq!(s1.decision, QualityDecisionKind::CompletedExactQuality);
        assert_eq!(s1.effective_format, "flac");

        // 2. Strict rejection of AAC stream
        let s2 = QualityPolicy::evaluate_stream_resolution(
            "lossless", "lossy", "AAC", 16, 44100.0, "tidal", "tidal", true, false,
        );
        assert_eq!(s2.decision, QualityDecisionKind::RejectedQuality);
        assert!(!s2.retryable);
        assert_eq!(s2.reason.as_deref(), Some("Provider returned AAC; lossy fallback is disabled"));

        // 3. Opt-in quality fallback AAC stream
        let s3 = QualityPolicy::evaluate_stream_resolution(
            "lossless", "lossy", "AAC", 16, 44100.0, "tidal", "tidal", false, true,
        );
        assert_eq!(s3.decision, QualityDecisionKind::CompletedWithQualityFallback);
        assert!(s3.quality_fallback_used);
        assert_eq!(s3.effective_format, "aac");

        // 4. Provider fallback + exact quality
        let s4 = QualityPolicy::evaluate_stream_resolution(
            "lossless", "lossless", "FLAC", 16, 44100.0, "spotify", "qobuz", true, false,
        );
        assert_eq!(s4.decision, QualityDecisionKind::CompletedWithProviderFallback);
        assert!(s4.provider_fallback_used);
        assert!(!s4.quality_fallback_used);

        // 5. Hi-Res requested, but 16/44.1 CD-quality FLAC delivered -> CompletedWithQualityShortfall
        let s5 = QualityPolicy::evaluate_stream_resolution(
            "hires", "lossless", "FLAC", 16, 44100.0, "qobuz", "qobuz", true, false,
        );
        assert_eq!(s5.decision, QualityDecisionKind::CompletedWithQualityShortfall);
        assert!(s5.quality_fallback_used);
        assert!(s5.reason.as_deref().unwrap().contains("Quality shortfall"));

        // 6. Hi-Res requested with format_id "7", 24/96 delivered -> CompletedExactQuality
        let s6 = QualityPolicy::evaluate_stream_resolution(
            "7", "hires", "FLAC", 24, 96000.0, "qobuz", "qobuz", true, false,
        );
        assert_eq!(s6.decision, QualityDecisionKind::CompletedExactQuality);
        assert!(!s6.quality_fallback_used);
    }

    #[test]
    fn test_classify_audio_tier() {
        // 24-bit FLAC -> HiRes
        assert_eq!(
            classify_audio_tier(Some(24), Some(96000), None, Some("FLAC")),
            AudioTier::HiRes
        );
        assert_eq!(
            classify_audio_tier(Some(24), Some(44100), None, Some("FLAC")),
            AudioTier::HiRes
        );
        // 16-bit 96kHz FLAC -> HiRes
        assert_eq!(
            classify_audio_tier(Some(16), Some(96000), None, Some("FLAC")),
            AudioTier::HiRes
        );
        // 16-bit 44.1kHz FLAC -> Lossless
        assert_eq!(
            classify_audio_tier(Some(16), Some(44100), None, Some("FLAC")),
            AudioTier::Lossless
        );
        // FLAC with no bit depth / sample rate -> Lossless
        assert_eq!(
            classify_audio_tier(None, None, None, Some("FLAC")),
            AudioTier::Lossless
        );
        // MP3 320kbps -> Lossy
        assert_eq!(
            classify_audio_tier(Some(16), Some(44100), Some(320), Some("MP3")),
            AudioTier::Lossy
        );
        // SoundCloud MP3 128kbps -> Lossy (even if bitrate is 128)
        assert_eq!(
            classify_audio_tier(None, None, Some(128), Some("MP3")),
            AudioTier::Lossy
        );
        // Apple Music AAC 256kbps -> Lossy
        assert_eq!(
            classify_audio_tier(Some(16), Some(44100), Some(256), Some("AAC")),
            AudioTier::Lossy
        );
        // Default when nothing provided -> Lossy
        assert_eq!(
            classify_audio_tier(None, None, None, None),
            AudioTier::Lossy
        );
        // Tidal labels
        assert_eq!(
            classify_audio_tier(None, None, None, Some("HI_RES_LOSSLESS")),
            AudioTier::HiRes
        );
        assert_eq!(
            classify_audio_tier(None, None, None, Some("LOSSLESS")),
            AudioTier::Lossless
        );
        assert_eq!(
            classify_audio_tier(None, None, None, Some("HIGH")),
            AudioTier::Lossy
        );
        assert_eq!(
            classify_audio_tier(None, None, None, Some("STANDARD")),
            AudioTier::Lossy
        );
        assert_eq!("hi_res_lossless".parse::<AudioTier>().unwrap(), AudioTier::HiRes);
        assert_eq!("hires_lossless".parse::<AudioTier>().unwrap(), AudioTier::HiRes);
        assert_eq!("lossless".parse::<AudioTier>().unwrap(), AudioTier::Lossless);
        assert_eq!("standard".parse::<AudioTier>().unwrap(), AudioTier::Lossy);
        // Ordering: Lossy < Lossless < HiRes
        assert!(AudioTier::Lossy < AudioTier::Lossless);
        assert!(AudioTier::Lossless < AudioTier::HiRes);
        // AudioTier string and quality class mapping
        assert_eq!(AudioTier::HiRes.as_str(), "hires");
        assert_eq!(AudioTier::Lossless.as_str(), "lossless");
        assert_eq!(AudioTier::Lossy.as_str(), "lossy");
        assert_eq!(AudioTier::HiRes.quality_class(), QualityClass::Lossless);
        assert_eq!(AudioTier::Lossless.quality_class(), QualityClass::Lossless);
        assert_eq!(AudioTier::Lossy.quality_class(), QualityClass::Lossy);

        // TASK-145: normalize_audio_quality verification
        assert_eq!(normalize_audio_quality("lossless"), "lossless");
        assert_eq!(normalize_audio_quality("LOSSLESS"), "lossless");
        assert_eq!(normalize_audio_quality("flac"), "lossless");
        assert_eq!(normalize_audio_quality("FLAC"), "lossless");
        assert_eq!(normalize_audio_quality("hires"), "hires");
        assert_eq!(normalize_audio_quality("HIRES"), "hires");
        assert_eq!(normalize_audio_quality("HI_RES"), "hires");
        assert_eq!(normalize_audio_quality("hi-res"), "hires");
        assert_eq!(normalize_audio_quality("HI_RES_LOSSLESS"), "hires");
        assert_eq!(normalize_audio_quality("standard"), "lossy");
        assert_eq!(normalize_audio_quality("STANDARD"), "lossy");
        assert_eq!(normalize_audio_quality("HIGH"), "lossy");
        assert_eq!(normalize_audio_quality("high"), "lossy");
        assert_eq!(normalize_audio_quality("LOW"), "lossy");
        assert_eq!(normalize_audio_quality("low"), "lossy");
        assert_eq!(normalize_audio_quality("normal"), "lossy");
        assert_eq!(normalize_audio_quality("mp3"), "lossy");
        assert_eq!(normalize_audio_quality("aac"), "lossy");
        assert_eq!(normalize_audio_quality("  LOSSLESS \n"), "lossless");
        assert_eq!(normalize_audio_quality("  standard \t"), "lossy");
        assert_eq!(normalize_audio_quality("unknown_fallback"), "lossy");
    }
}
