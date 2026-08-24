//! Service notification deduplication and dispatch module
//! Manages structured, deduplicated notifications across sync and download operations.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use lazy_static::lazy_static;
use tracing::info;
use crate::commands::types::ServiceNotification;

lazy_static! {
    static ref NOTIFICATION_CACHE: Mutex<HashMap<String, Instant>> = Mutex::new(HashMap::new());
}

#[allow(dead_code)] // ventana de dedupe consumida por should_emit_notification (cluster cubierto por tests)
const DEDUPE_WINDOW: Duration = Duration::from_secs(10);

/// Create a structured ServiceNotification with a stable deduplication key
#[allow(dead_code)] // notificaciones estructuradas: cubiertas por notification_auth_state_test; emisor pendiente de cablear al frontend
pub fn create_service_notification(
    service: &str,
    account_id: Option<i64>,
    operation: &str, // "sync" | "download"
    kind: &str,      // "auth" | "entitlement" | "rate_limit" | "network" | "quality" | "expansion"
    severity: &str,  // "info" | "warning" | "error"
    message: &str,
) -> ServiceNotification {
    let dedupe_key = format!("{}:{}:{}:{}:{}", service, account_id.unwrap_or(0), operation, kind, message);
    ServiceNotification {
        service: service.to_string(),
        account_id,
        operation: operation.to_string(),
        kind: kind.to_string(),
        severity: severity.to_string(),
        dedupe_key,
        message: message.to_string(),
        occurred_at: chrono::Utc::now().to_rfc3339(),
        resolved_at: None,
    }
}

/// Check if a notification should be emitted or suppressed by deduplication
#[allow(dead_code)] // notificaciones estructuradas: cubiertas por notification_auth_state_test; emisor pendiente de cablear al frontend
pub fn should_emit_notification(notification: &ServiceNotification) -> bool {
    let mut cache = match NOTIFICATION_CACHE.lock() {
        Ok(c) => c,
        Err(_) => return true,
    };

    let now = Instant::now();
    // Prune stale entries
    cache.retain(|_, last_seen| now.duration_since(*last_seen) < DEDUPE_WINDOW);

    if let Some(last_seen) = cache.get(&notification.dedupe_key) {
        if now.duration_since(*last_seen) < DEDUPE_WINDOW {
            return false;
        }
    }

    cache.insert(notification.dedupe_key.clone(), now);
    true
}

/// Reset notification cache (useful for testing)
#[allow(dead_code)] // notificaciones estructuradas: cubiertas por notification_auth_state_test; emisor pendiente de cablear al frontend
pub fn clear_notification_cache() {
    if let Ok(mut cache) = NOTIFICATION_CACHE.lock() {
        cache.clear();
    }
}

/// Emit structured notification to frontend if not deduplicated
#[allow(dead_code)] // notificaciones estructuradas: cubiertas por notification_auth_state_test; emisor pendiente de cablear al frontend
pub fn emit_service_notification<R: tauri::Runtime>(
    app_handle: &tauri::AppHandle<R>,
    notification: ServiceNotification,
) {
    if should_emit_notification(&notification) {
        use tauri::Emitter;
        info!(
            service = %notification.service,
            operation = %notification.operation,
            kind = %notification.kind,
            severity = %notification.severity,
            dedupe_key = %notification.dedupe_key,
            "Emitting service notification"
        );
        let _ = app_handle.emit("service-notification", &notification);
    }
}
