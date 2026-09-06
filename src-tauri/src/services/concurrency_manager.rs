//! Centralized asynchronous Keyed Lock Manager and single-writer concurrency controller.
//!
//! Enforces the global lock hierarchy (AccountSync -> CatalogWrite -> TrackIdentity -> CanonicalTrack -> Download/Repair -> FilesystemPath -> Settings)
//! and provides RAII guards with bounded timeouts and redacted telemetry.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, OwnedMutexGuard, RwLock};
use tracing::{debug, warn};

use syncify_core_domain::{
    ConcurrencyStatsSummary, LockHierarchyLevel, LockOutcome, LockScope, LockTelemetry,
};

/// Default timeout for lock acquisition (prevents indefinite blocking)
pub const DEFAULT_LOCK_TIMEOUT: Duration = Duration::from_secs(30);

/// Errors that can occur during lock acquisition
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConcurrencyError {
    Timeout {
        scope: LockScope,
        timeout_ms: u64,
        key_hash: String,
    },
    HierarchyViolation {
        expected_level: LockHierarchyLevel,
        actual_level: LockHierarchyLevel,
    },
    Poisoned(String),
}

impl std::fmt::Display for ConcurrencyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConcurrencyError::Timeout {
                timeout_ms,
                key_hash,
                ..
            } => write!(
                f,
                "Lock acquisition timed out after {}ms for lock [{}]",
                timeout_ms, key_hash
            ),
            ConcurrencyError::HierarchyViolation {
                expected_level,
                actual_level,
            } => write!(
                f,
                "Lock hierarchy inversion: expected >= {:?}, got {:?}",
                expected_level, actual_level
            ),
            ConcurrencyError::Poisoned(msg) => write!(f, "Concurrency lock poisoned: {}", msg),
        }
    }
}

impl std::error::Error for ConcurrencyError {}

struct KeyEntry {
    mutex: Arc<Mutex<()>>,
    ref_count: usize,
}

lazy_static::lazy_static! {
    static ref GLOBAL_CONCURRENCY_MANAGER: Arc<ConcurrencyManager> = Arc::new(ConcurrencyManager::new());
}

/// Returns the global shared instance of ConcurrencyManager
pub fn get_global_concurrency_manager() -> Arc<ConcurrencyManager> {
    Arc::clone(&GLOBAL_CONCURRENCY_MANAGER)
}

/// Centralized Concurrency Manager
pub struct ConcurrencyManager {
    /// Mutex registry keyed by normalized string representation
    registry: Mutex<HashMap<String, KeyEntry>>,
    /// Global catalog write coordinator (reader-writer)
    #[allow(dead_code)]
    catalog_rwlock: RwLock<()>,
    /// Global settings coordinator (reader-writer)
    #[allow(dead_code)]
    settings_rwlock: RwLock<()>,
    /// Telemetry & Statistics
    total_acquisitions: AtomicU64,
    contended_acquisitions: AtomicU64,
    timeouts: AtomicU64,
    max_wait_ms: AtomicU64,
    max_held_ms: AtomicU64,
}

impl Default for ConcurrencyManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConcurrencyManager {
    /// Creates a new instance of the ConcurrencyManager
    pub fn new() -> Self {
        Self {
            registry: Mutex::new(HashMap::new()),
            catalog_rwlock: RwLock::new(()),
            settings_rwlock: RwLock::new(()),
            total_acquisitions: AtomicU64::new(0),
            contended_acquisitions: AtomicU64::new(0),
            timeouts: AtomicU64::new(0),
            max_wait_ms: AtomicU64::new(0),
            max_held_ms: AtomicU64::new(0),
        }
    }

