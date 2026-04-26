//! Cryptography module for credential encryption
//!
//! Uses AES-256-GCM for encrypting service credentials before storage.
//! Key is stored in the OS Keychain (Windows Credential Manager / macOS Keychain / Linux Secret Service).
//!
//! Sprint 01: Replaced deterministic SHA256-derived key with OS Keychain-backed random key.

#![allow(dead_code)]

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use std::sync::OnceLock;

/// Encryption key stored in OnceLock — initialized once from OS Keychain at startup.
static ENCRYPTION_KEY: OnceLock<[u8; 32]> = OnceLock::new();

/// Keychain service/user identifiers for production use.
const KEYCHAIN_SERVICE: &str = "syncify";
const KEYCHAIN_USER: &str = "encryption-key";

// ═══════════════════════════════════════════════════════
// KEY MANAGEMENT
// ═══════════════════════════════════════════════════════

/// Initialize encryption with an explicitly provided key.
/// Returns Err if the OnceLock has already been initialized (double-init guard).
pub fn init_crypto(key: [u8; 32]) -> Result<(), String> {
    ENCRYPTION_KEY
        .set(key)
        .map_err(|_| "Crypto already initialized".to_string())
}

/// Get the active encryption key. Fails if init_crypto() was never called.
fn get_key() -> Result<&'static [u8; 32], String> {
    ENCRYPTION_KEY
        .get()
        .ok_or_else(|| "Crypto not initialized. Call init_keychain_crypto() first.".to_string())
}

/// Generate a cryptographically secure random 32-byte key using OsRng.
/// OsRng reads directly from BCryptGenRandom (Windows) / /dev/urandom (Linux/macOS)
/// without intermediate state. Correct interface for long-lived cryptographic material.
pub fn generate_random_key() -> [u8; 32] {
    use rand::rngs::OsRng as RandOsRng;
    use rand::RngCore;
    let mut key = [0u8; 32];
    RandOsRng.fill_bytes(&mut key);
    key
}

// ═══════════════════════════════════════════════════════
// OS KEYCHAIN OPERATIONS
// ═══════════════════════════════════════════════════════

/// Load the AES-256 key from the OS Keychain using default service/user identifiers.
fn load_key_from_keychain() -> Result<[u8; 32], String> {
    load_key_from_keychain_with_service(KEYCHAIN_SERVICE, KEYCHAIN_USER)
}

/// Store the AES-256 key in the OS Keychain using default service/user identifiers.
fn store_key_in_keychain(key: &[u8; 32]) -> Result<(), String> {
    store_key_in_keychain_with_service(key, KEYCHAIN_SERVICE, KEYCHAIN_USER)
}

/// Load the AES-256 key from the OS Keychain with parametrized service/user (for tests).
pub fn load_key_from_keychain_with_service(service: &str, user: &str) -> Result<[u8; 32], String> {
    let entry = keyring::Entry::new(service, user)
        .map_err(|e| format!("Keychain entry creation failed: {}", e))?;

    let encoded = entry
        .get_password()
        .map_err(|e| format!("Keychain read failed: {}", e))?;

    let decoded = BASE64
        .decode(&encoded)
        .map_err(|e| format!("Keychain Base64 decode error: {}", e))?;

    if decoded.len() != 32 {
        return Err(format!(
            "Keychain key has invalid length: {} (expected 32)",
            decoded.len()
        ));
    }

    let mut key = [0u8; 32];
    key.copy_from_slice(&decoded);
    Ok(key)
}

/// Store the AES-256 key in the OS Keychain with parametrized service/user (for tests).
pub fn store_key_in_keychain_with_service(
    key: &[u8; 32],
    service: &str,
    user: &str,
) -> Result<(), String> {
    let entry = keyring::Entry::new(service, user)
        .map_err(|e| format!("Keychain entry creation failed: {}", e))?;

    let encoded = BASE64.encode(key);
    entry
        .set_password(&encoded)
        .map_err(|e| format!("Keychain write failed: {}", e))?;

    Ok(())
}

/// Get the fallback key path
fn fallback_key_path() -> std::path::PathBuf {
    let mut path = dirs::data_local_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    path.push("com.syncify.app");
    std::fs::create_dir_all(&path).ok();
    path.push(".crypto_key");
    path
}

/// Initialize crypto from the OS Keychain or fallback file.
pub fn init_keychain_crypto() -> Result<(), String> {
    // Try to load from keychain first
    if let Ok(key) = load_key_from_keychain() {
        tracing::info!("Encryption key loaded from OS Keychain");
        return init_crypto(key);
    }
    
    // Fallback to file storage if keychain fails (common in Windows dev environments or missing cred manager)
    let fallback_path = fallback_key_path();
    if fallback_path.exists() {
        if let Ok(encoded) = std::fs::read_to_string(&fallback_path) {
            if let Ok(decoded) = BASE64.decode(encoded.trim()) {
                if decoded.len() == 32 {
                    let mut key = [0u8; 32];
                    key.copy_from_slice(&decoded);
                    tracing::info!("Encryption key loaded from fallback file");
                    return init_crypto(key);
                }
            }
        }
    }

    // If no key found anywhere, generate a new one
    tracing::info!("No existing key found in OS Keychain or fallback, generating new key...");
    let key = generate_random_key();
    
    // Try to store in keychain
    if let Err(e) = store_key_in_keychain(&key) {
        tracing::warn!("Failed to store key in OS Keychain: {}, using fallback file", e);
    } else {
        tracing::info!("New encryption key stored in OS Keychain");
    }
    
    // Always store in fallback file as a backup
    let encoded = BASE64.encode(&key);
    if let Err(e) = std::fs::write(&fallback_path, encoded) {
        tracing::error!("Failed to write fallback encryption key: {}", e);
    } else {
        tracing::info!("New encryption key stored in fallback file");
    }
    
    init_crypto(key)
}

