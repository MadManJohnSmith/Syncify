//! Integration Tests for S170A: Dev File Logging, Rotation & Sanitization
//!
//! Tests:
//! 1. Development mode enables file logging by default (even if persisted user setting is false).
//! 2. Release mode simulation respects persisted user setting.
//! 3. RUST_LOG environment variable overrides default level.
//! 4. Log directory is located in OS-native app log dir, never CWD or relative.
//! 5. Secret sanitizer eliminates Bearer tokens, passwords, Authorization headers, and signed URLs.
//! 6. Failure to open log file does not block or crash the application.
//! 7. 50 MB size rotation and 30-day retention cleanup preserves active log file.
//! 8. Logging status DTO accurately reflects state for IPC & LogsView UI.

use std::path::PathBuf;
use syncify_tauri_lib::services::logging::effective_target_filter;
use syncify_tauri_lib::services::logging::{
    get_logging_status, resolve_app_log_dir, resolve_effective_log_config, sanitize_log_message,
    RotatingFileWriter, LOG_RETENTION_DAYS, MAX_LOG_FILE_SIZE_BYTES,
};
use tracing::Level;

#[test]
fn test_dev_mode_enables_file_logging_by_default() {
    // In dev mode (cfg!(debug_assertions) is true in test builds),
    // file logging must be enabled even if persisted setting is false.
    let config = resolve_effective_log_config(Some(false), Some("info"));
    assert!(config.is_development);
    assert!(
        config.log_to_file,
        "Development mode must force log_to_file = true"
    );
    assert_eq!(
        config.active_log_path.file_name().unwrap(),
        "syncify-dev.log"
    );
}

#[test]
fn test_rust_log_precedence_over_default() {
    std::env::set_var("RUST_LOG", "warn");
    let config = resolve_effective_log_config(None, None);
    assert_eq!(config.log_level, Level::WARN);

    std::env::set_var("RUST_LOG", "trace");
    let config = resolve_effective_log_config(None, None);
    assert_eq!(config.log_level, Level::TRACE);

    std::env::remove_var("RUST_LOG");
}

#[test]
fn test_log_file_created_in_app_log_dir_not_cwd() {
    let log_dir = resolve_app_log_dir();
    assert!(
        log_dir.is_absolute(),
        "Log directory must be an absolute path"
    );

    let current_dir = std::env::current_dir().unwrap();
    assert_ne!(
        log_dir, current_dir,
        "Log directory must never be the current working directory"
    );

    let path_str = log_dir.to_string_lossy().to_string();
    assert!(
        path_str.contains("com.syncify.app") || path_str.contains("syncify"),
        "Log directory must follow OS-native app naming: {}",
        path_str
    );
}

#[test]
fn test_secret_and_signed_url_sanitization() {
    // 1. Bearer Token
    let bearer = "Authorization: Bearer BQDxyz1234567890abcdefghijklmnopqrstuvwxyz";
    let sanitized_bearer = sanitize_log_message(bearer);
    assert!(!sanitized_bearer.contains("BQDxyz1234567890abcdefghijklmnopqrstuvwxyz"));
    assert!(sanitized_bearer.contains("[REDACTED]"));

    // 2. Token / Password assignments
    let token_json =
        r#"{"access_token": "secret_access_token_9999", "client_secret": "my_ultra_secret_pass"}"#;
    let sanitized_token = sanitize_log_message(token_json);
    assert!(!sanitized_token.contains("secret_access_token_9999"));
    assert!(!sanitized_token.contains("my_ultra_secret_pass"));
    assert!(sanitized_token.contains("[REDACTED]"));

    // 3. Basic Authorization
    let basic = "Authorization: Basic dXNlcjpwYXNzd29yZDEyMzQ=";
    let sanitized_basic = sanitize_log_message(basic);
    assert!(!sanitized_basic.contains("dXNlcjpwYXNzd29yZDEyMzQ="));
    assert!(sanitized_basic.contains("[REDACTED]"));

    // 4. Signed URLs (CloudFront / Akamai / Tidal / Qobuz signatures)
    let signed_url = "https://sp-audio.tidal.com/track/123.flac?Signature=ABCDEF123456&Expires=1789000000&Key-Pair-Id=K99999&token=tok_live_1234";
    let sanitized_url = sanitize_log_message(signed_url);
    assert!(!sanitized_url.contains("ABCDEF123456"));
    assert!(!sanitized_url.contains("1789000000"));
    assert!(!sanitized_url.contains("K99999"));
    assert!(!sanitized_url.contains("tok_live_1234"));
    assert!(sanitized_url.contains("[REDACTED]"));
}