    /// Acquires an exclusive RAII lock guard for the given LockScope with bounded timeout
    pub async fn acquire(
        self: &Arc<Self>,
        scope: LockScope,
        operation_id: Option<&str>,
        timeout_dur: Option<Duration>,
    ) -> Result<ConcurrencyGuard, ConcurrencyError> {
        let op_id = operation_id.unwrap_or("anon-op").to_string();
        let key = scope.to_key_string();
        let key_hash = scope.to_redacted_key_hash();
        let timeout = timeout_dur.unwrap_or(DEFAULT_LOCK_TIMEOUT);

        // 1. Get or create the underlying Mutex
        let lock_arc = {
            let mut reg = self.registry.lock().await;
            let entry = reg.entry(key.clone()).or_insert_with(|| KeyEntry {
                mutex: Arc::new(Mutex::new(())),
                ref_count: 0,
            });
            entry.ref_count += 1;
            Arc::clone(&entry.mutex)
        };

        let start_wait = Instant::now();

        // 2. Try acquiring lock within timeout
        let guard_res = tokio::time::timeout(timeout, lock_arc.lock_owned()).await;

        let wait_dur = start_wait.elapsed();
        let wait_ms = wait_dur.as_millis() as u64;

        // Update max wait time metric
        self.max_wait_ms.fetch_max(wait_ms, Ordering::Relaxed);

        match guard_res {
            Ok(owned_guard) => {
                let outcome = if wait_dur > Duration::from_millis(5) {
                    self.contended_acquisitions.fetch_add(1, Ordering::Relaxed);
                    LockOutcome::ContendedAcquired
                } else {
                    LockOutcome::Acquired
                };

                self.total_acquisitions.fetch_add(1, Ordering::Relaxed);

                debug!(
                    op_id = %op_id,
                    scope = ?scope,
                    key_hash = %key_hash,
                    wait_ms = wait_ms,
                    outcome = ?outcome,
                    "Acquired concurrency lock"
                );

                Ok(ConcurrencyGuard {
                    manager: Arc::clone(self),
                    scope,
                    key,
                    key_hash,
                    operation_id: op_id,
                    acquired_at: Instant::now(),
                    wait_ms,
                    guard: Some(owned_guard),
                })
            }
            Err(_) => {
                self.timeouts.fetch_add(1, Ordering::Relaxed);
                // Decrement ref count on failure
                let mut reg = self.registry.lock().await;
                if let Some(entry) = reg.get_mut(&key) {
                    entry.ref_count = entry.ref_count.saturating_sub(1);
                    if entry.ref_count == 0 {
                        reg.remove(&key);
                    }
                }

                warn!(
                    op_id = %op_id,
                    scope = ?scope,
                    key_hash = %key_hash,
                    timeout_ms = timeout.as_millis() as u64,
                    "Concurrency lock acquisition timed out"
                );

                Err(ConcurrencyError::Timeout {
                    scope,
                    timeout_ms: timeout.as_millis() as u64,
                    key_hash,
                })
            }
        }
    }

    /// Acquires multiple locks simultaneously in strictly sorted hierarchy and key order to prevent deadlocks
    pub async fn acquire_multi(
        self: &Arc<Self>,
        mut scopes: Vec<LockScope>,
        operation_id: Option<&str>,
        timeout_dur: Option<Duration>,
    ) -> Result<MultiConcurrencyGuard, ConcurrencyError> {
        let op_id = operation_id.unwrap_or("anon-multi-op").to_string();
        let timeout = timeout_dur.unwrap_or(DEFAULT_LOCK_TIMEOUT);
        let start_total = Instant::now();

        // 1. Sort scopes by hierarchy level and key order (Deadlock Prevention Order)
        scopes.sort();
        scopes.dedup();

        let mut guards = Vec::with_capacity(scopes.len());

        for scope in scopes {
            let elapsed = start_total.elapsed();
            if elapsed >= timeout {
                return Err(ConcurrencyError::Timeout {
                    scope: scope.clone(),
                    timeout_ms: timeout.as_millis() as u64,
                    key_hash: scope.to_redacted_key_hash(),
                });
            }
            let remaining_budget = timeout.saturating_sub(elapsed);
            let guard = self
                .acquire(scope, Some(&op_id), Some(remaining_budget))
                .await?;
            guards.push(guard);
        }

        Ok(MultiConcurrencyGuard { guards })
    }

