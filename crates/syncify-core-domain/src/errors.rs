//! Classified errors for Syncify pipeline without I/O dependencies.

use serde::{Deserialize, Serialize};

/// High-level classified error categories for pipeline execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "details", rename_all = "snake_case")]
pub enum PipelineError {
    /// Track could not be resolved or found on provider
    TrackUnresolved { provider: String, query: String },
    /// Authentication required or expired
    RequiresAuth(RequiresAuthReason),
    /// Playback endpoint returned unauthorized (e.g. HTTP 401 with subStatus or expired session)
    PlaybackUnauthorized {
        provider: String,
        http_status: u16,
        sub_status: Option<String>,
        message: String,
    },
    /// Provider API temporarily unavailable or service down
    SourceUnavailable { provider: String, message: String },
    /// Stream obtained failed strict quality criteria (downgrade rejected)
    RejectedQuality {
        requested: String,
        obtained: String,
        reason: String,
    },
    /// Network connection or timeout error
    NetworkError {
        provider: String,
        endpoint: String,
        message: String,
    },
    /// Track not found on provider (alias for TrackUnresolved)
    NotFound { provider: String, query: String },
    /// Audio file corruption or invalid header
    InvalidAudioPayload { format: String, reason: String },
    /// Cover art processing error
    CoverError { stage: String, reason: String },
    /// Input file or sidecar changed between dry-run validation and apply execution
    RepairInputChanged { reason: String },
    /// General unclassified failure
    InternalError(String),
}

impl PipelineError {
    /// Whether this error represents a transient failure that may be retried automatically.
    /// Permanent errors (RequiresAuth, PlaybackUnauthorized, RejectedQuality, TrackUnresolved) return false.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            PipelineError::NetworkError { .. } | PipelineError::SourceUnavailable { .. }
        )
    }

    /// Whether this error requires user authentication / re-login intervention.
    pub fn is_auth_failure(&self) -> bool {
        matches!(
            self,
            PipelineError::RequiresAuth(_) | PipelineError::PlaybackUnauthorized { .. }
        )
    }

    /// Machine-readable classification code.
    pub fn error_code(&self) -> &'static str {
        match self {
            PipelineError::TrackUnresolved { .. } | PipelineError::NotFound { .. } => "TrackUnresolved",
            PipelineError::RequiresAuth(_) => "RequiresAuth",
            PipelineError::PlaybackUnauthorized { .. } => "PlaybackUnauthorized",
            PipelineError::SourceUnavailable { .. } => "SourceUnavailable",
            PipelineError::RejectedQuality { .. } => "RejectedQuality",
            PipelineError::NetworkError { .. } => "NetworkError",
            PipelineError::InvalidAudioPayload { .. } => "InvalidAudioPayload",
            PipelineError::CoverError { .. } => "CoverError",
            PipelineError::RepairInputChanged { .. } => "RepairInputChanged",
            PipelineError::InternalError(_) => "InternalError",
        }
    }
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PipelineError::TrackUnresolved { provider, query } => {
                write!(f, "Track unresolved on {}: {}", provider, query)
            }
            PipelineError::RequiresAuth(r) => write!(f, "Authentication required: {}", r),
            PipelineError::PlaybackUnauthorized { provider, http_status, sub_status, message } => {
                if let Some(sub) = sub_status {
                    write!(f, "Playback unauthorized on {} (HTTP {}, subStatus {}): {}", provider, http_status, sub, message)
                } else {
                    write!(f, "Playback unauthorized on {} (HTTP {}): {}", provider, http_status, message)
                }
            }
            PipelineError::SourceUnavailable { provider, message } => {
                write!(f, "Provider {} unavailable: {}", provider, message)
            }
            PipelineError::RejectedQuality { requested, obtained, reason } => {
                write!(f, "Quality rejected (requested: {}, obtained: {}): {}", requested, obtained, reason)
            }
            PipelineError::NetworkError { provider, endpoint, message } => {
                write!(f, "Network error on {} [{}]: {}", provider, endpoint, message)
            }
            PipelineError::NotFound { provider, query } => {
                write!(f, "Track not found on {}: {}", provider, query)
            }
            PipelineError::InvalidAudioPayload { format, reason } => {
                write!(f, "Invalid {} audio payload: {}", format, reason)
            }
            PipelineError::CoverError { stage, reason } => {
                write!(f, "Cover processing error during {}: {}", stage, reason)
            }
            PipelineError::RepairInputChanged { reason } => {
                write!(f, "RepairInputChanged: {}", reason)
            }
            PipelineError::InternalError(msg) => write!(f, "Internal pipeline error: {}", msg),
        }
    }
}