// ═══════════════════════════════════════════════════════
// ENCRYPT / DECRYPT (public API — signatures UNCHANGED)
// ═══════════════════════════════════════════════════════

/// Encrypt a string for storage.
pub fn encrypt(plaintext: &str) -> Result<String, String> {
    let key = get_key()?;
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| format!("Cipher init error: {}", e))?;

    // Generate random nonce
    let mut nonce_bytes = [0u8; 12];
    use aes_gcm::aead::rand_core::RngCore;
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| format!("Encryption error: {}", e))?;

    // Combine nonce + ciphertext and base64 encode
    let mut combined = nonce_bytes.to_vec();
    combined.extend(ciphertext);

    Ok(BASE64.encode(&combined))
}

/// Decrypt a stored string.
pub fn decrypt(encrypted: &str) -> Result<String, String> {
    let key = get_key()?;
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| format!("Cipher init error: {}", e))?;

    // Decode base64
    let combined = BASE64
        .decode(encrypted)
        .map_err(|e| format!("Base64 decode error: {}", e))?;

    if combined.len() < 12 {
        return Err("Invalid encrypted data: too short".into());
    }

    // Split nonce and ciphertext
    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    // Decrypt
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decryption error: {}", e))?;

    String::from_utf8(plaintext).map_err(|e| format!("UTF-8 decode error: {}", e))
}

// ═══════════════════════════════════════════════════════
// LEGACY MIGRATION
// ═══════════════════════════════════════════════════════

/// Decrypt using an explicitly provided key instead of the OnceLock.
/// PRIVATE — only caller is migrate_legacy_credentials.
fn decrypt_with_key(encrypted: &str, key: &[u8; 32]) -> Result<String, String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| format!("Cipher init error: {}", e))?;

    let combined = BASE64
        .decode(encrypted)
        .map_err(|e| format!("Base64 decode error: {}", e))?;

    if combined.len() < 12 {
        return Err("Invalid encrypted data: too short".into());
    }

    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decryption error: {}", e))?;

    String::from_utf8(plaintext).map_err(|e| format!("UTF-8 decode error: {}", e))
}

