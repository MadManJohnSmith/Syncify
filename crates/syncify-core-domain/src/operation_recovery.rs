//! Domain models for persistent operation journal, checkpointing, and post-crash recovery.

use serde::{Deserialize, Serialize};

/// High-level type of operation tracked by the persistent journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationType {
    ServiceSync,
    PlaylistImport,
    QueueEnqueue,
    DownloadQobuz,
    DownloadTidal,
    CrossProviderFallback,
    Tagging,
    Promotion,
    CatalogIdentityRepair,
    MetadataPathRepair,
}

impl OperationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            OperationType::ServiceSync => "service_sync",
            OperationType::PlaylistImport => "playlist_import",
            OperationType::QueueEnqueue => "queue_enqueue",
            OperationType::DownloadQobuz => "download_qobuz",
            OperationType::DownloadTidal => "download_tidal",
            OperationType::CrossProviderFallback => "cross_provider_fallback",
            OperationType::Tagging => "tagging",
            OperationType::Promotion => "promotion",
            OperationType::CatalogIdentityRepair => "catalog_identity_repair",
            OperationType::MetadataPathRepair => "metadata_path_repair",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "service_sync" => Some(OperationType::ServiceSync),
            "playlist_import" => Some(OperationType::PlaylistImport),
            "queue_enqueue" => Some(OperationType::QueueEnqueue),
            "download_qobuz" => Some(OperationType::DownloadQobuz),
            "download_tidal" => Some(OperationType::DownloadTidal),
            "cross_provider_fallback" => Some(OperationType::CrossProviderFallback),
            "tagging" => Some(OperationType::Tagging),
            "promotion" => Some(OperationType::Promotion),
            "catalog_identity_repair" => Some(OperationType::CatalogIdentityRepair),
            "metadata_path_repair" => Some(OperationType::MetadataPathRepair),
            _ => None,
        }
    }
}

/// Lifecycle status of an operation recorded in the journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationStatus {
    Planned,
    Started,
    Checkpointed,
    Persisting,
    Committed,
    RolledBack,
    Interrupted,
    Recovering,
    Recovered,
    FailedTerminal,
    Cancelled,
}

impl OperationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            OperationStatus::Planned => "planned",
            OperationStatus::Started => "started",
            OperationStatus::Checkpointed => "checkpointed",
            OperationStatus::Persisting => "persisting",
            OperationStatus::Committed => "committed",
            OperationStatus::RolledBack => "rolled_back",
            OperationStatus::Interrupted => "interrupted",
            OperationStatus::Recovering => "recovering",
            OperationStatus::Recovered => "recovered",
            OperationStatus::FailedTerminal => "failed_terminal",
            OperationStatus::Cancelled => "cancelled",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "planned" => Some(OperationStatus::Planned),
            "started" => Some(OperationStatus::Started),
            "checkpointed" => Some(OperationStatus::Checkpointed),
            "persisting" => Some(OperationStatus::Persisting),
            "committed" => Some(OperationStatus::Committed),
            "rolled_back" => Some(OperationStatus::RolledBack),
            "interrupted" => Some(OperationStatus::Interrupted),
            "recovering" => Some(OperationStatus::Recovering),
            "recovered" => Some(OperationStatus::Recovered),
            "failed_terminal" => Some(OperationStatus::FailedTerminal),
            "cancelled" => Some(OperationStatus::Cancelled),
            _ => None,
        }
    }

    /// Whether this status represents a finished state that needs no crash recovery.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            OperationStatus::Committed
                | OperationStatus::RolledBack
                | OperationStatus::Recovered
                | OperationStatus::FailedTerminal
                | OperationStatus::Cancelled
        )
    }

    /// UI display message conforming to the user-facing recovery requirement.
    pub fn display_label(&self) -> &'static str {
        match self {
            OperationStatus::Recovered => "Recovered after restart",
            OperationStatus::Interrupted => "Interrupted — retry available",
            OperationStatus::FailedTerminal => "Failed terminal — user action required",
            OperationStatus::Committed => "Completed",
            OperationStatus::RolledBack => "Rolled back safely",
            OperationStatus::Cancelled => "Cancelled",
            OperationStatus::Recovering => "Recovering...",
            OperationStatus::Persisting => "Persisting...",
            OperationStatus::Checkpointed => "In progress (checkpointed)",
            OperationStatus::Started => "Started",
            OperationStatus::Planned => "Planned",
        }
    }
}

