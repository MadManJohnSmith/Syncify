//! Security test suite for Master Key Storage & Permissions [TASK-93] / [SEC-009]
//!
//! Validates:
//! 1. When Keychain storage succeeds, `.crypto_key` is NEVER created on disk.
//! 2. If a legacy `.crypto_key` file previously existed on disk, it is proactively purged when Keychain succeeds.
//! 3. When Keychain fails (e.g. headless, missing D-Bus Secret Service), `.crypto_key` is used as fallback.
//! 4. On Unix platforms, fallback `.crypto_key` is created with strict 0600 permissions.
//! 5. Pre-existing insecure fallback files (0644/0666) have their permissions hardened to 0600 upon load.
//! 6. When a fallback key exists and Keychain becomes available, the key is migrated to Keychain and the file purged.
//! 7. AES-256-GCM encryption and decryption function identically and interoperably between keychain and fallback keys.

use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use syncify_tauri_lib::crypto::{
    decrypt_with_key, encrypt_with_key, generate_random_key, load_fallback_key,
    resolve_or_create_key, write_fallback_key,
};

#[test]
fn test_keychain_success_does_not_create_crypto_key_file() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temporary directory");
    let fallback_key_path = temp_dir.path().join(".crypto_key");

    assert!(!fallback_key_path.exists());

    let mut stored_key: Option<[u8; 32]> = None;

    // Simulate: Keychain has no key stored initially (load fails), but storing new key in Keychain succeeds
    let resolved_key = resolve_or_create_key(
        || Err("Keychain is empty".to_string()),
        |key| {
            stored_key = Some(*key);
            Ok(())
        },
        Some(&fallback_key_path),
    )
    .expect("resolve_or_create_key failed");

    assert_eq!(stored_key, Some(resolved_key));
    // SEC-009 Core Invariant: Master key MUST NOT be written to disk when Keychain is functional
    assert!(
        !fallback_key_path.exists(),
        "Fallback file (.crypto_key) must NOT be generated when Keychain store succeeds"
    );
}

#[test]
fn test_keychain_load_success_removes_preexisting_fallback_file() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temporary directory");
    let fallback_key_path = temp_dir.path().join(".crypto_key");

    // Seed a legacy fallback file
    fs::write(&fallback_key_path, b"legacy_unsecured_master_key")
        .expect("Failed to write legacy key file");
    assert!(fallback_key_path.exists());

    let existing_kc_key = generate_random_key();

    // Simulate: Keychain load succeeds immediately
    let resolved_key = resolve_or_create_key(
        || Ok(existing_kc_key),
        |_| panic!("store_kc must not be called when load_kc succeeds"),
        Some(&fallback_key_path),
    )
    .expect("resolve_or_create_key failed");

    assert_eq!(resolved_key, existing_kc_key);
    // SEC-009 Invariant: Legacy fallback file must be purged proactively
    assert!(
        !fallback_key_path.exists(),
        "Legacy fallback file must be deleted when key is successfully loaded from Keychain"
    );
}

#[test]
fn test_keychain_store_success_removes_preexisting_fallback_file() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temporary directory");
    let fallback_key_path = temp_dir.path().join(".crypto_key");

    // Seed an obsolete file on disk
    fs::write(&fallback_key_path, b"obsolete_key_file").expect("Failed to write obsolete file");
    assert!(fallback_key_path.exists());

    let mut stored_key: Option<[u8; 32]> = None;

    // Simulate: Keychain load fails (empty/new), but store succeeds
    let resolved_key = resolve_or_create_key(
        || Err("Keychain empty".to_string()),
        |k| {
            stored_key = Some(*k);
            Ok(())
        },
        Some(&fallback_key_path),
    )
    .expect("resolve_or_create_key failed");

    assert_eq!(stored_key, Some(resolved_key));
    assert!(
        !fallback_key_path.exists(),
        "Existing .crypto_key file must be purged when a new key is successfully stored in Keychain"
    );
}

#[test]
fn test_keychain_failure_triggers_fallback_with_strict_0600_permissions() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temporary directory");
    let fallback_key_path = temp_dir.path().join(".crypto_key");

    assert!(!fallback_key_path.exists());

    // Simulate: Headless / missing Secret Service / D-Bus failure
    let resolved_key = resolve_or_create_key(
        || Err("D-Bus Secret Service not available".to_string()),
        |_| Err("D-Bus Secret Service connection refused".to_string()),
        Some(&fallback_key_path),
    )
    .expect("resolve_or_create_key should succeed using fallback");

    // Fallback file must exist
    assert!(
        fallback_key_path.exists(),
        "Fallback file (.crypto_key) must be generated when Keychain operations fail"
    );

    #[cfg(unix)]
    {
        let metadata = fs::metadata(&fallback_key_path).expect("Failed to read metadata");
        let mode = metadata.permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "Fallback file must be created with strict 0600 permissions (got 0o{:o})",
            mode
        );
    }

    // Verify written content is valid Base64 decoding to the generated 32-byte key
    let content = fs::read_to_string(&fallback_key_path).expect("Failed to read fallback file");
    let decoded = BASE64.decode(content.trim()).expect("Base64 decode failed");
    assert_eq!(decoded.len(), 32);
    assert_eq!(&decoded[..], &resolved_key[..]);
}