/// Central taxonomy of pipeline and catalog errors for import, download, and UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "category", rename_all = "snake_case")]
pub enum ErrorTaxonomy {
    AuthInvalid { message: String },
    AuthRefreshable { message: String },
    EntitlementDenied { provider: String, reason: String },
    RejectedQuality { requested: String, obtained: String, reason: String },
    RegionRestricted { provider: String, country: String },
    UnavailableFromProvider { provider: String, item_id: String, reason: String },
    RateLimited { provider: String, retry_after_sec: Option<u64> },
    TemporaryNetworkFailure { endpoint: String, message: String },
    Timeout { endpoint: String, elapsed_ms: u64 },
    MalformedProviderPayload { provider: String, field: String, reason: String },
    IdentityConflict { field: String, existing_value: String, conflicting_value: String },
    MetadataResolutionFailed { provider: String, query: String, reason: String },
    AudioValidationFailed { format: String, reason: String },
    TaggingFailed { stage: String, reason: String },
    FilesystemFailed { path: String, reason: String },
    DatabaseFailed { operation: String, reason: String },
    RepairInputChanged { reason: String },
    Cancelled { reason: String },
}

impl ErrorTaxonomy {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            ErrorTaxonomy::AuthRefreshable { .. }
                | ErrorTaxonomy::RateLimited { .. }
                | ErrorTaxonomy::TemporaryNetworkFailure { .. }
                | ErrorTaxonomy::Timeout { .. }
        )
    }

    pub fn retry_delay_sec(&self) -> u64 {
        match self {
            ErrorTaxonomy::RateLimited { retry_after_sec, .. } => retry_after_sec.unwrap_or(30),
            ErrorTaxonomy::TemporaryNetworkFailure { .. } => 3,
            ErrorTaxonomy::Timeout { .. } => 5,
            ErrorTaxonomy::AuthRefreshable { .. } => 1,
            _ => 0,
        }
    }

    pub fn max_attempts(&self) -> u32 {
        match self {
            ErrorTaxonomy::TemporaryNetworkFailure { .. } => 3,
            ErrorTaxonomy::Timeout { .. } => 2,
            ErrorTaxonomy::RateLimited { .. } => 3,
            ErrorTaxonomy::AuthRefreshable { .. } => 2,
            _ => 1,
        }
    }

    pub fn invalidates_credentials(&self) -> bool {
        matches!(self, ErrorTaxonomy::AuthInvalid { .. })
    }

    pub fn requires_user_action(&self) -> bool {
        matches!(
            self,
            ErrorTaxonomy::AuthInvalid { .. }
                | ErrorTaxonomy::EntitlementDenied { .. }
                | ErrorTaxonomy::RegionRestricted { .. }
                | ErrorTaxonomy::IdentityConflict { .. }
        )
    }

    pub fn is_terminal(&self) -> bool {
        !self.is_retryable()
    }

    pub fn ui_message(&self) -> String {
        match self {
            ErrorTaxonomy::AuthInvalid { message } => format!("Authentication invalid: {}", message),
            ErrorTaxonomy::AuthRefreshable { message } => format!("Refreshing session: {}", message),
            ErrorTaxonomy::EntitlementDenied { provider, reason } => format!("Access denied on {}: {}", provider, reason),
            ErrorTaxonomy::RejectedQuality { requested, obtained, reason } => format!("Quality {} rejected (obtained {}): {}", requested, obtained, reason),
            ErrorTaxonomy::RegionRestricted { provider, country } => format!("Content unavailable in {} on {}", country, provider),
            ErrorTaxonomy::UnavailableFromProvider { provider, item_id, reason } => format!("Item {} unavailable on {}: {}", item_id, provider, reason),
            ErrorTaxonomy::RateLimited { provider, retry_after_sec } => format!("Rate limit on {}, wait {}s", provider, retry_after_sec.unwrap_or(30)),
            ErrorTaxonomy::TemporaryNetworkFailure { message, .. } => format!("Network error: {}", message),
            ErrorTaxonomy::Timeout { endpoint, elapsed_ms } => format!("Timeout after {}ms on {}", elapsed_ms, endpoint),
            ErrorTaxonomy::MalformedProviderPayload { provider, field, reason } => format!("Malformed {} from {}: {}", field, provider, reason),
            ErrorTaxonomy::IdentityConflict { field, existing_value, conflicting_value } => format!("Conflict on {}: existing '{}' vs candidate '{}'", field, existing_value, conflicting_value),
            ErrorTaxonomy::MetadataResolutionFailed { query, reason, .. } => format!("Metadata failed for {}: {}", query, reason),
            ErrorTaxonomy::AudioValidationFailed { format, reason } => format!("Invalid {} audio: {}", format, reason),
            ErrorTaxonomy::TaggingFailed { stage, reason } => format!("Tagging error in {}: {}", stage, reason),
            ErrorTaxonomy::FilesystemFailed { path, reason } => format!("Disk error at {}: {}", path, reason),
            ErrorTaxonomy::DatabaseFailed { operation, reason } => format!("Database error during {}: {}", operation, reason),
            ErrorTaxonomy::RepairInputChanged { reason } => format!("Repair input changed: {}", reason),
            ErrorTaxonomy::Cancelled { reason } => format!("Operation cancelled: {}", reason),
        }
    }

    pub fn log_severity(&self) -> &'static str {
        match self {
            ErrorTaxonomy::DatabaseFailed { .. }
            | ErrorTaxonomy::FilesystemFailed { .. }
            | ErrorTaxonomy::AudioValidationFailed { .. } => "ERROR",
            ErrorTaxonomy::AuthInvalid { .. }
            | ErrorTaxonomy::EntitlementDenied { .. }
            | ErrorTaxonomy::RejectedQuality { .. }
            | ErrorTaxonomy::IdentityConflict { .. }
            | ErrorTaxonomy::MalformedProviderPayload { .. } => "WARN",
            ErrorTaxonomy::TemporaryNetworkFailure { .. }
            | ErrorTaxonomy::Timeout { .. }
            | ErrorTaxonomy::RateLimited { .. }
            | ErrorTaxonomy::AuthRefreshable { .. }
            | ErrorTaxonomy::RegionRestricted { .. }
            | ErrorTaxonomy::UnavailableFromProvider { .. }
            | ErrorTaxonomy::MetadataResolutionFailed { .. }
            | ErrorTaxonomy::TaggingFailed { .. }
            | ErrorTaxonomy::RepairInputChanged { .. }
            | ErrorTaxonomy::Cancelled { .. } => "INFO",
        }
    }
}