    /// Internal cleanup called when a ConcurrencyGuard is dropped
    pub(crate) async fn release_key(&self, key: &str, held_ms: u64) {
        self.max_held_ms.fetch_max(held_ms, Ordering::Relaxed);

        let mut reg = self.registry.lock().await;
        if let Some(entry) = reg.get_mut(key) {
            entry.ref_count = entry.ref_count.saturating_sub(1);
            if entry.ref_count == 0 {
                reg.remove(key);
            }
        }
    }

    /// Returns the current concurrency statistics summary for telemetry and UI
    pub async fn get_stats_summary(&self) -> ConcurrencyStatsSummary {
        let active_count = {
            let reg = self.registry.lock().await;
            reg.len()
        };

        ConcurrencyStatsSummary {
            total_acquisitions: self.total_acquisitions.load(Ordering::Relaxed),
            contended_acquisitions: self.contended_acquisitions.load(Ordering::Relaxed),
            timeouts: self.timeouts.load(Ordering::Relaxed),
            active_locks_count: active_count,
            max_wait_duration_ms: self.max_wait_ms.load(Ordering::Relaxed),
            max_held_duration_ms: self.max_held_ms.load(Ordering::Relaxed),
        }
    }

    /// Returns a list of active redacted key hashes
    pub async fn get_active_locks(&self) -> Vec<String> {
        let reg = self.registry.lock().await;
        reg.keys()
            .map(|k| {
                let mut hash: u64 = 5381;
                for b in k.bytes() {
                    hash = ((hash << 5).wrapping_add(hash)).wrapping_add(b as u64);
                }
                format!("lock:{:016x}", hash)
            })
            .collect()
    }
}

/// RAII Guard that releases the lock upon completion, drop, error, or panic
pub struct ConcurrencyGuard {
    manager: Arc<ConcurrencyManager>,
    pub scope: LockScope,
    key: String,
    pub key_hash: String,
    pub operation_id: String,
    acquired_at: Instant,
    pub wait_ms: u64,
    guard: Option<OwnedMutexGuard<()>>,
}

impl ConcurrencyGuard {
    /// Returns telemetry details for this guard
    pub fn telemetry(&self) -> LockTelemetry {
        LockTelemetry {
            operation_id: self.operation_id.clone(),
            lock_type: format!("{:?}", self.scope.hierarchy_level()),
            key_hash: self.key_hash.clone(),
            wait_duration_ms: self.wait_ms,
            held_duration_ms: Some(self.acquired_at.elapsed().as_millis() as u64),
            contention_count: 0,
            timeout_ms: None,
            outcome: LockOutcome::Acquired,
        }
    }
}

impl Drop for ConcurrencyGuard {
    fn drop(&mut self) {
        // Drop the underlying mutex guard first
        let _ = self.guard.take();
        let held_ms = self.acquired_at.elapsed().as_millis() as u64;

        let manager = Arc::clone(&self.manager);
        let key = self.key.clone();

        // Spawn async background cleanup only if a Tokio runtime is available, avoiding panic during thread teardown/shutdown
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                manager.release_key(&key, held_ms).await;
            });
        } else {
            tracing::debug!("No active Tokio runtime in ConcurrencyGuard::drop, skipping async release_key for '{}'", key);
        }
    }
}

/// RAII Guard managing multiple ordered locks simultaneously
pub struct MultiConcurrencyGuard {
    guards: Vec<ConcurrencyGuard>,
}

impl MultiConcurrencyGuard {
    pub fn count(&self) -> usize {
        self.guards.len()
    }