#[test]
fn test_write_fallback_key_enforces_0600() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temporary directory");
    let fallback_key_path = temp_dir.path().join(".crypto_key");
    let key = generate_random_key();

    write_fallback_key(&fallback_key_path, &key).expect("write_fallback_key failed");

    #[cfg(unix)]
    {
        let metadata = fs::metadata(&fallback_key_path).expect("Failed to read metadata");
        let mode = metadata.permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "write_fallback_key must enforce 0600 permissions"
        );
    }

    let loaded = load_fallback_key(&fallback_key_path).expect("load_fallback_key failed");
    assert_eq!(key, loaded);
}

#[test]
fn test_load_fallback_key_hardens_insecure_permissions() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temporary directory");
    let fallback_key_path = temp_dir.path().join(".crypto_key");
    let key = generate_random_key();

    // Intentionally write file with overly permissive mode 0644
    fs::write(&fallback_key_path, BASE64.encode(key)).expect("Failed to write key");

    #[cfg(unix)]
    {
        fs::set_permissions(&fallback_key_path, fs::Permissions::from_mode(0o644))
            .expect("Failed to set 0644");
        let mode_before = fs::metadata(&fallback_key_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode_before, 0o644);
    }

    let loaded = load_fallback_key(&fallback_key_path).expect("load_fallback_key failed");
    assert_eq!(key, loaded);

    #[cfg(unix)]
    {
        let mode_after = fs::metadata(&fallback_key_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            mode_after, 0o600,
            "load_fallback_key must remediate insecure 0644 permissions to strict 0600"
        );
    }
}

#[test]
fn test_fallback_migration_to_keychain_when_available() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temporary directory");
    let fallback_key_path = temp_dir.path().join(".crypto_key");
    let fallback_key = generate_random_key();

    // Fallback key exists from a prior headless run
    write_fallback_key(&fallback_key_path, &fallback_key).expect("Failed to write fallback");
    assert!(fallback_key_path.exists());

    let migrated_to_kc = Arc::new(AtomicBool::new(false));
    let migrated_to_kc_clone = Arc::clone(&migrated_to_kc);

    // Keychain load fails (not yet in keychain), but store succeeds (keychain is now alive)
    let resolved_key = resolve_or_create_key(
        || Err("Key not yet in keychain".to_string()),
        move |k| {
            assert_eq!(*k, fallback_key);
            migrated_to_kc_clone.store(true, Ordering::SeqCst);
            Ok(())
        },
        Some(&fallback_key_path),
    )
    .expect("resolve_or_create_key failed");

    assert_eq!(resolved_key, fallback_key);
    assert!(migrated_to_kc.load(Ordering::SeqCst), "Fallback key should be migrated to Keychain");
    assert!(
        !fallback_key_path.exists(),
        "Fallback file should be purged after migration to Keychain"
    );
}

#[test]
fn test_aes_256_gcm_interoperability_between_modes() {
    let keychain_key = generate_random_key();
    let fallback_key = generate_random_key();

    let sensitive_payload = r#"{"account":"spotify","token":"BQC123xyz","refresh":"AQB789"}"#;

    // 1. Encrypt and decrypt with keychain-simulated key
    let encrypted_kc = encrypt_with_key(sensitive_payload, &keychain_key)
        .expect("Encryption with keychain key failed");
    assert_ne!(encrypted_kc, sensitive_payload);
    let decrypted_kc = decrypt_with_key(&encrypted_kc, &keychain_key)
        .expect("Decryption with keychain key failed");
    assert_eq!(decrypted_kc, sensitive_payload);

    // 2. Encrypt and decrypt with fallback-simulated key
    let encrypted_fb = encrypt_with_key(sensitive_payload, &fallback_key)
        .expect("Encryption with fallback key failed");
    assert_ne!(encrypted_fb, sensitive_payload);
    let decrypted_fb = decrypt_with_key(&encrypted_fb, &fallback_key)
        .expect("Decryption with fallback key failed");
    assert_eq!(decrypted_fb, sensitive_payload);

    // 3. Verify cross-key protection: key A cannot decrypt payload encrypted with key B
    let cross_decrypt = decrypt_with_key(&encrypted_kc, &fallback_key);
    assert!(
        cross_decrypt.is_err(),
        "Ciphertext encrypted with one key must fail decryption with a different key"
    );

    // 4. Verify disk persistence roundtrip with fallback key
    let temp_dir = tempfile::tempdir().expect("Failed to create temporary directory");
    let fallback_key_path = temp_dir.path().join(".crypto_key");

    write_fallback_key(&fallback_key_path, &fallback_key).expect("write_fallback_key failed");
    let loaded_from_disk = load_fallback_key(&fallback_key_path).expect("load_fallback_key failed");
    assert_eq!(loaded_from_disk, fallback_key);

    let decrypted_from_disk_key = decrypt_with_key(&encrypted_fb, &loaded_from_disk)
        .expect("Decryption with key loaded from disk failed");
    assert_eq!(decrypted_from_disk_key, sensitive_payload);
}
