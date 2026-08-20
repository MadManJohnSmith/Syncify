//! Domain models for repair integrity guardrails, baseline validation, and hash reporting.

use serde::{Deserialize, Serialize};

/// Snapshot baseline of an audio file and its optional sidecar LRC
/// calculated during the dry-run inspection phase.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct RepairFileBaseline {
    pub file_path: String,
    pub input_sha256: String,
    pub input_size: u64,
    pub input_modified_at: u64,
    pub audio_content_hash: Option<String>,
    pub lrc_path: Option<String>,
    pub lrc_sha256: Option<String>,
    pub lrc_size: Option<u64>,
    pub lrc_modified_at: Option<u64>,
}

/// Output hash audit capturing before-and-after states of the file,
/// audio payload, and optional sidecar LRC.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct RepairOutputHashes {
    pub file_hash_before: String,
    pub file_hash_after: Option<String>,
    pub audio_content_hash_before: Option<String>,
    pub audio_content_hash_after: Option<String>,
    pub lrc_hash_before: Option<String>,
    pub lrc_hash_after: Option<String>,
}

/// Outcome of pre-flight baseline validation before applying mutations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "status", content = "details", rename_all = "snake_case")]
pub enum RepairValidationStatus {
    Valid,
    RepairInputChanged { reason: String },
    FileNotFound { path: String },
}

impl Default for RepairValidationStatus {
    fn default() -> Self {
        RepairValidationStatus::Valid
    }
}

impl RepairValidationStatus {
    pub fn is_valid(&self) -> bool {
        matches!(self, RepairValidationStatus::Valid)
    }

    pub fn error_message(&self) -> Option<String> {
        match self {
            RepairValidationStatus::Valid => None,
            RepairValidationStatus::RepairInputChanged { reason } => {
                Some(format!("RepairInputChanged: {}", reason))
            }
            RepairValidationStatus::FileNotFound { path } => {
                Some(format!("FileNotFound: {}", path))
            }
        }
    }
}

/// Complete auditable report produced for every repair execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct RepairReport {
    pub success: bool,
    pub dry_run: bool,
    pub download_id: Option<i64>,
    pub track_id: Option<i64>,
    pub source_path: String,
    pub target_path: String,
    pub baseline: Option<RepairFileBaseline>,
    pub validation_result: RepairValidationStatus,
    pub applied_actions: Vec<String>,
    pub rollback_state: Option<String>,
    pub output_hashes: Option<RepairOutputHashes>,
    pub error: Option<String>,
}