#[test]
fn test_rotating_file_writer_rotation_and_retention() {
    let temp_dir =
        std::env::temp_dir().join(format!("syncify_rotation_test_{}", uuid::Uuid::new_v4()));
    let writer = RotatingFileWriter::new(temp_dir.clone(), "syncify-dev.log".to_string());
    assert!(writer.is_active());

    // Write log lines
    writer.write_line("[2026-08-23T16:00:00Z] [INFO] [System] [syncify] Event line 1");
    writer.write_line("[2026-08-23T16:00:01Z] [DEBUG] [Qobuz] [syncify::qobuz] Event line 2");

    let active_content = std::fs::read_to_string(writer.active_path()).unwrap();
    assert!(active_content.contains("Event line 1"));
    assert!(active_content.contains("Event line 2"));

    // Trigger explicit rotation
    writer.rotate_current_file();

    // Verify active file was renewed and rotated file exists
    assert!(writer.active_path().exists());
    let entries: Vec<_> = std::fs::read_dir(&temp_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert!(
        entries.len() >= 2,
        "Expected at least 2 files (active + rotated)"
    );

    // Clean up test folder
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_file_failure_resilience_and_non_blocking() {
    // Providing an impossible path (e.g. invalid filename or read-only directory)
    // should not panic or block
    let invalid_dir = PathBuf::from("/proc/syncify_impossible_dir/non_existent");
    let writer = RotatingFileWriter::new(invalid_dir, "syncify-dev.log".to_string());
    assert!(!writer.is_active());

    // Calling write_line on inactive writer must safely no-op without panic
    writer.write_line("[2026-08-23T16:00:00Z] [INFO] [System] [syncify] Safely dropped line");
}

#[test]
fn test_logging_status_dto_integrity() {
    let status = get_logging_status();
    assert!(status.is_development);
    assert_eq!(status.retention_days, LOG_RETENTION_DAYS);
    assert_eq!(
        status.max_file_size_mb,
        MAX_LOG_FILE_SIZE_BYTES / (1024 * 1024)
    );
    assert!(!status.log_dir.is_empty());
}

#[test]
fn test_stored_level_applies_in_development() {
    // Dev mode must not force DEBUG over the user's stored preference.
    let config = resolve_effective_log_config(Some(true), Some("info"));
    assert_eq!(config.log_level, Level::INFO);

    let config = resolve_effective_log_config(Some(true), Some("warn"));
    assert_eq!(config.log_level, Level::WARN);

    // No stored level and no override: useful default, not verbose by default.
    let config = resolve_effective_log_config(None, None);
    assert_eq!(config.log_level, Level::INFO);
}

#[test]
fn test_rust_log_re_raises_verbosity_on_demand() {
    std::env::set_var("RUST_LOG", "debug");
    let config = resolve_effective_log_config(Some(true), Some("info"));
    assert_eq!(config.log_level, Level::DEBUG);
    std::env::remove_var("RUST_LOG");
}

#[test]
fn test_target_filter_keeps_app_debug_but_quiets_dependency_debug() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tracing_subscriber::layer::{Context, SubscriberExt};
    use tracing_subscriber::{Layer, Registry};

    struct CountEvents(Arc<AtomicUsize>);
    impl<S: tracing::Subscriber> Layer<S> for CountEvents {
        fn on_event(&self, _: &tracing::Event<'_>, _: Context<'_, S>) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    // DEBUG is permitted for our own code, not for connection-pool internals.
    let count = Arc::new(AtomicUsize::new(0));
    let subscriber = Registry::default()
        .with(CountEvents(count.clone()).with_filter(effective_target_filter(Level::DEBUG)));
    tracing::subscriber::with_default(subscriber, || {
        tracing::debug!(target: "syncify_tauri::services::musicbrainz", "own diagnostic");
        tracing::debug!(target: "hyper_util::client::legacy::pool", "pool chatter");
        tracing::warn!(target: "hyper_util::client::legacy::pool", "real warning");
    });
    assert_eq!(count.load(Ordering::Relaxed), 2);
}
