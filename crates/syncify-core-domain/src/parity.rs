//! Parity Domain Contract
//!
//! Pure, I/O-free behavioral parity structures, canonical snapshots,
//! classification rules, and expected intentional difference registry
//! comparing legacy/syncify-cli and Syncify Tauri/UI.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 20 Mandatory Behavioral Cases for CLI vs Tauri Parity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParityCaseId {
    /// 1. Numeric Tidal ID -> metadata canonical
    Case01NumericTidalIdCanonical,
    /// 2. Same ISRC cross-service -> one canonical track, multiple sources
    Case02SameIsrcCrossServiceDeduplication,
    /// 3. Different masters same title -> distinct tracks
    Case03DifferentMastersSameTitleDistinct,
    /// 4. Strict lossless with AAC response -> RejectedQuality
    Case04StrictLosslessAacResponseRejection,
    /// 5. Fallback provider exact identity
    Case05FallbackProviderExactIdentity,
    /// 6. No provider -> NoDownloadProvider
    Case06NoProviderClassification,
    /// 7. Auth invalid vs entitlement vs 404
    Case07AuthInvalidVsEntitlementVs404,
    /// 8. Placeholder metadata -> Deferred, no fake canonical entity
    Case08PlaceholderMetadataDeferred,
    /// 9. Symbolic title -> tags preserved, safe filename
    Case09SymbolicTitleTagsAndFilename,
    /// 10. Tagging failure -> rollback
    Case10TaggingFailureRollback,
    /// 11. Filesystem failure -> rollback
    Case11FilesystemFailureRollback,
    /// 12. Lyrics failure -> best effort success
    Case12LyricsFailureBestEffort,
    /// 13. Cover failure -> best effort success
    Case13CoverFailureBestEffort,
    /// 14. Interrupted transfer -> recovery
    Case14InterruptedTransferRecovery,
    /// 15. Playlist pagination/order
    Case15PlaylistPaginationOrdering,
    /// 16. Fresh import idempotency
    Case16FreshImportIdempotency,
    /// 17. Repair hash mismatch -> abort
    Case17RepairHashMismatchAbort,
    /// 18. Existing library enrichment precedence
    Case18ExistingLibraryEnrichmentPrecedence,
    /// 19. Concurrency settings effective behavior
    Case19ConcurrencySettingsEffectiveBehavior,
    /// 20. Output path/layout behavior
    Case20OutputPathLayoutBehavior,
}

