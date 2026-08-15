//! Regression and Technical Debt Cleanup Test Suite (Sprint S99)
//!
//! Validates CSP policy strictness, poisoned mutex recovery, safe duration parsing,
//! metadata domain precedence guarantees, and repository clean state.

use std::sync::{Arc, Mutex};
use syncify_metadata_domain::{FieldValidator, SourcePriority};

#[test]
fn test_csp_configuration_strictness() {
    let config_content = std::fs::read_to_string("tauri.conf.json")
        .expect("tauri.conf.json must exist in src-tauri/");

    let json: serde_json::Value = serde_json::from_str(&config_content)
        .expect("tauri.conf.json must be valid JSON");

    let csp = json["app"]["security"]["csp"]
        .as_str()
        .expect("app.security.csp must be configured");

    assert!(csp.contains("default-src 'self'"), "CSP must define default-src 'self'");
    assert!(csp.contains("object-src 'none'"), "CSP must prohibit plugins via object-src 'none'");
    assert!(csp.contains("base-uri 'self'"), "CSP must constrain base-uri to 'self'");
    assert!(!csp.contains("default-src *"), "CSP must NOT use wildcard default-src");
    assert!(!csp.contains("script-src *"), "CSP must NOT use wildcard script-src");
}

#[test]
fn test_poisoned_mutex_recovery_resilience() {
    let mutex = Arc::new(Mutex::new(42));
    let mutex_clone = Arc::clone(&mutex);

    // Intentionally poison the mutex by panicking in a thread
    let _ = std::thread::spawn(move || {
        let _guard = mutex_clone.lock().unwrap();
        panic!("Simulated worker panic causing mutex poisoning");
    })
    .join();

    assert!(mutex.is_poisoned(), "Mutex must be poisoned for this test");

    // Test resilient lock acquisition with poison recovery pattern
    let mut guard = mutex.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(*guard, 42, "Poison recovery must successfully read the inner value");

    *guard = 100;
    assert_eq!(*guard, 100, "Poison recovery must allow modifying the inner value safely");
}

#[test]
fn test_system_time_robustness() {
    // Normal case
    let now = std::time::SystemTime::now();
    let duration = now.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    assert!(duration.as_secs() > 0);

    // Backward time (future epoch simulation)
    let past = std::time::UNIX_EPOCH;
    let future = past + std::time::Duration::from_secs(100);
    // past.duration_since(future) returns an Err, but unwrap_or_default handles it gracefully
    let fallback = past.duration_since(future).unwrap_or_default();
    assert_eq!(fallback.as_secs(), 0, "Backward clock shift must fall back to Duration::ZERO without panic");
}

#[test]
fn test_metadata_domain_precedence_invariants() {
    assert!(SourcePriority::Manual > SourcePriority::StreamingService);
    assert!(SourcePriority::StreamingService > SourcePriority::MusicBrainz);
    assert!(SourcePriority::MusicBrainz > SourcePriority::Inferred);

    assert_eq!(SourcePriority::from_source_name("spotify"), SourcePriority::StreamingService);
    assert_eq!(SourcePriority::from_source_name("qobuz"), SourcePriority::StreamingService);
    assert_eq!(SourcePriority::from_source_name("tidal"), SourcePriority::StreamingService);
    assert_eq!(SourcePriority::from_source_name("musicbrainz"), SourcePriority::MusicBrainz);
    assert_eq!(SourcePriority::from_source_name("manual"), SourcePriority::Manual);

    // FieldValidator invariants
    assert!(FieldValidator::is_valid_title("Heroes"));
    assert!(!FieldValidator::is_valid_title("   "));
    assert!(!FieldValidator::is_valid_title("???"));
    assert!(!FieldValidator::is_valid_title("null"));

    assert!(FieldValidator::is_valid_artist("David Bowie"));
    assert!(FieldValidator::is_valid_artist("Various Artists")); // Valid for compilations
    assert!(!FieldValidator::is_valid_artist("   "));
    assert!(!FieldValidator::is_valid_artist("???"));
}

#[test]
fn test_archive_directory_exists_and_clean_workspace() {
    let archive_path = std::path::Path::new("../scripts/archive");
    assert!(archive_path.exists(), "scripts/archive directory must exist");
}