/// Execution phase within an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationPhase {
    Init,
    Transfer,
    Validate,
    Metadata,
    Lyrics,
    Tagging,
    Promotion,
    Persist,
    Completed,
}

impl OperationPhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            OperationPhase::Init => "init",
            OperationPhase::Transfer => "transfer",
            OperationPhase::Validate => "validate",
            OperationPhase::Metadata => "metadata",
            OperationPhase::Lyrics => "lyrics",
            OperationPhase::Tagging => "tagging",
            OperationPhase::Promotion => "promotion",
            OperationPhase::Persist => "persist",
            OperationPhase::Completed => "completed",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "init" => Some(OperationPhase::Init),
            "transfer" => Some(OperationPhase::Transfer),
            "validate" => Some(OperationPhase::Validate),
            "metadata" => Some(OperationPhase::Metadata),
            "lyrics" => Some(OperationPhase::Lyrics),
            "tagging" => Some(OperationPhase::Tagging),
            "promotion" => Some(OperationPhase::Promotion),
            "persist" => Some(OperationPhase::Persist),
            "completed" => Some(OperationPhase::Completed),
            _ => None,
        }
    }
}

/// An entry in the persistent operation journal table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OperationJournalEntry {
    pub operation_id: String,
    pub operation_type: OperationType,
    pub entity_id: Option<String>,
    pub account_id: Option<i64>,
    pub track_id: Option<i64>,
    pub download_id: Option<i64>,
    pub provider: Option<String>,
    pub phase: OperationPhase,
    pub attempt: i32,
    pub started_at: String,
    pub checkpoint_at: String,
    pub status: OperationStatus,
    pub input_identity: Option<String>,
    pub expected_output_path: Option<String>,
    pub staging_path: Option<String>,
    pub file_baseline: Option<String>,
    pub db_transaction_state: Option<String>,
    pub rollback_state: Option<String>,
    pub error_taxonomy: Option<String>,
    pub retry_policy: Option<String>,
    pub result_summary: Option<String>,
}

/// Actions decided during post-crash startup reconciliation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryAction {
    PromoteAndCommit,
    CompletePromotion,
    ReconcileDbOnly,
    RollbackStaging,
    RollbackFileToBaseline,
    MarkTerminal,
    ScheduleRetry,
    MarkRecovered,
    NoOp,
}

/// Detailed outcome for a single recovered or reconciled operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OperationRecoveryDetail {
    pub operation_id: String,
    pub operation_type: OperationType,
    pub previous_status: OperationStatus,
    pub new_status: OperationStatus,
    pub phase: OperationPhase,
    pub action_taken: RecoveryAction,
    pub message: String,
    pub ui_label: String,
    pub error_taxonomy: Option<String>,
}

/// Aggregate summary of post-crash startup reconciliation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryAuditSummary {
    pub total_journal_scanned: usize,
    pub active_operations_found: usize,
    pub recovered_count: usize,
    pub interrupted_retryable_count: usize,
    pub failed_terminal_count: usize,
    pub cleaned_staging_files: usize,
    pub details: Vec<OperationRecoveryDetail>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operation_status_terminal_and_labels() {
        assert!(OperationStatus::Committed.is_terminal());
        assert!(OperationStatus::Recovered.is_terminal());
        assert!(OperationStatus::FailedTerminal.is_terminal());
        assert!(OperationStatus::Cancelled.is_terminal());
        assert!(OperationStatus::RolledBack.is_terminal());

        assert!(!OperationStatus::Started.is_terminal());
        assert!(!OperationStatus::Checkpointed.is_terminal());
        assert!(!OperationStatus::Persisting.is_terminal());
        assert!(!OperationStatus::Recovering.is_terminal());

        assert_eq!(OperationStatus::Recovered.display_label(), "Recovered after restart");
        assert_eq!(OperationStatus::Interrupted.display_label(), "Interrupted — retry available");
        assert_eq!(OperationStatus::FailedTerminal.display_label(), "Failed terminal — user action required");
    }

    #[test]
    fn test_operation_type_and_phase_serialization() {
        let op = OperationType::DownloadTidal;
        assert_eq!(op.as_str(), "download_tidal");
        assert_eq!(OperationType::from_str("download_tidal"), Some(op));

        let phase = OperationPhase::Promotion;
        assert_eq!(phase.as_str(), "promotion");
        assert_eq!(OperationPhase::from_str("promotion"), Some(phase));
    }
}