impl ParityCaseId {
    pub fn all_cases() -> &'static [ParityCaseId] {
        &[
            ParityCaseId::Case01NumericTidalIdCanonical,
            ParityCaseId::Case02SameIsrcCrossServiceDeduplication,
            ParityCaseId::Case03DifferentMastersSameTitleDistinct,
            ParityCaseId::Case04StrictLosslessAacResponseRejection,
            ParityCaseId::Case05FallbackProviderExactIdentity,
            ParityCaseId::Case06NoProviderClassification,
            ParityCaseId::Case07AuthInvalidVsEntitlementVs404,
            ParityCaseId::Case08PlaceholderMetadataDeferred,
            ParityCaseId::Case09SymbolicTitleTagsAndFilename,
            ParityCaseId::Case10TaggingFailureRollback,
            ParityCaseId::Case11FilesystemFailureRollback,
            ParityCaseId::Case12LyricsFailureBestEffort,
            ParityCaseId::Case13CoverFailureBestEffort,
            ParityCaseId::Case14InterruptedTransferRecovery,
            ParityCaseId::Case15PlaylistPaginationOrdering,
            ParityCaseId::Case16FreshImportIdempotency,
            ParityCaseId::Case17RepairHashMismatchAbort,
            ParityCaseId::Case18ExistingLibraryEnrichmentPrecedence,
            ParityCaseId::Case19ConcurrencySettingsEffectiveBehavior,
            ParityCaseId::Case20OutputPathLayoutBehavior,
        ]
    }

    pub fn title(&self) -> &'static str {
        match self {
            ParityCaseId::Case01NumericTidalIdCanonical => "Numeric Tidal ID -> metadata canonical",
            ParityCaseId::Case02SameIsrcCrossServiceDeduplication => "Same ISRC cross-service -> one canonical track, multiple sources",
            ParityCaseId::Case03DifferentMastersSameTitleDistinct => "Different masters same title -> distinct tracks",
            ParityCaseId::Case04StrictLosslessAacResponseRejection => "Strict lossless with AAC response -> RejectedQuality",
            ParityCaseId::Case05FallbackProviderExactIdentity => "Fallback provider exact identity",
            ParityCaseId::Case06NoProviderClassification => "No provider -> NoDownloadProvider",
            ParityCaseId::Case07AuthInvalidVsEntitlementVs404 => "Auth invalid vs entitlement vs 404",
            ParityCaseId::Case08PlaceholderMetadataDeferred => "Placeholder metadata -> Deferred, no fake canonical entity",
            ParityCaseId::Case09SymbolicTitleTagsAndFilename => "Symbolic title -> tags preserved, safe filename",
            ParityCaseId::Case10TaggingFailureRollback => "Tagging failure -> rollback",
            ParityCaseId::Case11FilesystemFailureRollback => "Filesystem failure -> rollback",
            ParityCaseId::Case12LyricsFailureBestEffort => "Lyrics failure -> best effort success",
            ParityCaseId::Case13CoverFailureBestEffort => "Cover failure -> best effort success",
            ParityCaseId::Case14InterruptedTransferRecovery => "Interrupted transfer -> recovery",
            ParityCaseId::Case15PlaylistPaginationOrdering => "Playlist pagination/order",
            ParityCaseId::Case16FreshImportIdempotency => "Fresh import idempotency",
            ParityCaseId::Case17RepairHashMismatchAbort => "Repair hash mismatch -> abort",
            ParityCaseId::Case18ExistingLibraryEnrichmentPrecedence => "Existing library enrichment precedence",
            ParityCaseId::Case19ConcurrencySettingsEffectiveBehavior => "Concurrency settings effective behavior",
            ParityCaseId::Case20OutputPathLayoutBehavior => "Output path/layout behavior",
        }
    }
}

/// Parity Difference Classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ParityClassification {
    /// Observable outcomes are strictly equivalent
    Equivalent,
    /// Intentional divergence designed exclusively for GUI (e.g. IPC events, reactive toast, cancellation token)
    IntentionalUIOnly,
    /// Intentional legacy CLI behavior (e.g. stdout progress bar, single-pass terminal exit code)
    IntentionalCLILegacyOnly,
    /// Unintentional behavioral regression (MUST block release)
    Regression,
    /// Known unsupported behavior explicitly declared and guarded
    UnsupportedButExplicit,
}

/// Normalized snapshot of observable execution results for comparison
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct NormalizedOutputSnapshot {
    /// Canonical track identity (title, artist, album, isrc, duration_ms)
    pub canonical_track_identity: Option<String>,
    /// Number of distinct track sources linked
    pub track_sources_count: usize,
    /// Primary service track ID
    pub primary_service_track_id: Option<String>,
    /// Linked artist & album names
    pub artist_and_album: Option<String>,
    /// Sequential playlist ordering (e.g. "1,2,3,4")
    pub playlist_order: Option<String>,
    /// Download outcome (Success, RejectedQuality, NoDownloadProvider, Failed, SkippedExisting)
    pub download_decision: String,
    /// Effective provider used (tidal, qobuz, etc.)
    pub effective_provider: Option<String>,
    /// Quality class obtained (Lossless, HiRes, Lossy)
    pub quality_decision: Option<String>,
    /// Standardized error taxonomy
    pub error_taxonomy: Option<String>,
    /// Whether the failure is retryable
    pub is_retryable: bool,
    /// Normalized relative filesystem path (e.g. "Artist/Album/01 - Track.flac")
    pub filesystem_path: Option<String>,
    /// Audio codec and container (e.g. "FLAC/FLAC", "AAC/M4A")
    pub codec_and_container: Option<String>,
    /// Tagging result (Success, Skipped, RolledBack)
    pub tagging_result: String,
    /// Vorbis comments map preview (Title, Artist, Album, ISRC, Year)
    pub vorbis_tags_preview: HashMap<String, String>,
    /// Sidecar status (e.g. "LRC:synced,Cover:JPEG")
    pub sidecars_status: String,
    /// Audio payload hash (SHA-256 of pure audio content if valid)
    pub audio_content_hash: Option<String>,
    /// Final SQLite persistence status
    pub sqlite_persisted: bool,
    /// Journal / Recovery state (Committed, Recovered, CleanedStaging)
    pub journal_state: String,
    /// User visible message or UI label
    pub user_visible_message: String,
}

