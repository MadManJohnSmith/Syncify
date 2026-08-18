//! Unified Progress Emission for Sync & Import Services (S128B)
//!
//! Provides a single structured emission contract for service synchronization progress.

use super::types::SyncProgressEvent;
use tauri::Emitter;

/// Trait for emitting sync progress events to frontend or test subscribers
pub trait SyncProgressEmitter: Send + Sync {
    fn emit_sync_progress(&self, event: &SyncProgressEvent);
}

impl SyncProgressEmitter for tauri::AppHandle {
    fn emit_sync_progress(&self, event: &SyncProgressEvent) {
        let _ = self.emit("sync-progress", event);
        let _ = self.emit("import-progress", event);
        let _ = self.emit("syncify:sync_progress", event);
        if event.terminal {
            if event.status == "completed" {
                let _ = self.emit("sync-complete", event);
                let _ = self.emit("import-complete", serde_json::json!({
                    "service": &event.service,
                    "imported": event.imported_tracks_total,
                    "skipped": 0,
                    "message": &event.message,
                }));
            } else if event.status == "failed" {
                let _ = self.emit("import-failed", serde_json::json!({
                    "service": &event.service,
                    "message": &event.message,
                }));
            } else if event.status == "requires_auth" {
                let _ = self.emit("auth-session-expired", serde_json::json!({
                    "service": &event.service,
                    "message": &event.message,
                }));
            }
        }
    }
}

impl SyncProgressEmitter for tauri::Window {
    fn emit_sync_progress(&self, event: &SyncProgressEvent) {
        let _ = self.emit("sync-progress", event);
        let _ = self.emit("import-progress", event);
        let _ = self.emit("syncify:sync_progress", event);
        if event.terminal {
            if event.status == "completed" {
                let _ = self.emit("sync-complete", event);
                let _ = self.emit("import-complete", serde_json::json!({
                    "service": &event.service,
                    "imported": event.imported_tracks_total,
                    "skipped": 0,
                    "message": &event.message,
                }));
            } else if event.status == "failed" {
                let _ = self.emit("import-failed", serde_json::json!({
                    "service": &event.service,
                    "message": &event.message,
                }));
            } else if event.status == "requires_auth" {
                let _ = self.emit("auth-session-expired", serde_json::json!({
                    "service": &event.service,
                    "message": &event.message,
                }));
            }
        }
    }
}

/// Closure wrapper implementing `SyncProgressEmitter`
#[allow(dead_code)]
pub struct SyncCallback<F>(pub F);

impl<F> SyncProgressEmitter for SyncCallback<F>
where
    F: Fn(&SyncProgressEvent) + Send + Sync,
{
    fn emit_sync_progress(&self, event: &SyncProgressEvent) {
        (self.0)(event);
    }
}

impl SyncProgressEmitter for dyn Fn(&SyncProgressEvent) + Send + Sync {
    fn emit_sync_progress(&self, event: &SyncProgressEvent) {
        (self)(event);
    }
}

impl<T: SyncProgressEmitter + ?Sized> SyncProgressEmitter for &T {
    fn emit_sync_progress(&self, event: &SyncProgressEvent) {
        (*self).emit_sync_progress(event);
    }
}

impl<T: SyncProgressEmitter + ?Sized> SyncProgressEmitter for std::sync::Arc<T> {
    fn emit_sync_progress(&self, event: &SyncProgressEvent) {
        (**self).emit_sync_progress(event);
    }
}

impl<T: SyncProgressEmitter> SyncProgressEmitter for Option<T> {
    fn emit_sync_progress(&self, event: &SyncProgressEvent) {
        if let Some(emitter) = self {
            emitter.emit_sync_progress(event);
        }
    }
}

impl SyncProgressEmitter for () {
    fn emit_sync_progress(&self, _event: &SyncProgressEvent) {}
}
