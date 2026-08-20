//! Pure domain contracts, lock scopes, hierarchy levels, and safe telemetry for concurrency management.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Global Lock Hierarchy Levels
///
/// Prevents circular wait deadlocks:
/// Any operation acquiring multiple locks MUST acquire them in strictly increasing level order.
/// If multiple locks of the same level are acquired, they MUST be acquired in sorted key order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LockHierarchyLevel {
    /// Level 1: Per-account synchronization exclusion
    AccountSync = 1,
    /// Level 2: Whole-catalog / batch coordinator
    CatalogWrite = 2,
    /// Level 3: Service-specific track identity (service_id, service_track_id)
    TrackIdentity = 3,
    /// Level 4: Canonical track mutation & metadata precedence
    CanonicalTrack = 4,
    /// Level 5: Download & Repair mutual exclusion
    DownloadOrRepair = 5,
    /// Level 6: Physical filesystem target path
    FilesystemPath = 6,
    /// Level 7: Coherent settings snapshots
    Settings = 7,
}

impl LockHierarchyLevel {
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }
}

/// Fine-grained Lock Scope
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "key", rename_all = "snake_case")]
pub enum LockScope {
    /// Serializes sync operations for the same account
    AccountSync(i64),
    /// Coordinates batch catalog write/repair operations
    CatalogWrite,
    /// Keyed by (service_id, service_track_id) to prevent duplicate track_sources
    TrackIdentity {
        service_id: i64,
        service_track_id: String,
    },
    /// Keyed by canonical track_id to prevent metadata/favorite/playlist link races
    CanonicalTrack(i64),
    /// Exclusive lock for an active download of a track
    Download(i64),
    /// Exclusive lock for repairing a download/file (mutually exclusive with Download)
    Repair(i64),
    /// Keyed by normalized canonical filesystem path
    FilesystemPath(String),
    /// Read/Write lock for application settings
    Settings,
}

impl LockScope {
    /// Returns the hierarchy level of the lock scope
    pub fn hierarchy_level(&self) -> LockHierarchyLevel {
        match self {
            LockScope::AccountSync(_) => LockHierarchyLevel::AccountSync,
            LockScope::CatalogWrite => LockHierarchyLevel::CatalogWrite,
            LockScope::TrackIdentity { .. } => LockHierarchyLevel::TrackIdentity,
            LockScope::CanonicalTrack(_) => LockHierarchyLevel::CanonicalTrack,
            LockScope::Download(_) | LockScope::Repair(_) => LockHierarchyLevel::DownloadOrRepair,
            LockScope::FilesystemPath(_) => LockHierarchyLevel::FilesystemPath,
            LockScope::Settings => LockHierarchyLevel::Settings,
        }
    }

    /// Generates a normalized internal key for map indexing and mutual exclusion
    pub fn to_key_string(&self) -> String {
        match self {
            LockScope::AccountSync(acc_id) => format!("acc:{}", acc_id),
            LockScope::CatalogWrite => "catalog:write".to_string(),
            LockScope::TrackIdentity {
                service_id,
                service_track_id,
            } => format!("identity:{}:{}", service_id, service_track_id),
            LockScope::CanonicalTrack(tid) => format!("track:{}", tid),
            // Download and Repair share the mutual exclusion key "dl_rep:{track_id}"
            LockScope::Download(tid) => format!("dl_rep:{}", tid),
            LockScope::Repair(tid) => format!("dl_rep:{}", tid),
            LockScope::FilesystemPath(path) => {
                let norm = path.replace('\\', "/").to_lowercase();
                format!("fs:{}", norm)
            }
            LockScope::Settings => "settings:global".to_string(),
        }
    }

    /// Generates a redacted diagnostic key hash (zero PII, zero tokens, zero personal paths)
    pub fn to_redacted_key_hash(&self) -> String {
        let raw = self.to_key_string();
        // Return prefix with short SHA-like deterministic tag
        let mut hash: u64 = 5381;
        for byte in raw.bytes() {
            hash = ((hash << 5).wrapping_add(hash)).wrapping_add(byte as u64);
        }
        let type_name = match self {
            LockScope::AccountSync(_) => "AccountSync",
            LockScope::CatalogWrite => "CatalogWrite",
            LockScope::TrackIdentity { .. } => "TrackIdentity",
            LockScope::CanonicalTrack(_) => "CanonicalTrack",
            LockScope::Download(_) => "Download",
            LockScope::Repair(_) => "Repair",
            LockScope::FilesystemPath(_) => "FilesystemPath",
            LockScope::Settings => "Settings",
        };
        format!("{}:{:016x}", type_name, hash)
    }
}