    pub fn scopes(&self) -> Vec<LockScope> {
        self.guards.iter().map(|g| g.scope.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_concurrency_manager_mutual_exclusion_same_key() {
        let mgr = Arc::new(ConcurrencyManager::new());
        let scope = LockScope::AccountSync(101);

        let g1 = mgr.acquire(scope.clone(), Some("op-1"), Some(Duration::from_millis(50))).await.unwrap();
        assert_eq!(g1.operation_id, "op-1");

        // Concurrent attempt with short timeout should time out
        let g2_res = mgr.acquire(scope.clone(), Some("op-2"), Some(Duration::from_millis(50))).await;
        assert!(matches!(g2_res, Err(ConcurrencyError::Timeout { .. })));

        // Once g1 is dropped, g3 should succeed immediately
        drop(g1);
        tokio::time::sleep(Duration::from_millis(10)).await;

        let g3 = mgr.acquire(scope, Some("op-3"), Some(Duration::from_millis(100))).await.unwrap();
        assert_eq!(g3.operation_id, "op-3");
    }

    #[tokio::test]
    async fn test_concurrency_manager_different_accounts_coexist() {
        let mgr = Arc::new(ConcurrencyManager::new());

        let g1 = mgr.acquire(LockScope::AccountSync(1), Some("op-acc-1"), None).await.unwrap();
        let g2 = mgr.acquire(LockScope::AccountSync(2), Some("op-acc-2"), None).await.unwrap();

        assert_eq!(g1.operation_id, "op-acc-1");
        assert_eq!(g2.operation_id, "op-acc-2");
    }

    #[tokio::test]
    async fn test_download_and_repair_mutual_exclusion() {
        let mgr = Arc::new(ConcurrencyManager::new());

        let dl_guard = mgr.acquire(LockScope::Download(42), Some("op-dl"), Some(Duration::from_millis(50))).await.unwrap();

        // Attempting to acquire Repair on same track should fail with timeout
        let rep_res = mgr.acquire(LockScope::Repair(42), Some("op-repair"), Some(Duration::from_millis(50))).await;
        assert!(matches!(rep_res, Err(ConcurrencyError::Timeout { .. })));

        drop(dl_guard);
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Now repair succeeds
        let rep_guard = mgr.acquire(LockScope::Repair(42), Some("op-repair-2"), Some(Duration::from_millis(100))).await.unwrap();
        assert_eq!(rep_guard.operation_id, "op-repair-2");
    }

    #[tokio::test]
    async fn test_multi_lock_deadlock_free_acquisition() {
        let mgr = Arc::new(ConcurrencyManager::new());

        let scopes = vec![
            LockScope::FilesystemPath("C:/music/track.flac".to_string()),
            LockScope::CanonicalTrack(5),
            LockScope::AccountSync(1),
        ];

        let multi_guard = mgr.acquire_multi(scopes, Some("op-batch"), None).await.unwrap();
        assert_eq!(multi_guard.count(), 3);

        let ordered = multi_guard.scopes();
        assert_eq!(ordered[0], LockScope::AccountSync(1));
        assert_eq!(ordered[1], LockScope::CanonicalTrack(5));
        assert_eq!(ordered[2], LockScope::FilesystemPath("C:/music/track.flac".to_string()));
    }

    #[tokio::test]
    async fn test_concurrency_guard_drop_without_tokio_reactor_does_not_panic() {
        let mgr = Arc::new(ConcurrencyManager::new());
        let guard = mgr
            .acquire(LockScope::AccountSync(999), Some("op-drop-test"), None)
            .await
            .unwrap();

        // Spawn a plain std::thread with NO Tokio runtime to drop the guard
        let join_handle = std::thread::spawn(move || {
            drop(guard);
        });

        let res = join_handle.join();
        assert!(
            res.is_ok(),
            "Dropping ConcurrencyGuard outside Tokio runtime panicked!"
        );

        // Verify lock can be reacquired cleanly
        let guard2 = mgr
            .acquire(
                LockScope::AccountSync(999),
                Some("op-reacquire"),
                Some(Duration::from_millis(100)),
            )
            .await;
        assert!(
            guard2.is_ok(),
            "Should be able to reacquire lock after guard dropped outside Tokio"
        );
    }
}