/// Detailed reasons for authentication requirements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequiresAuthReason {
    NoCredentialsStored,
    TokenExpired,
    InvalidPayload,
    DeviceCodePending,
    Unauthorized(String),
}

impl std::fmt::Display for RequiresAuthReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequiresAuthReason::NoCredentialsStored => write!(f, "No active credentials stored"),
            RequiresAuthReason::TokenExpired => write!(f, "User token has expired and refresh failed"),
            RequiresAuthReason::InvalidPayload => write!(f, "Token payload invalid for requested endpoint"),
            RequiresAuthReason::DeviceCodePending => write!(f, "OAuth device code authorization pending"),
            RequiresAuthReason::Unauthorized(msg) => write!(f, "Unauthorized: {}", msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_serialization() {
        let err = PipelineError::RejectedQuality {
            requested: "Lossless".to_string(),
            obtained: "AAC".to_string(),
            reason: "requested_lossless_but_received_aac".to_string(),
        };

        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("rejected_quality"));
        assert!(json.contains("Lossless"));

        let deserialized: PipelineError = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, err);
    }

    #[test]
    fn test_error_taxonomy_and_retryability() {
        let auth_err = PipelineError::RequiresAuth(RequiresAuthReason::TokenExpired);
        assert!(!auth_err.is_retryable());
        assert!(auth_err.is_auth_failure());
        assert_eq!(auth_err.error_code(), "RequiresAuth");

        let playback_err = PipelineError::PlaybackUnauthorized {
            provider: "tidal".to_string(),
            http_status: 401,
            sub_status: Some("11002".to_string()),
            message: "Token has invalid payload".to_string(),
        };
        assert!(!playback_err.is_retryable());
        assert!(playback_err.is_auth_failure());
        assert_eq!(playback_err.error_code(), "PlaybackUnauthorized");

        let quality_err = PipelineError::RejectedQuality {
            requested: "24-192".to_string(),
            obtained: "320".to_string(),
            reason: "Lossy downgrade rejected".to_string(),
        };
        assert!(!quality_err.is_retryable());
        assert!(!quality_err.is_auth_failure());
        assert_eq!(quality_err.error_code(), "RejectedQuality");

        let unresolved_err = PipelineError::TrackUnresolved {
            provider: "tidal".to_string(),
            query: "Unknown Track".to_string(),
        };
        assert!(!unresolved_err.is_retryable());
        assert!(!unresolved_err.is_auth_failure());
        assert_eq!(unresolved_err.error_code(), "TrackUnresolved");

        let net_err = PipelineError::NetworkError {
            provider: "tidal".to_string(),
            endpoint: "playbackinfopostpaywall".to_string(),
            message: "Connection timed out".to_string(),
        };
        assert!(net_err.is_retryable());
        assert!(!net_err.is_auth_failure());
        assert_eq!(net_err.error_code(), "NetworkError");
    }

    #[test]
    fn test_error_taxonomy_comprehensive() {
        let auth_inv = ErrorTaxonomy::AuthInvalid { message: "Token revoked".to_string() };
        assert!(!auth_inv.is_retryable());
        assert!(auth_inv.invalidates_credentials());
        assert!(auth_inv.requires_user_action());
        assert_eq!(auth_inv.log_severity(), "WARN");

        let rate_lim = ErrorTaxonomy::RateLimited { provider: "spotify".to_string(), retry_after_sec: Some(45) };
        assert!(rate_lim.is_retryable());
        assert_eq!(rate_lim.retry_delay_sec(), 45);
        assert_eq!(rate_lim.max_attempts(), 3);
        assert!(!rate_lim.invalidates_credentials());

        let timeout = ErrorTaxonomy::Timeout { endpoint: "api.tidal.com".to_string(), elapsed_ms: 10000 };
        assert!(timeout.is_retryable());
        assert_eq!(timeout.retry_delay_sec(), 5);

        let rejected_q = ErrorTaxonomy::RejectedQuality {
            requested: "Lossless".to_string(),
            obtained: "AAC".to_string(),
            reason: "Strict lossless policy".to_string(),
        };
        assert!(!rejected_q.is_retryable());
        assert!(rejected_q.is_terminal());
    }
}