/// Registry item documenting an expected intentional difference between CLI and Tauri
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParityDifferenceRegistryItem {
    pub case_id: ParityCaseId,
    pub difference: String,
    pub reason: String,
    pub owner: String,
    pub test_name: String,
    pub ui_wording: String,
    pub cli_wording: String,
    pub risk: String,
}

/// Complete Parity Execution Result for a case
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParityExecutionResult {
    pub case_id: ParityCaseId,
    pub title: String,
    pub classification: ParityClassification,
    pub cli_snapshot: NormalizedOutputSnapshot,
    pub tauri_snapshot: NormalizedOutputSnapshot,
    pub normalized_diff: Vec<String>,
    pub intentional_difference: Option<ParityDifferenceRegistryItem>,
    pub passed: bool,
}

/// Complete Parity Report across all 20 cases
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParityReport {
    pub total_cases: usize,
    pub equivalent_count: usize,
    pub intentional_ui_count: usize,
    pub intentional_cli_count: usize,
    pub regression_count: usize,
    pub unsupported_count: usize,
    pub all_passed: bool,
    pub results: Vec<ParityExecutionResult>,
    pub registry: Vec<ParityDifferenceRegistryItem>,
}

/// Domain helper to build the canonical expected intentional difference registry
pub fn get_expected_intentional_difference_registry() -> Vec<ParityDifferenceRegistryItem> {
    vec![
        ParityDifferenceRegistryItem {
            case_id: ParityCaseId::Case07AuthInvalidVsEntitlementVs404,
            difference: "Tauri emits IPC 'requires-auth' event and updates reactive account badge, whereas CLI outputs stderr warning and exits with code 1".to_string(),
            reason: "GUI requires non-blocking reactive user prompt while CLI is designed for terminal pipelines".to_string(),
            owner: "Auth & UI Subsystem".to_string(),
            test_name: "test_concurrency_auth_events".to_string(),
            ui_wording: "Authentication required for account".to_string(),
            cli_wording: "Error: 401 Unauthorized - access token expired".to_string(),
            risk: "Low - both correctly halt downloads and mark credentials invalid".to_string(),
        },
        ParityDifferenceRegistryItem {
            case_id: ParityCaseId::Case10TaggingFailureRollback,
            difference: "Tauri persists error taxonomy in downloads SQLite table with transactional journal rollback, CLI prints rollback message to stdout".to_string(),
            reason: "Tauri manages a durable SQLite database and UI queue state".to_string(),
            owner: "Worker & SQLite Layer".to_string(),
            test_name: "test_tagging_failure_rollback".to_string(),
            ui_wording: "Tagging failed: staging audio cleaned up".to_string(),
            cli_wording: "FLAC tag writing error, deleted temp staging file".to_string(),
            risk: "Low - both guarantee 0 orphaned staging files and 0 corrupt files in library".to_string(),
        },
        ParityDifferenceRegistryItem {
            case_id: ParityCaseId::Case14InterruptedTransferRecovery,
            difference: "Tauri uses startup operation journal reconciliation to clean orphaned .part files, CLI checks filesystem during initial scan".to_string(),
            reason: "Tauri tracks lifecycle in operation_journal for deterministic crash recovery across app restarts".to_string(),
            owner: "Recovery Subsystem".to_string(),
            test_name: "test_operation_recovery_crash".to_string(),
            ui_wording: "Crash recovery: 1 partial download cleaned from staging".to_string(),
            cli_wording: "Removed residual .part file".to_string(),
            risk: "Low - both safely purge incomplete payloads".to_string(),
        },
        ParityDifferenceRegistryItem {
            case_id: ParityCaseId::Case19ConcurrencySettingsEffectiveBehavior,
            difference: "Tauri uses dynamic keyed Mutex pool (ConcurrencyManager) with UI toast notifications, CLI uses tokio Semaphore".to_string(),
            reason: "Tauri coordinates multi-threaded UI events, queue retries, and background sync without blocking IPC".to_string(),
            owner: "Concurrency Layer".to_string(),
            test_name: "test_concurrency_stress".to_string(),
            ui_wording: "Max concurrent downloads: 3 (active: 3)".to_string(),
            cli_wording: "Processing with concurrency=3".to_string(),
            risk: "Low - both strictly enforce upper concurrency bound".to_string(),
        },
    ]
}