impl PartialOrd for LockScope {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LockScope {
    fn cmp(&self, other: &Self) -> Ordering {
        let level_cmp = self.hierarchy_level().cmp(&other.hierarchy_level());
        if level_cmp != Ordering::Equal {
            return level_cmp;
        }
        match (self, other) {
            (LockScope::AccountSync(a), LockScope::AccountSync(b)) => a.cmp(b),
            (LockScope::CatalogWrite, LockScope::CatalogWrite) => Ordering::Equal,
            (
                LockScope::TrackIdentity {
                    service_id: s1,
                    service_track_id: t1,
                },
                LockScope::TrackIdentity {
                    service_id: s2,
                    service_track_id: t2,
                },
            ) => (s1, t1).cmp(&(s2, t2)),
            (LockScope::CanonicalTrack(a), LockScope::CanonicalTrack(b)) => a.cmp(b),
            (LockScope::Download(a), LockScope::Download(b)) => a.cmp(b),
            (LockScope::Repair(a), LockScope::Repair(b)) => a.cmp(b),
            (LockScope::Download(a), LockScope::Repair(b)) => a.cmp(b),
            (LockScope::Repair(a), LockScope::Download(b)) => a.cmp(b),
            (LockScope::FilesystemPath(a), LockScope::FilesystemPath(b)) => a.cmp(b),
            (LockScope::Settings, LockScope::Settings) => Ordering::Equal,
            _ => Ordering::Equal,
        }
    }
}

/// Lock acquisition outcome
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LockOutcome {
    /// Lock was acquired immediately without waiting
    Acquired,
    /// Lock was acquired after waiting for a previous holder
    ContendedAcquired,
    /// Acquisition timed out
    Timeout,
    /// Acquisition failed due to lock manager shutdown or error
    Failed,
}

/// Redacted, safe diagnostic telemetry for lock operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockTelemetry {
    pub operation_id: String,
    pub lock_type: String,
    pub key_hash: String,
    pub wait_duration_ms: u64,
    pub held_duration_ms: Option<u64>,
    pub contention_count: u64,
    pub timeout_ms: Option<u64>,
    pub outcome: LockOutcome,
}

/// Concurrency statistics summary for UI / monitoring
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ConcurrencyStatsSummary {
    pub total_acquisitions: u64,
    pub contended_acquisitions: u64,
    pub timeouts: u64,
    pub active_locks_count: usize,
    pub max_wait_duration_ms: u64,
    pub max_held_duration_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lock_hierarchy_ordering_prevents_inversion() {
        let s_acc = LockScope::AccountSync(1);
        let s_cat = LockScope::CatalogWrite;
        let s_ident = LockScope::TrackIdentity {
            service_id: 1,
            service_track_id: "123".to_string(),
        };
        let s_track = LockScope::CanonicalTrack(42);
        let s_dl = LockScope::Download(42);
        let s_rep = LockScope::Repair(42);
        let s_fs = LockScope::FilesystemPath("C:/Music/song.flac".to_string());
        let s_sett = LockScope::Settings;

        assert!(s_acc < s_cat);
        assert!(s_cat < s_ident);
        assert!(s_ident < s_track);
        assert!(s_track < s_dl);
        assert!(s_track < s_rep);
        assert!(s_dl < s_fs);
        assert!(s_fs < s_sett);
    }

    #[test]
    fn test_download_and_repair_share_exclusion_key() {
        let s_dl = LockScope::Download(999);
        let s_rep = LockScope::Repair(999);

        assert_eq!(s_dl.to_key_string(), s_rep.to_key_string());
        assert_eq!(s_dl.to_key_string(), "dl_rep:999");
    }

    #[test]
    fn test_redacted_key_hash_contains_no_pii() {
        let s_acc = LockScope::AccountSync(123456);
        let hash = s_acc.to_redacted_key_hash();
        assert!(!hash.contains("123456"));
        assert!(hash.starts_with("AccountSync:"));

        let s_fs = LockScope::FilesystemPath("C:/Users/JohnDoe/Private/secret_track.flac".to_string());
        let fs_hash = s_fs.to_redacted_key_hash();
        assert!(!fs_hash.contains("JohnDoe"));
        assert!(!fs_hash.contains("secret_track"));
        assert!(fs_hash.starts_with("FilesystemPath:"));
    }

    #[test]
    fn test_multi_scope_sorting_for_deadlock_prevention() {
        let mut scopes = vec![
            LockScope::Settings,
            LockScope::CanonicalTrack(10),
            LockScope::AccountSync(1),
            LockScope::CanonicalTrack(5),
            LockScope::FilesystemPath("C:/test.flac".to_string()),
        ];

        scopes.sort();

        assert_eq!(scopes[0], LockScope::AccountSync(1));
        assert_eq!(scopes[1], LockScope::CanonicalTrack(5));
        assert_eq!(scopes[2], LockScope::CanonicalTrack(10));
        assert_eq!(scopes[3], LockScope::FilesystemPath("C:/test.flac".to_string()));
        assert_eq!(scopes[4], LockScope::Settings);
    }
}