/// Derive the legacy encryption key from machine-specific data (SHA256).
///
/// DEPRECATED: Only used for migration from legacy encryption scheme.
/// Will be removed in Sprint 03.
///
/// Contains NO unwrap() or expect() — uses if let Ok / if let Some throughout.
#[deprecated(
    note = "Legacy key derivation. Only used for migration. Will be removed in Sprint 03."
)]
fn derive_stable_key() -> [u8; 32] {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();

    // 1. App identifier (always the same)
    hasher.update(b"syncify-music-library-v1");

    // 2. Username (stable per user)
    if let Ok(user) = std::env::var("USERNAME").or_else(|_| std::env::var("USER")) {
        hasher.update(user.as_bytes());
    }

    // 3. Home directory path (stable per user)
    if let Some(home) = dirs::home_dir() {
        hasher.update(home.to_string_lossy().as_bytes());
    }

    // 4. Hostname (stable per machine)
    if let Ok(hostname) = hostname::get() {
        hasher.update(hostname.to_string_lossy().as_bytes());
    }

    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

/// Migrate credentials encrypted with the legacy SHA256-derived key to the new
/// keychain-backed key. Non-blocking — designed to be called from an async task.
///
/// Schema verified against migrations/0002_normalize_schema.sql:38-48:
///   Table: accounts
///   Column: credentials_json (TEXT, nullable, stores encrypted tokens/cookies)
///
/// Returns Ok((migrated_count, failed_account_ids)).
/// Rows that fail to decrypt with the legacy key are left INTACT (not corrupted).
/// These accounts require manual re-authentication.
#[allow(deprecated)] // derive_stable_key is intentionally used here for migration
pub async fn migrate_legacy_credentials(db: &sqlx::SqlitePool) -> Result<(u32, Vec<i64>), String> {
    // Check if migration was already completed
    let already_done: Option<(String,)> =
        sqlx::query_as("SELECT value FROM settings WHERE key = 'credential_migration_v1_complete'")
            .fetch_optional(db)
            .await
            .map_err(|e| format!("Failed to check migration status: {}", e))?;

    if already_done.is_some() {
        tracing::debug!("Legacy credential migration already completed, skipping");
        return Ok((0, vec![]));
    }

    let legacy_key = derive_stable_key();

    // Read all accounts with non-null credentials
    let rows: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, credentials_json FROM accounts WHERE credentials_json IS NOT NULL",
    )
    .fetch_all(db)
    .await
    .map_err(|e| format!("Failed to read accounts for migration: {}", e))?;

    let mut migrated_count: u32 = 0;
    let mut failed_ids: Vec<i64> = Vec::new();

    for (account_id, ciphertext) in &rows {
        // Attempt decrypt with legacy key — NOT crate::crypto::decrypt()
        match decrypt_with_key(ciphertext, &legacy_key) {
            Ok(plaintext) => {
                // Re-encrypt with new keychain-backed key (via get_key() inside encrypt())
                match encrypt(&plaintext) {
                    Ok(new_ciphertext) => {
                        let update_result =
                            sqlx::query("UPDATE accounts SET credentials_json = ? WHERE id = ?")
                                .bind(&new_ciphertext)
                                .bind(account_id)
                                .execute(db)
                                .await;

                        match update_result {
                            Ok(_) => {
                                migrated_count += 1;
                                tracing::debug!(
                                    "Migrated credentials for account_id={}",
                                    account_id
                                );
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to update migrated credentials for account_id={}: {}",
                                    account_id,
                                    e
                                );
                                failed_ids.push(*account_id);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to re-encrypt credentials for account_id={}: {}",
                            account_id,
                            e
                        );
                        failed_ids.push(*account_id);
                    }
                }
            }
            Err(e) => {
                // Row not decryptable with legacy key — leave intact.
                // Could be: already migrated, different user/machine, or corrupted.
                tracing::warn!(
                    "Failed to migrate credentials for account_id={}: {}. \
                     Row left intact — account requires re-authentication.",
                    account_id,
                    e
                );
                failed_ids.push(*account_id);
            }
        }
    }

    // Determine if migration should be marked as complete.
    // Three conditions trigger completion:
    //
    // 1. Perfect migration: migrated_count > 0 && failed_ids.is_empty()
    //    All legacy credentials were successfully re-encrypted.
    //
    // 2. Fresh install / no credentials: rows.is_empty()
    //    No credentials exist to migrate — mark done so we never run again.
    //
    // 3. All rows failed = no legacy credentials remain:
    //    failed_ids.len() == rows.len() && migrated_count == 0
    //    Every row failed to decrypt with the legacy key. This means either:
    //    (a) All credentials were already migrated in a previous session, OR
    //    (b) All credentials are genuinely corrupt (unrelated to migration).
    //    In both cases, re-running migration on every boot adds no value.
    //    Rows are left intact — accounts will require re-authentication if
    //    genuinely unreadable. Mark as complete to stop infinite retries.
    //
    // NOT marked as complete: partial success (migrated_count > 0 && !failed_ids.is_empty())
    // This case means some rows migrated and some failed — the failed ones
    // might succeed on retry (e.g., transient DB lock). Re-run next boot.
    let should_mark_complete = rows.is_empty()
        || (migrated_count > 0 && failed_ids.is_empty())
        || (migrated_count == 0 && failed_ids.len() == rows.len());

    if should_mark_complete {
        let _ = sqlx::query(
            "INSERT OR REPLACE INTO settings (key, value, updated_at) \
             VALUES ('credential_migration_v1_complete', 'true', CURRENT_TIMESTAMP)",
        )
        .execute(db)
        .await
        .map_err(|e| {
            tracing::warn!("Failed to record migration completion flag: {}", e);
            // Non-fatal: migration will re-run next boot but won't corrupt data
        });
    }

    Ok((migrated_count, failed_ids))
}

// ═══════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires OS keychain daemon (run with cargo test -- --include-ignored)"]
    fn test_keychain_roundtrip() {
        let key = generate_random_key();
        store_key_in_keychain_with_service(&key, "syncify-test", "test-key")
            .expect("Failed to store key in keychain");
        let loaded = load_key_from_keychain_with_service("syncify-test", "test-key")
            .expect("Failed to load key from keychain");
        assert_eq!(key, loaded);
        // Cleanup: remove test entry from OS keychain
        let _ = keyring::Entry::new("syncify-test", "test-key").and_then(|e| e.delete_credential());
    }

    #[test]
    fn test_key_base64_roundtrip() {
        let key = generate_random_key();
        let encoded = BASE64.encode(&key);
        let decoded = BASE64.decode(&encoded).expect("Base64 decode failed");
        assert_eq!(key.len(), 32);
        assert_eq!(&key[..], &decoded[..]);
    }

    #[test]
    fn test_credential_encrypt_decrypt() {
        // OnceLock may already be initialized from another test in this process.
        // init_crypto returns Err if already set — absorb that case.
        let key = generate_random_key();
        let _ = init_crypto(key);

        let original = r#"{"access_token":"abc123","refresh_token":"xyz789"}"#;

        let encrypted = encrypt(original).expect("encrypt() failed");
        assert_ne!(encrypted, original);

        let decrypted = decrypt(&encrypted).expect("decrypt() failed");
        assert_eq!(decrypted, original);
    }
}