/// Compare CLI snapshot and Tauri snapshot to derive ParityExecutionResult
pub fn compare_snapshots(
    case_id: ParityCaseId,
    cli: NormalizedOutputSnapshot,
    tauri: NormalizedOutputSnapshot,
    registry: &[ParityDifferenceRegistryItem],
) -> ParityExecutionResult {
    let mut diffs = Vec::new();

    if cli.canonical_track_identity != tauri.canonical_track_identity {
        diffs.push(format!(
            "Canonical Identity mismatch: CLI={:?} vs Tauri={:?}",
            cli.canonical_track_identity, tauri.canonical_track_identity
        ));
    }
    if cli.track_sources_count != tauri.track_sources_count {
        diffs.push(format!(
            "Track sources count mismatch: CLI={} vs Tauri={}",
            cli.track_sources_count, tauri.track_sources_count
        ));
    }
    if cli.download_decision != tauri.download_decision {
        diffs.push(format!(
            "Download decision mismatch: CLI={} vs Tauri={}",
            cli.download_decision, tauri.download_decision
        ));
    }
    if cli.effective_provider != tauri.effective_provider {
        diffs.push(format!(
            "Effective provider mismatch: CLI={:?} vs Tauri={:?}",
            cli.effective_provider, tauri.effective_provider
        ));
    }
    if cli.quality_decision != tauri.quality_decision {
        diffs.push(format!(
            "Quality decision mismatch: CLI={:?} vs Tauri={:?}",
            cli.quality_decision, tauri.quality_decision
        ));
    }
    if cli.error_taxonomy != tauri.error_taxonomy {
        diffs.push(format!(
            "Error taxonomy mismatch: CLI={:?} vs Tauri={:?}",
            cli.error_taxonomy, tauri.error_taxonomy
        ));
    }
    if cli.is_retryable != tauri.is_retryable {
        diffs.push(format!(
            "Retryability mismatch: CLI={} vs Tauri={}",
            cli.is_retryable, tauri.is_retryable
        ));
    }
    if cli.filesystem_path != tauri.filesystem_path {
        diffs.push(format!(
            "Filesystem path mismatch: CLI={:?} vs Tauri={:?}",
            cli.filesystem_path, tauri.filesystem_path
        ));
    }
    if cli.codec_and_container != tauri.codec_and_container {
        diffs.push(format!(
            "Codec/Container mismatch: CLI={:?} vs Tauri={:?}",
            cli.codec_and_container, tauri.codec_and_container
        ));
    }
    if cli.tagging_result != tauri.tagging_result {
        diffs.push(format!(
            "Tagging result mismatch: CLI={} vs Tauri={}",
            cli.tagging_result, tauri.tagging_result
        ));
    }
    if cli.audio_content_hash != tauri.audio_content_hash {
        diffs.push(format!(
            "Audio hash mismatch: CLI={:?} vs Tauri={:?}",
            cli.audio_content_hash, tauri.audio_content_hash
        ));
    }

    let matching_registry_item = registry.iter().find(|r| r.case_id == case_id).cloned();

    let classification = if diffs.is_empty() {
        ParityClassification::Equivalent
    } else if matching_registry_item.is_some() {
        ParityClassification::IntentionalUIOnly
    } else {
        ParityClassification::Regression
    };

    let passed = classification != ParityClassification::Regression;

    ParityExecutionResult {
        case_id,
        title: case_id.title().to_string(),
        classification,
        cli_snapshot: cli,
        tauri_snapshot: tauri,
        normalized_diff: diffs,
        intentional_difference: matching_registry_item,
        passed,
    }
}

/// Compute summary ParityReport from results
pub fn build_parity_report(
    results: Vec<ParityExecutionResult>,
    registry: Vec<ParityDifferenceRegistryItem>,
) -> ParityReport {
    let total_cases = results.len();
    let equivalent_count = results.iter().filter(|r| r.classification == ParityClassification::Equivalent).count();
    let intentional_ui_count = results.iter().filter(|r| r.classification == ParityClassification::IntentionalUIOnly).count();
    let intentional_cli_count = results.iter().filter(|r| r.classification == ParityClassification::IntentionalCLILegacyOnly).count();
    let regression_count = results.iter().filter(|r| r.classification == ParityClassification::Regression).count();
    let unsupported_count = results.iter().filter(|r| r.classification == ParityClassification::UnsupportedButExplicit).count();

    let all_passed = regression_count == 0 && total_cases == 20;

    ParityReport {
        total_cases,
        equivalent_count,
        intentional_ui_count,
        intentional_cli_count,
        regression_count,
        unsupported_count,
        all_passed,
        results,
        registry,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_20_cases_present_and_unique() {
        let cases = ParityCaseId::all_cases();
        assert_eq!(cases.len(), 20, "Must contain exactly 20 cases");

        let mut seen = std::collections::HashSet::new();
        for case in cases {
            assert!(seen.insert(case), "Duplicate case found: {:?}", case);
            assert!(!case.title().is_empty());
        }
    }

    #[test]
    fn test_compare_snapshots_equivalent() {
        let snap1 = NormalizedOutputSnapshot {
            canonical_track_identity: Some("Track:Heroes|Artist:David Bowie".to_string()),
            track_sources_count: 1,
            download_decision: "Success".to_string(),
            effective_provider: Some("tidal".to_string()),
            quality_decision: Some("Lossless".to_string()),
            error_taxonomy: None,
            is_retryable: false,
            filesystem_path: Some("David Bowie/Heroes/01 - Heroes.flac".to_string()),
            codec_and_container: Some("FLAC/FLAC".to_string()),
            tagging_result: "Success".to_string(),
            audio_content_hash: Some("sha256:12345".to_string()),
            ..Default::default()
        };
        let snap2 = snap1.clone();
        let reg = get_expected_intentional_difference_registry();

        let res = compare_snapshots(ParityCaseId::Case01NumericTidalIdCanonical, snap1, snap2, &reg);
        assert_eq!(res.classification, ParityClassification::Equivalent);
        assert!(res.passed);
        assert!(res.normalized_diff.is_empty());
    }

    #[test]
    fn test_compare_snapshots_intentional_difference() {
        let snap_cli = NormalizedOutputSnapshot {
            download_decision: "Failed".to_string(),
            error_taxonomy: Some("RequiresAuth".to_string()),
            user_visible_message: "Error: 401 Unauthorized".to_string(),
            ..Default::default()
        };
        let mut snap_tauri = snap_cli.clone();
        snap_tauri.download_decision = "RequiresAuth".to_string(); // slight UI phrasing diff handled in registry
        let reg = get_expected_intentional_difference_registry();

        let res = compare_snapshots(ParityCaseId::Case07AuthInvalidVsEntitlementVs404, snap_cli, snap_tauri, &reg);
        assert_eq!(res.classification, ParityClassification::IntentionalUIOnly);
        assert!(res.passed);
        assert!(res.intentional_difference.is_some());
    }

    #[test]
    fn test_compare_snapshots_regression_detected() {
        let snap_cli = NormalizedOutputSnapshot {
            canonical_track_identity: Some("Track:Heroes|Artist:David Bowie".to_string()),
            download_decision: "Success".to_string(),
            ..Default::default()
        };
        let snap_tauri = NormalizedOutputSnapshot {
            canonical_track_identity: Some("Track:Unknown|Artist:Unknown".to_string()),
            download_decision: "Success".to_string(),
            ..Default::default()
        };
        let reg = get_expected_intentional_difference_registry();

        let res = compare_snapshots(ParityCaseId::Case01NumericTidalIdCanonical, snap_cli, snap_tauri, &reg);
        assert_eq!(res.classification, ParityClassification::Regression);
        assert!(!res.passed);
        assert!(!res.normalized_diff.is_empty());
    }
}
